use std::path::PathBuf;
use std::sync::Arc;
use std::{net::TcpListener as StdTcpListener, process::ExitCode};

use arc_swap::ArcSwap;
use axum::{
    routing::{any, get},
    Router,
};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use service_router::{
    config::{load_config, watcher::start_config_watcher},
    registry::factory::build_resolver,
    routing::{rebuild_router, SharedRouter},
    server::{
        handlers::{health_handler, proxy_handler, ready_handler},
        AppState,
    },
};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}

async fn run() -> anyhow::Result<ExitCode> {
    let command = parse_command(std::env::args().skip(1).collect());
    match command {
        Command::Run(config_path) => run_server(config_path).await,
        Command::CheckConfig {
            config_path,
            as_json,
            strict,
        } => check_config(config_path, as_json, strict).await,
        Command::Doctor {
            config_path,
            probe_upstream,
            as_json,
        } => doctor(config_path, probe_upstream, as_json).await,
        Command::RouteExplain {
            config_path,
            path,
            method,
            headers,
            as_json,
            verbose,
        } => route_explain(config_path, path, method, headers, as_json, verbose),
        Command::Help => {
            print_help();
            Ok(ExitCode::SUCCESS)
        }
    }
}

async fn run_server(config_path: PathBuf) -> anyhow::Result<ExitCode> {

    // --- Load initial config ---
    let config = load_config(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to load config from {}: {}", config_path.display(), e))?;

    // --- Set up logging ---
    let log_level = config.log_level.clone();
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&log_level)),
        )
        .with(fmt::layer())
        .init();

    info!("service-router starting — config: {}", config_path.display());

    // --- Shared config slot (hot-reload) ---
    let config_slot = Arc::new(ArcSwap::from_pointee(config.clone()));

    // --- Build registry resolver ---
    let resolver = build_resolver(&config).await?;
    let resolver_slot = Arc::new(ArcSwap::from_pointee(resolver));

    // --- Build initial router snapshot ---
    let shared_router: SharedRouter = Arc::new(ArcSwap::from_pointee(
        service_router::routing::RouterSnapshot::from_config(&config)
            .map_err(|e| anyhow::anyhow!("Failed to build router: {}", e))?,
    ));

    // --- Start config hot-reload watcher ---
    let _watcher = start_config_watcher(config_path.clone(), Arc::clone(&config_slot))
        .map_err(|e| anyhow::anyhow!("Failed to start config watcher: {}", e))?;

    // Hook router rebuild to config changes. A background task watches the
    // config slot and rebuilds the router when the config changes.
    {
        let router_clone = Arc::clone(&shared_router);
        let config_clone = Arc::clone(&config_slot);
        tokio::spawn(async move {
            let mut last_ptr = Arc::as_ptr(&config_clone.load_full()) as usize;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                let current = config_clone.load_full();
                let current_ptr = Arc::as_ptr(&current) as usize;
                if current_ptr != last_ptr {
                    last_ptr = current_ptr;
                    if let Err(e) = rebuild_router(&router_clone, &current) {
                        tracing::error!("Router rebuild failed: {}", e);
                    } else {
                        info!("Router rebuilt after config change");
                    }
                }
            }
        });
    }

    // --- Build Axum app ---
    let state = AppState::new(
        shared_router,
        resolver_slot,
        config_slot,
        config.server.upstream_timeout_secs,
    );

    let listen_addr = format!("{}:{}", config.server.host, config.server.port);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .fallback(any(proxy_handler))
        .with_state(state);

    info!("Listening on {}", listen_addr);

    let listener = TcpListener::bind(&listen_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("service-router stopped");
    Ok(ExitCode::SUCCESS)
}

/// Waits for SIGINT (Ctrl-C) or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl-C");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    info!("Shutdown signal received");
}

#[derive(Debug)]
enum Command {
    Run(PathBuf),
    CheckConfig {
        config_path: PathBuf,
        as_json: bool,
        strict: bool,
    },
    Doctor {
        config_path: PathBuf,
        probe_upstream: bool,
        as_json: bool,
    },
    RouteExplain {
        config_path: PathBuf,
        path: String,
        method: String,
        headers: Vec<(String, String)>,
        as_json: bool,
        verbose: bool,
    },
    Help,
}

fn parse_command(args: Vec<String>) -> Command {
    let default_config = || PathBuf::from("config/config.yaml");
    match args.first().map(String::as_str) {
        None => Command::Run(default_config()),
        Some("run") => Command::Run(
            args.get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_config),
        ),
        Some("check-config") => {
            let mut as_json = false;
            let mut strict = false;
            let mut config_path = default_config();
            for arg in args.iter().skip(1) {
                if arg == "--json" {
                    as_json = true;
                } else if arg == "--strict" {
                    strict = true;
                } else {
                    config_path = PathBuf::from(arg);
                }
            }
            Command::CheckConfig {
                config_path,
                as_json,
                strict,
            }
        }
        Some("doctor") => {
            let mut config_path = default_config();
            let mut probe_upstream = false;
            let mut as_json = false;
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--config" {
                    if let Some(value) = args.get(i + 1) {
                        config_path = PathBuf::from(value);
                        i += 2;
                        continue;
                    }
                } else if args[i] == "--probe-upstream" {
                    probe_upstream = true;
                } else if args[i] == "--json" {
                    as_json = true;
                } else {
                    config_path = PathBuf::from(&args[i]);
                }
                i += 1;
            }
            Command::Doctor {
                config_path,
                probe_upstream,
                as_json,
            }
        }
        Some("route-explain") => {
            let path = args.get(1).cloned().unwrap_or_else(|| "/".to_string());
            let method = args.get(2).cloned().unwrap_or_else(|| "GET".to_string());
            let mut config_path = default_config();
            let mut headers = Vec::new();
            let mut as_json = false;
            let mut verbose = false;
            let mut i = 3;
            while i < args.len() {
                let arg = &args[i];
                if arg == "--config" {
                    if let Some(value) = args.get(i + 1) {
                        config_path = PathBuf::from(value);
                        i += 2;
                        continue;
                    }
                } else if arg == "--header" {
                    if let Some(value) = args.get(i + 1) {
                        if let Some((k, v)) = value.split_once(':') {
                            headers.push((k.trim().to_string(), v.trim().to_string()));
                        }
                        i += 2;
                        continue;
                    }
                } else if arg == "--json" {
                    as_json = true;
                } else if arg == "--verbose" {
                    verbose = true;
                }
                i += 1;
            }
            Command::RouteExplain {
                config_path,
                path,
                method,
                headers,
                as_json,
                verbose,
            }
        }
        Some("-h") | Some("--help") | Some("help") => Command::Help,
        Some(other) => {
            eprintln!("Unknown command: {other}");
            Command::Help
        }
    }
}

fn print_help() {
    println!(
        "service-router commands:\n  run [config]                                       Start proxy server (default)\n  check-config [config] [--json] [--strict]          Validate config and registry setup\n  doctor [config] [--config <path>] [--probe-upstream] [--json]  Environment checks; --probe-upstream TCP-probes registry endpoints (non-mock) and route targets\n  route-explain <path> [method] [options]            Explain route match result\n    options: --config <path> --header \"key:value\" [--json] [--verbose]\n  help                                               Show help"
    );
}

async fn check_config(config_path: PathBuf, as_json: bool, strict: bool) -> anyhow::Result<ExitCode> {
    let config = load_config(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to load config from {}: {}", config_path.display(), e))?;
    service_router::routing::RouterSnapshot::from_config(&config)
        .map_err(|e| anyhow::anyhow!("Failed to compile routing rules: {e}"))?;
    let resolver = build_resolver(&config).await?;
    let strict_findings = if strict {
        run_strict_config_checks(&config)
    } else {
        Vec::new()
    };
    let strict_passed = strict_findings.is_empty();
    let summary = serde_json::json!({
        "status": "ok",
        "config_path": config_path.display().to_string(),
        "routes": config.routes.len(),
        "registries": config.registries.sources.len(),
        "resolver_empty": resolver.is_empty(),
        "strict_enabled": strict,
        "strict_passed": strict_passed,
        "strict_findings": strict_findings
    });
    if as_json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(if strict && !strict_passed {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        });
    }
    if resolver.is_empty() {
        println!(
            "check-config OK: no registries configured, direct upstream routes only ({})",
            config_path.display()
        );
    } else {
        println!(
            "check-config OK: {} registries configured, {} routes compiled ({})",
            config.registries.sources.len(),
            config.routes.len(),
            config_path.display()
        );
    }
    if strict {
        if strict_passed {
            println!("strict-check OK: no findings");
        } else {
            println!("strict-check FAIL:");
            for finding in strict_findings {
                println!(" - {}", finding);
            }
            return Ok(ExitCode::from(1));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn route_explain(
    config_path: PathBuf,
    path: String,
    method: String,
    headers: Vec<(String, String)>,
    as_json: bool,
    verbose: bool,
) -> anyhow::Result<ExitCode> {
    let config = load_config(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to load config from {}: {}", config_path.display(), e))?;
    let snapshot = service_router::routing::RouterSnapshot::from_config(&config)
        .map_err(|e| anyhow::anyhow!("Failed to compile routing rules: {e}"))?;

    let mut header_map = axum::http::HeaderMap::new();
    for (k, v) in &headers {
        if let (Ok(name), Ok(value)) = (
            axum::http::header::HeaderName::from_bytes(k.as_bytes()),
            axum::http::HeaderValue::from_str(v),
        ) {
            header_map.insert(name, value);
        }
    }

    if let Some(rule) = snapshot.resolve(&path, &method, &header_map) {
        if as_json {
            let output = serde_json::json!({
                "diagnostic_version": "1.0",
                "matched": true,
                "config_path": config_path.display().to_string(),
                "path": path,
                "method": method.to_uppercase(),
                "rule_id": rule.id,
                "priority": rule.priority,
                "target": rule.upstream_url.clone().or_else(|| rule.service_id.clone()),
                "rewritten_path": rule.rewrite_path(&path).to_string()
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(ExitCode::SUCCESS);
        }
        println!(
            "Route explain - path={}, method={}, config={}",
            path,
            method.to_uppercase(),
            config_path.display()
        );
        println!("Matched rule: {} (priority={})", rule.id, rule.priority);
        println!(
            "Target: {}",
            rule.upstream_url
                .clone()
                .or_else(|| rule.service_id.clone())
                .unwrap_or_else(|| "<missing>".to_string())
        );
        println!("Rewritten path: {}", rule.rewrite_path(&path));
        return Ok(ExitCode::SUCCESS);
    }

    let config_path_str = config_path.display().to_string();
    let mut diagnostics = Vec::new();
    let inspect_limit = if verbose { snapshot.rules.len() } else { 5 };
    for rule in snapshot.rules.iter().take(inspect_limit) {
        let (path_ok, method_ok, headers_ok, reasons, suggestions) =
            explain_rule_mismatch(rule, &path, &method, &header_map, &config_path_str);
        diagnostics.push(serde_json::json!({
            "rule_id": rule.id,
            "path": path_ok,
            "method": method_ok,
            "headers": headers_ok,
            "reasons": reasons,
            "suggestions": suggestions
        }));
    }
    let remediation_outline = merge_remediation_outline(&diagnostics);
    if as_json {
        let output = serde_json::json!({
            "diagnostic_version": "1.0",
            "matched": false,
            "config_path": config_path_str,
            "path": path,
            "method": method.to_uppercase(),
            "inspected_rules": inspect_limit,
            "remediation_outline": remediation_outline,
            "diagnostics": diagnostics
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(ExitCode::from(1));
    }
    println!(
        "Route explain - path={}, method={}, config={}",
        path,
        method.to_uppercase(),
        config_path.display()
    );
    println!(
        "No route matched. Candidate diagnostics (showing {} rule(s)):",
        inspect_limit
    );
    for item in &diagnostics {
        let reasons = item
            .get("reasons")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_else(|| "no additional reason".to_string());
        let suggestions = item
            .get("suggestions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let message = v.get("message").and_then(|m| m.as_str())?;
                        let command = v.get("command").and_then(|c| c.as_str())?;
                        Some(format!("{} (cmd: {})", message, command))
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_else(|| "no suggestion".to_string());
        println!(
            " - {}: path={}, method={}, headers={} | {} | suggestion: {}",
            item.get("rule_id").and_then(|v| v.as_str()).unwrap_or("-"),
            item.get("path").and_then(|v| v.as_bool()).unwrap_or(false),
            item.get("method").and_then(|v| v.as_bool()).unwrap_or(false),
            item.get("headers").and_then(|v| v.as_bool()).unwrap_or(false),
            reasons,
            suggestions
        );
    }
    if !remediation_outline.is_empty() {
        println!("Suggested actions (first hit per issue code):");
        for entry in &remediation_outline {
            let code = entry.get("code").and_then(|c| c.as_str()).unwrap_or("?");
            let message = entry.get("message").and_then(|m| m.as_str()).unwrap_or("");
            let command = entry.get("command").and_then(|c| c.as_str()).unwrap_or("");
            println!(" - [{}] {} | {}", code, message, command);
        }
    }
    Ok(ExitCode::from(1))
}

/// Builds a stable, de-duplicated list of suggestions (one entry per `code`) for unmatched runs.
fn merge_remediation_outline(diagnostics: &[serde_json::Value]) -> Vec<serde_json::Value> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for d in diagnostics {
        let Some(arr) = d.get("suggestions").and_then(|x| x.as_array()) else {
            continue;
        };
        for s in arr {
            let Some(code) = s.get("code").and_then(|c| c.as_str()) else {
                continue;
            };
            if seen.insert(code.to_string()) {
                out.push(s.clone());
            }
        }
    }
    out
}

fn describe_path_expectation(
    compiled: &service_router::routing::matcher::CompiledPath,
) -> String {
    use service_router::routing::matcher::CompiledPath;
    match compiled {
        CompiledPath::Exact(v) => format!("exact '{}'", v),
        CompiledPath::Prefix(v) => format!("prefix '{}' (path must start with this)", v),
        CompiledPath::Glob(p) => format!("glob '{}'", p.as_str()),
        CompiledPath::Regex(re) => format!("regex `{}`", re.as_str()),
    }
}

fn path_mismatch_action_message(
    compiled: &service_router::routing::matcher::CompiledPath,
) -> String {
    use service_router::routing::matcher::CompiledPath;
    match compiled {
        CompiledPath::Exact(v) => format!("use path '{}' or change the rule to the path you need", v),
        CompiledPath::Prefix(v) => format!(
            "ensure the path starts with '{}' or adjust the rule prefix / try a more specific rule first",
            v
        ),
        CompiledPath::Glob(p) => format!(
            "shape the path to match glob '{}' or relax/tighten the pattern in config",
            p.as_str()
        ),
        CompiledPath::Regex(re) => format!(
            "adjust the path to satisfy regex `{}` or update the pattern in config",
            re.as_str()
        ),
    }
}

fn explain_rule_mismatch(
    rule: &service_router::routing::CompiledRoutingRule,
    path: &str,
    method: &str,
    header_map: &axum::http::HeaderMap,
    config_path: &str,
) -> (bool, bool, bool, Vec<String>, Vec<serde_json::Value>) {
    let mut reasons = Vec::new();
    let mut suggestions: Vec<serde_json::Value> = Vec::new();
    let path_ok = rule.compiled_path.matches(path);
    if !path_ok {
        let expectation = describe_path_expectation(&rule.compiled_path);
        reasons.push(format!(
            "path '{}' does not match rule ({})",
            path, expectation
        ));
        suggestions.push(serde_json::json!({
            "code": "PATH_MISMATCH",
            "message": path_mismatch_action_message(&rule.compiled_path),
            "command": format!(
                "cargo run -- route-explain {} {} --config {} --verbose",
                path,
                method.to_uppercase(),
                config_path
            )
        }));
    }
    let method_ok = rule
        .methods
        .as_ref()
        .map(|ms| ms.iter().any(|m| m.eq_ignore_ascii_case(method)))
        .unwrap_or(true);
    if !method_ok {
        let allowed = rule
            .methods
            .as_ref()
            .map(|m| m.join(","))
            .unwrap_or_else(|| "*".to_string());
        reasons.push(format!("method '{}' not in [{}]", method.to_uppercase(), allowed));
        let sample_method = rule
            .methods
            .as_ref()
            .and_then(|ms| ms.first())
            .map(|m| m.to_uppercase())
            .unwrap_or_else(|| "GET".to_string());
        suggestions.push(serde_json::json!({
            "code": "METHOD_MISMATCH",
            "message": format!(
                "call with one of [{}] or add '{}' to rule.methods",
                allowed,
                method.to_uppercase()
            ),
            "command": format!(
                "cargo run -- route-explain {} {} --config {}",
                path, sample_method, config_path
            )
        }));
    }
    let headers_ok = rule
        .headers
        .as_ref()
        .map(|hs| {
            let mut ok = true;
            for (name, expected) in hs {
                let name = match axum::http::header::HeaderName::from_bytes(name.as_bytes()) {
                    Ok(n) => n,
                    Err(_) => {
                        reasons.push(format!("invalid rule header name '{}'", name));
                        suggestions.push(serde_json::json!({
                            "code": "RULE_HEADER_NAME_INVALID",
                            "message": "header names must be valid HTTP tokens; fix the key in YAML for this rule",
                            "command": format!("edit route '{}' headers: replace invalid key '{}'", rule.id, name)
                        }));
                        ok = false;
                        continue;
                    }
                };
                match header_map.get(&name).and_then(|v| v.to_str().ok()) {
                    Some(actual) if actual == expected => {}
                    Some(actual) => {
                        reasons.push(format!(
                            "header '{}' mismatch: expected '{}', got '{}'",
                            name, expected, actual
                        ));
                        suggestions.push(serde_json::json!({
                            "code": "HEADER_VALUE_MISMATCH",
                            "message": format!("set header '{}' to '{}' or update rule condition", name, expected),
                            "command": format!(
                                "cargo run -- route-explain {} {} --config {} --header \"{}:{}\"",
                                path, method.to_uppercase(), config_path, name, expected
                            )
                        }));
                        ok = false;
                    }
                    None => {
                        reasons.push(format!("missing required header '{}'", name));
                        suggestions.push(serde_json::json!({
                            "code": "HEADER_MISSING",
                            "message": format!("add required header '{}' with expected value '{}'", name, expected),
                            "command": format!(
                                "cargo run -- route-explain {} {} --config {} --header \"{}:{}\"",
                                path, method.to_uppercase(), config_path, name, expected
                            )
                        }));
                        ok = false;
                    }
                }
            }
            ok
        })
        .unwrap_or(true);

    (path_ok, method_ok, headers_ok, reasons, suggestions)
}

fn run_strict_config_checks(config: &service_router::config::AppConfig) -> Vec<String> {
    let mut findings = Vec::new();

    if config.routes.is_empty() {
        findings.push("routes list is empty".to_string());
    }

    let mut id_count: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for rule in &config.routes {
        *id_count.entry(rule.id.as_str()).or_insert(0) += 1;
    }
    for (id, count) in id_count {
        if count > 1 {
            findings.push(format!("duplicate route id '{}' appears {} times", id, count));
        }
    }

    for (i, left) in config.routes.iter().enumerate() {
        for right in config.routes.iter().skip(i + 1) {
            let same_matcher = format!("{:?}", left.path) == format!("{:?}", right.path)
                && left.methods == right.methods
                && left.headers == right.headers;
            if same_matcher {
                findings.push(format!(
                    "rules '{}' and '{}' have identical match conditions",
                    left.id, right.id
                ));
            }
        }
    }

    // Shadows / unreachable overlaps: iterate in router evaluation order (stable sort by
    // priority, ties keep YAML declaration order) per `RouterSnapshot::from_config`.
    let indices = routing_evaluation_order_indices(&config.routes);
    for ei in 0..indices.len() {
        let left_idx = indices[ei];
        let left = &config.routes[left_idx];
        for &right_idx in indices.iter().skip(ei + 1) {
            let right = &config.routes[right_idx];
            if method_constraints_cover(left.methods.as_ref(), right.methods.as_ref())
                && header_constraints_cover(left.headers.as_ref(), right.headers.as_ref())
                && path_matcher_covers(&left.path, &right.path)
            {
                findings.push(format!(
                    "rule '{}' is evaluated before '{}' and covers its path; overlapping requests cannot reach '{}'",
                    left.id, right.id, right.id
                ));
            }
        }
    }

    use service_router::config::model::PathMatcher;

    // Redundant downstream fields: resolver prefers `upstream_url` (see handlers); `service_id` is ignored then.
    for rule in &config.routes {
        if rule.upstream_url.is_some() && rule.service_id.is_some() {
            findings.push(format!(
                "rule '{}' sets both upstream_url and service_id (upstream wins; registry lookup is unreachable)",
                rule.id
            ));
        }
        // Prefix route with strip_prefix that never applies to matched paths (`strip_prefix(&path)` misses when path starts with matcher prefix).
        match (&rule.path, &rule.strip_prefix) {
            (PathMatcher::Prefix { value: p }, Some(strip)) if !strip.is_empty() && p.as_str() != "/" => {
                if !strip_prefix_applies_to_matched_requests(p.as_str(), strip.as_str()) {
                    findings.push(format!(
                        "rule '{}' strip_prefix '{}' never applies (prefix matcher '{}') — path matches never begin with '{}'",
                        rule.id,
                        strip,
                        p,
                        strip
                    ));
                }
            }
            _ => {}
        }
    }

    findings
}

/// Indices `0..routes.len()` sorted like [`service_router::routing::RouterSnapshot`]: ascending `priority`,
/// preserving declaration order for ties (`sort_by_key` is stable).
fn routing_evaluation_order_indices(
    routes: &[service_router::config::model::RoutingRule],
) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..routes.len()).collect();
    indices.sort_by_key(|&i| routes[i].priority);
    indices
}

/// True iff some request path admitted by Prefix `matcher_prefix` is also prefixed by `strip`
/// (`CompiledRoutingRule::rewrite_path`).
fn strip_prefix_applies_to_matched_requests(matcher_prefix: &str, strip: &str) -> bool {
    matcher_prefix.starts_with(strip)
}

fn path_matcher_covers(
    left: &service_router::config::model::PathMatcher,
    right: &service_router::config::model::PathMatcher,
) -> bool {
    use service_router::config::model::PathMatcher;
    match (left, right) {
        (PathMatcher::Prefix { value: lp }, PathMatcher::Prefix { value: rp }) => rp.starts_with(lp),
        (PathMatcher::Prefix { value: lp }, PathMatcher::Exact { value: re }) => re.starts_with(lp),
        (PathMatcher::Exact { value: le }, PathMatcher::Exact { value: re }) => le == re,
        _ => false,
    }
}

fn method_constraints_cover(left: Option<&Vec<String>>, right: Option<&Vec<String>>) -> bool {
    match (left, right) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(l), Some(r)) => r
            .iter()
            .all(|rm| l.iter().any(|lm| lm.eq_ignore_ascii_case(rm))),
    }
}

fn header_constraints_cover(
    left: Option<&std::collections::HashMap<String, String>>,
    right: Option<&std::collections::HashMap<String, String>>,
) -> bool {
    match (left, right) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(l), Some(r)) => r.iter().all(|(rk, rv)| l.get(rk) == Some(rv)),
    }
}

async fn doctor(config_path: PathBuf, probe_upstream: bool, as_json: bool) -> anyhow::Result<ExitCode> {
    if !as_json {
        println!("Doctor checks for {}", config_path.display());
    }

    if !config_path.exists() {
        if as_json {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "diagnostic_version": "1.0",
                "status": "fail",
                "config_path": config_path.display().to_string(),
                "error": "config file not found"
            }))?);
        } else {
            println!(" - config file: FAIL (not found)");
        }
        return Ok(ExitCode::from(1));
    }
    if !as_json {
        println!(" - config file: OK");
    }

    let config = match load_config(&config_path) {
        Ok(c) => {
            if !as_json {
                println!(" - config parse: OK");
            }
            c
        }
        Err(e) => {
            if as_json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "diagnostic_version": "1.0",
                    "status": "fail",
                    "config_path": config_path.display().to_string(),
                    "error": format!("config parse failed: {e}")
                }))?);
            } else {
                println!(" - config parse: FAIL ({e})");
            }
            return Ok(ExitCode::from(1));
        }
    };

    match StdTcpListener::bind(format!("{}:{}", config.server.host, config.server.port)) {
        Ok(listener) => {
            drop(listener);
            if !as_json {
                println!(" - listen addr: OK ({}:{})", config.server.host, config.server.port);
            }
        }
        Err(e) => {
            if as_json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "diagnostic_version": "1.0",
                    "status": "fail",
                    "config_path": config_path.display().to_string(),
                    "error": format!("listen addr unavailable: {e}")
                }))?);
            } else {
                println!(
                    " - listen addr: FAIL ({}:{} unavailable: {})",
                    config.server.host, config.server.port, e
                );
            }
            return Ok(ExitCode::from(1));
        }
    }

    let resolver = match build_resolver(&config).await {
        Ok(resolver) => {
            if !as_json {
                println!(" - registry init: OK");
            }
            resolver
        }
        Err(e) => {
            if as_json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "diagnostic_version": "1.0",
                    "status": "fail",
                    "config_path": config_path.display().to_string(),
                    "error": format!("registry init failed: {e}")
                }))?);
            } else {
                println!(" - registry init: FAIL ({e})");
                println!("Doctor result: FAIL");
            }
            return Ok(ExitCode::from(1));
        }
    };

    let mut has_unhealthy = false;
    let report = resolver.health_report().await;
    let mut registry_health_json: Vec<serde_json::Value> = Vec::new();
    if report.is_empty() {
        if !as_json {
            println!(" - registry health: SKIP (no registry configured)");
        }
    } else {
        if !as_json {
            println!(" - registry health:");
        }
        for (priority, kind, health) in report {
            match health {
                service_router::registry::RegistryHealth::Healthy => {
                    registry_health_json.push(serde_json::json!({"priority": priority, "kind": kind, "status": "healthy"}));
                    if !as_json {
                        println!("   - [{}] {}: healthy", priority, kind);
                    }
                }
                service_router::registry::RegistryHealth::Degraded(msg) => {
                    registry_health_json.push(serde_json::json!({"priority": priority, "kind": kind, "status": "degraded", "message": msg}));
                    if !as_json {
                        println!("   - [{}] {}: degraded ({})", priority, kind, msg);
                    }
                }
                service_router::registry::RegistryHealth::Unhealthy(msg) => {
                    has_unhealthy = true;
                    registry_health_json.push(serde_json::json!({"priority": priority, "kind": kind, "status": "unhealthy", "message": msg}));
                    if !as_json {
                        println!("   - [{}] {}: unhealthy ({})", priority, kind, msg);
                    }
                }
            }
        }
    }

    let mut upstream_probe_json: Vec<serde_json::Value> = Vec::new();
    let mut registry_endpoint_probe_json: Vec<serde_json::Value> = Vec::new();
    let mut probe_failures = 0usize;
    if probe_upstream {
        use service_router::config::model::RegistryConfig;

        let has_remote_registry = config
            .registries
            .sources
            .iter()
            .any(|s| !matches!(s, RegistryConfig::Mock(_)));

        if has_remote_registry {
            if !as_json {
                println!(" - registry endpoint probe:");
            }
            for src in &config.registries.sources {
                let (kind, priority, target_parse) = match src {
                    RegistryConfig::Nacos(c) => (
                        "Nacos",
                        c.priority,
                        parse_host_port_for_probe(&c.server_addr).map(|x| (c.server_addr.clone(), x)),
                    ),
                    RegistryConfig::Eureka(c) => (
                        "Eureka",
                        c.priority,
                        parse_host_port_for_probe(&c.server_url).map(|x| (c.server_url.clone(), x)),
                    ),
                    RegistryConfig::Kubernetes(c) => (
                        "Kubernetes",
                        c.priority,
                        parse_host_port_for_probe(&c.api_server_url).map(|x| (c.api_server_url.clone(), x)),
                    ),
                    RegistryConfig::Mock(_) => continue,
                };
                match target_parse {
                    Ok((configured, (host, port))) => {
                        let reachable = probe_tcp(&host, port).await;
                        if !reachable {
                            probe_failures += 1;
                        }
                        let mut entry = serde_json::json!({
                            "kind": kind,
                            "priority": priority,
                            "configured": configured,
                            "host": host,
                            "port": port,
                            "reachable": reachable
                        });
                        if !reachable {
                            entry["failure_code"] = serde_json::json!("TCP_UNREACHABLE");
                            entry["reason"] = serde_json::json!(format!(
                                "TCP connect to {}:{} failed or timed out (2s)",
                                host, port
                            ));
                        }
                        registry_endpoint_probe_json.push(entry);
                        if !as_json {
                            if reachable {
                                println!(
                                    "   - [{}] {} {} ({}:{}): reachable",
                                    priority, kind, configured, host, port
                                );
                            } else {
                                println!(
                                    "   - [{}] {} {} ({}:{}): unreachable (TCP_UNREACHABLE)",
                                    priority, kind, configured, host, port
                                );
                            }
                        }
                    }
                    Err(e) => {
                        probe_failures += 1;
                        registry_endpoint_probe_json.push(serde_json::json!({
                            "kind": kind,
                            "priority": priority,
                            "reachable": false,
                            "failure_code": "ENDPOINT_PARSE_ERROR",
                            "reason": e.to_string()
                        }));
                        if !as_json {
                            println!(
                                "   - [{}] {} endpoint parse failed: {} (ENDPOINT_PARSE_ERROR)",
                                priority, kind, e
                            );
                        }
                    }
                }
            }
        } else if !as_json {
            println!(" - registry endpoint probe: SKIP (mock registry only)");
        }

        if !as_json {
            println!(" - upstream probe:");
        }
        for route in &config.routes {
            if let Some(url) = &route.upstream_url {
                match parse_host_port_from_url(url) {
                    Ok((host, port)) => {
                        let reachable = probe_tcp(&host, port).await;
                        upstream_probe_json.push(serde_json::json!({
                            "route_id": route.id,
                            "target_type": "upstream_url",
                            "host": host,
                            "port": port,
                            "reachable": reachable
                        }));
                        if reachable {
                            if !as_json {
                                println!("   - route {} direct {}:{} reachable", route.id, host, port);
                            }
                        } else {
                            probe_failures += 1;
                            if !as_json {
                                println!("   - route {} direct {}:{} unreachable", route.id, host, port);
                            }
                        }
                    }
                    Err(e) => {
                        probe_failures += 1;
                        upstream_probe_json.push(serde_json::json!({
                            "route_id": route.id,
                            "target_type": "upstream_url",
                            "reachable": false,
                            "error": e.to_string()
                        }));
                        if !as_json {
                            println!("   - route {} direct URL parse failed: {}", route.id, e);
                        }
                    }
                }
            } else if let Some(service_id) = &route.service_id {
                match resolver.resolve(service_id).await {
                    Ok(instances) if instances.is_empty() => {
                        probe_failures += 1;
                        upstream_probe_json.push(serde_json::json!({
                            "route_id": route.id,
                            "target_type": "service_id",
                            "service_id": service_id,
                            "reachable": false,
                            "error": "resolved 0 instances"
                        }));
                        if !as_json {
                            println!("   - route {} service {} resolved 0 instances", route.id, service_id);
                        }
                    }
                    Ok(instances) => {
                        let mut ok_any = false;
                        for inst in instances.iter().take(3) {
                            if probe_tcp(&inst.host, inst.port).await {
                                ok_any = true;
                                if !as_json {
                                    println!(
                                        "   - route {} service {} instance {}:{} reachable",
                                        route.id, service_id, inst.host, inst.port
                                    );
                                }
                                break;
                            }
                        }
                        upstream_probe_json.push(serde_json::json!({
                            "route_id": route.id,
                            "target_type": "service_id",
                            "service_id": service_id,
                            "resolved_instances": instances.len(),
                            "reachable": ok_any
                        }));
                        if !ok_any {
                            probe_failures += 1;
                            if !as_json {
                                println!(
                                    "   - route {} service {} unresolved reachable instances",
                                    route.id, service_id
                                );
                            }
                        }
                    }
                    Err(e) => {
                        probe_failures += 1;
                        upstream_probe_json.push(serde_json::json!({
                            "route_id": route.id,
                            "target_type": "service_id",
                            "service_id": service_id,
                            "reachable": false,
                            "error": e.to_string()
                        }));
                        if !as_json {
                            println!(
                                "   - route {} service {} resolve failed: {}",
                                route.id, service_id, e
                            );
                        }
                    }
                }
            }
        }
        if probe_failures > 0 {
            if !as_json {
                println!(
                    " - network probe result: FAIL ({} issue(s); registry endpoints + route upstreams)",
                    probe_failures
                );
            }
            has_unhealthy = true;
        } else if !as_json {
            println!(" - network probe result: PASS");
        }
    }

    if as_json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "diagnostic_version": "1.0",
            "status": if has_unhealthy { "fail" } else { "pass" },
            "config_path": config_path.display().to_string(),
            "probe_upstream_enabled": probe_upstream,
            "registry_health": registry_health_json,
            "registry_endpoint_probe": registry_endpoint_probe_json,
            "upstream_probe": upstream_probe_json
        }))?);
    }

    if has_unhealthy {
        if !as_json {
            println!("Doctor result: FAIL");
        }
        Ok(ExitCode::from(1))
    } else {
        if !as_json {
            println!("Doctor result: PASS");
        }
        Ok(ExitCode::SUCCESS)
    }
}

fn parse_host_port_from_url(url: &str) -> anyhow::Result<(String, u16)> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| anyhow::anyhow!("invalid URL '{}': {}", url, e))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("missing host in URL '{}'", url))?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("missing port and scheme default for '{}'", url))?;
    Ok((host, port))
}

/// Host/port for TCP probes: full URL with scheme, or `host:port`.
fn parse_host_port_for_probe(addr_or_url: &str) -> anyhow::Result<(String, u16)> {
    let t = addr_or_url.trim();
    if t.contains("://") {
        parse_host_port_from_url(t)
    } else {
        let colon = t
            .rfind(':')
            .ok_or_else(|| anyhow::anyhow!("expected host:port, got {:?}", t))?;
        let host = t[..colon].trim();
        if host.is_empty() {
            anyhow::bail!("empty host in {:?}", t);
        }
        let port: u16 = t[colon + 1..]
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid port in {:?}: {e}", t))?;
        Ok((host.to_string(), port))
    }
}

async fn probe_tcp(host: &str, port: u16) -> bool {
    let addr = format!("{}:{}", host, port);
    matches!(
        tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            tokio::net::TcpStream::connect(addr)
        )
        .await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use service_router::config::model::{
        AppConfig, PathMatcher, RegistriesConfig, RoutingRule, ServerConfig,
    };
    use service_router::routing::CompiledRoutingRule;

    use super::{
        explain_rule_mismatch, merge_remediation_outline, parse_host_port_for_probe,
        run_strict_config_checks,
    };

    #[test]
    fn strict_check_reports_duplicate_route_ids() {
        let config = AppConfig {
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes: vec![
                RoutingRule {
                    id: "dup".to_string(),
                    path: PathMatcher::Prefix {
                        value: "/a".to_string(),
                    },
                    methods: None,
                    headers: None,
                    service_id: Some("svc-a".to_string()),
                    upstream_url: None,
                    strip_prefix: None,
                    priority: 10,
                },
                RoutingRule {
                    id: "dup".to_string(),
                    path: PathMatcher::Prefix {
                        value: "/b".to_string(),
                    },
                    methods: None,
                    headers: None,
                    service_id: Some("svc-b".to_string()),
                    upstream_url: None,
                    strip_prefix: None,
                    priority: 20,
                },
            ],
            log_level: "info".to_string(),
        };
        let findings = run_strict_config_checks(&config);
        assert!(findings.iter().any(|f| f.contains("duplicate route id 'dup'")));
    }

    #[test]
    fn strict_check_reports_catch_all_shadowing() {
        let config = AppConfig {
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes: vec![
                RoutingRule {
                    id: "catch-all".to_string(),
                    path: PathMatcher::Prefix {
                        value: "/".to_string(),
                    },
                    methods: Some(vec!["GET".to_string()]),
                    headers: None,
                    service_id: Some("svc-all".to_string()),
                    upstream_url: None,
                    strip_prefix: None,
                    priority: 1,
                },
                RoutingRule {
                    id: "orders".to_string(),
                    path: PathMatcher::Prefix {
                        value: "/api/orders".to_string(),
                    },
                    methods: Some(vec!["GET".to_string()]),
                    headers: None,
                    service_id: Some("svc-orders".to_string()),
                    upstream_url: None,
                    strip_prefix: None,
                    priority: 10,
                },
            ],
            log_level: "info".to_string(),
        };
        let findings = run_strict_config_checks(&config);
        assert!(findings
            .iter()
            .any(|f| f.contains("evaluated before") || f.contains("covers its path")));
    }

    #[test]
    fn strict_check_priority_order_masks_narrow_when_broad_runs_first() {
        let config = AppConfig {
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes: vec![
                RoutingRule {
                    id: "detail".to_string(),
                    path: PathMatcher::Prefix {
                        value: "/api/item".to_string(),
                    },
                    methods: None,
                    headers: None,
                    service_id: Some("svc-detail".to_string()),
                    upstream_url: None,
                    strip_prefix: None,
                    priority: 80,
                },
                RoutingRule {
                    id: "site".to_string(),
                    path: PathMatcher::Prefix {
                        value: "/".to_string(),
                    },
                    methods: None,
                    headers: None,
                    service_id: Some("svc-site".to_string()),
                    upstream_url: None,
                    strip_prefix: None,
                    priority: 40,
                },
            ],
            log_level: "info".to_string(),
        };
        let findings = run_strict_config_checks(&config);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("'site'") && f.contains("'detail'") && f.contains("evaluated before"))
        );
    }

    #[test]
    fn strict_check_reports_upstream_plus_service_ambiguity() {
        let config = AppConfig {
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes: vec![RoutingRule {
                id: "dup-target".to_string(),
                path: PathMatcher::Prefix {
                    value: "/hook".to_string(),
                },
                methods: None,
                headers: None,
                service_id: Some("ignored-registry".to_string()),
                upstream_url: Some("http://127.0.0.1:9090".to_string()),
                strip_prefix: None,
                priority: 10,
            }],
            log_level: "info".to_string(),
        };
        let findings = run_strict_config_checks(&config);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("upstream_url") && f.contains("service_id"))
        );
    }

    #[test]
    fn strict_check_reports_strip_prefix_never_applies() {
        let config = AppConfig {
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes: vec![RoutingRule {
                id: "bad-strip".to_string(),
                path: PathMatcher::Prefix {
                    value: "/api".to_string(),
                },
                methods: None,
                headers: None,
                service_id: Some("svc".to_string()),
                upstream_url: None,
                strip_prefix: Some("/nope".to_string()),
                priority: 10,
            }],
            log_level: "info".to_string(),
        };
        let findings = run_strict_config_checks(&config);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("strip_prefix") && f.contains("never applies") && f.contains("bad-strip"))
        );
    }

    #[test]
    fn explain_rule_mismatch_reports_method_and_header_reason() {
        let rule = RoutingRule {
            id: "test-rule".to_string(),
            path: PathMatcher::Prefix {
                value: "/api".to_string(),
            },
            methods: Some(vec!["GET".to_string()]),
            headers: Some(HashMap::from([("x-env".to_string(), "dev".to_string())])),
            service_id: Some("svc".to_string()),
            upstream_url: None,
            strip_prefix: None,
            priority: 10,
        };
        let compiled = CompiledRoutingRule::compile(&rule).expect("compile rule");
        let header_map = axum::http::HeaderMap::new();
        let cfg = "config/dev-routes.yaml";
        let (_path_ok, method_ok, headers_ok, reasons, suggestions) =
            explain_rule_mismatch(&compiled, "/api/orders", "POST", &header_map, cfg);
        assert!(!method_ok);
        assert!(!headers_ok);
        assert!(reasons.iter().any(|r| r.contains("method 'POST' not in")));
        assert!(reasons.iter().any(|r| r.contains("missing required header")));
        assert!(suggestions.iter().any(|s| {
            s.get("code")
                .and_then(|v| v.as_str())
                .map(|c| c == "METHOD_MISMATCH")
                .unwrap_or(false)
        }));
        assert!(suggestions.iter().any(|s| {
            s.get("command")
                .and_then(|v| v.as_str())
                .map(|c| c.contains(cfg) && c.contains("--config"))
                .unwrap_or(false)
        }));
    }

    #[test]
    fn explain_path_reason_includes_prefix_expectation() {
        let rule = RoutingRule {
            id: "pfx".to_string(),
            path: PathMatcher::Prefix {
                value: "/shop".to_string(),
            },
            methods: None,
            headers: None,
            service_id: Some("svc".to_string()),
            upstream_url: None,
            strip_prefix: None,
            priority: 10,
        };
        let compiled = CompiledRoutingRule::compile(&rule).expect("compile");
        let header_map = axum::http::HeaderMap::new();
        let (path_ok, _m, _h, reasons, suggestions) =
            explain_rule_mismatch(&compiled, "/other", "GET", &header_map, "c.yaml");
        assert!(!path_ok);
        assert!(reasons.iter().any(|r| r.contains("prefix")));
        assert!(reasons.iter().any(|r| r.contains("/shop")));
        assert!(suggestions.iter().any(|s| {
            s.get("code").and_then(|v| v.as_str()) == Some("PATH_MISMATCH")
        }));
    }

    #[test]
    fn explain_invalid_rule_header_name_has_remediation() {
        let rule = RoutingRule {
            id: "bad-hdr-rule".to_string(),
            path: PathMatcher::Prefix {
                value: "/".to_string(),
            },
            methods: None,
            headers: Some(HashMap::from([("not a token".to_string(), "v".to_string())])),
            service_id: Some("svc".to_string()),
            upstream_url: None,
            strip_prefix: None,
            priority: 10,
        };
        let compiled = CompiledRoutingRule::compile(&rule).expect("compile");
        let header_map = axum::http::HeaderMap::new();
        let (_p, _m, _h, _r, suggestions) =
            explain_rule_mismatch(&compiled, "/x", "GET", &header_map, "c.yaml");
        assert!(suggestions.iter().any(|s| {
            s.get("code").and_then(|v| v.as_str()) == Some("RULE_HEADER_NAME_INVALID")
        }));
    }

    #[test]
    fn merge_remediation_outline_keeps_first_code_only() {
        let diagnostics = vec![
            serde_json::json!({"suggestions":[{"code":"PATH_MISMATCH","message":"a","command":"c1"}]}),
            serde_json::json!({"suggestions":[{"code":"PATH_MISMATCH","message":"b","command":"c2"},{"code":"METHOD_MISMATCH","message":"m","command":"c3"}]}),
        ];
        let merged = merge_remediation_outline(&diagnostics);
        assert_eq!(merged.len(), 2);
        assert_eq!(
            merged[0].get("command").and_then(|v| v.as_str()),
            Some("c1")
        );
    }

    #[test]
    fn parse_host_port_for_probe_accepts_host_colon_port() {
        let (h, p) = parse_host_port_for_probe("127.0.0.1:8848").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 8848);
    }

    #[test]
    fn parse_host_port_for_probe_accepts_http_url() {
        let (h, p) = parse_host_port_for_probe("http://example.com:8080/v1/api").unwrap();
        assert_eq!(h, "example.com");
        assert_eq!(p, 8080);
    }
}
