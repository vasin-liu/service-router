use axum::{
    extract::{Request, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use dashmap::DashMap;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{debug, info};

use crate::config::model::InstanceSelection;
use crate::error::ProxyError;
use crate::proxy::{http_proxy, ws_proxy};
use crate::registry::{any_registry_operational, registry_health_json_row, ServiceInstance};
use crate::server::metrics::{
    failure_code_for_proxy, failure_code_for_registry, render_prometheus,
};
use crate::server::state::AppState;

/// Main proxy handler — routes every incoming request through the routing
/// rules and forwards to the appropriate upstream (registry-discovered or
/// direct URL).
pub async fn proxy_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response, ProxyError> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // Detect WebSocket upgrade early so we can hand it off to the WS handler.
    let is_upgrade = req
        .headers()
        .get(axum::http::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    // Resolve the routing rule.
    let router_snapshot = state.router.load();
    let rule = match router_snapshot.resolve(&path, method.as_str(), req.headers()) {
        Some(r) => r,
        None => {
            debug!(path = %path, "No matching route found");
            state.metrics.record_failure("no_matching_route");
            return Err(ProxyError::NoInstances(path.clone()));
        }
    };

    state.metrics.record_route_hit(&rule.id);

    // Determine the base URL of the upstream.
    let upstream_base = if let Some(url) = &rule.upstream_url {
        url.clone()
    } else if let Some(svc_id) = &rule.service_id {
        let resolver = state.resolver.load();
        let instances = match resolver.resolve(svc_id.as_str()).await {
            Ok(i) => i,
            Err(e) => {
                state.metrics.record_failure(failure_code_for_registry(&e));
                return Err(e.into());
            }
        };
        if instances.is_empty() {
            state.metrics.record_failure("no_instances");
            return Err(ProxyError::NoInstances(svc_id.clone()));
        }
        let selection = state.config.load().server.instance_selection;
        select_service_instance(
            &instances,
            svc_id.as_str(),
            selection,
            &state.instance_rr_counters,
        )
        .base_url()
    } else {
        state.metrics.record_failure("no_instances");
        return Err(ProxyError::NoInstances(path.clone()));
    };

    // Rewrite path (strip prefix if configured).
    let rewritten_path = rule.rewrite_path(&path);

    debug!(
        rule_id = %rule.id,
        path = %path,
        "Matched routing rule"
    );

    let res = if is_upgrade {
        info!(
            upstream = %upstream_base,
            path = %rewritten_path,
            "Proxying WebSocket connection"
        );
        ws_proxy::proxy_websocket(req, &upstream_base, &rewritten_path).await
    } else {
        info!(
            method = %method,
            upstream = %upstream_base,
            path = %rewritten_path,
            "Proxying HTTP request"
        );
        http_proxy::proxy_http(
            req,
            &state.http_client,
            &upstream_base,
            &rewritten_path,
            rule.response_headers.as_deref(),
        )
        .await
    };
    res.map_err(|e| {
        state.metrics.record_failure(failure_code_for_proxy(&e));
        e
    })
}

/// `/health` — liveness probe.
pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// `/ready` — readiness probe; aggregates registry health (same row shape as `doctor --json`).
///
/// Returns 503 when every configured registry reports [`crate::registry::RegistryHealth::Unhealthy`].
pub async fn ready_handler(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    if config.registries.sources.is_empty() {
        // No registries configured — still considered ready (direct URL routing works).
        return (StatusCode::OK, Json(json!({ "status": "ready", "registries": 0 })));
    }
    let resolver = state.resolver.load();
    let report = resolver.health_report().await;
    let registry_health: Vec<serde_json::Value> = report
        .iter()
        .map(|(p, k, h)| registry_health_json_row(*p, k, h))
        .collect();
    let n = registry_health.len();
    if any_registry_operational(&report) {
        (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "registries": n,
                "registry_health": registry_health
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "reason": "all_registries_unhealthy",
                "registries": n,
                "registry_health": registry_health
            })),
        )
    }
}

/// Minimal JSON counters: route hits and failure reasons (B08).
pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(state.metrics.snapshot()))
}

/// Prometheus-compatible text exposition for proxy counters.
pub async fn metrics_prometheus_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    let body = render_prometheus(&state.metrics.snapshot());
    (StatusCode::OK, headers, body)
}

fn select_service_instance<'a>(
    instances: &'a [ServiceInstance],
    svc_id: &str,
    selection: InstanceSelection,
    rr: &DashMap<String, AtomicUsize>,
) -> &'a ServiceInstance {
    debug_assert!(!instances.is_empty());
    match selection {
        InstanceSelection::First => instances.first().expect("non-empty instances"),
        InstanceSelection::RoundRobin => {
            let idx = rr
                .entry(svc_id.to_string())
                .or_insert_with(|| AtomicUsize::new(0))
                .fetch_add(1, Ordering::Relaxed);
            &instances[idx % instances.len()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_swap::ArcSwap;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::config::model::{
        AppConfig, PathMatcher, RegistriesConfig, RoutingRule, ServerConfig,
    };
    use crate::registry::MultiRegistryResolver;
    use crate::routing::RouterSnapshot;
    use crate::server::ProxyMetrics;

    #[test]
    fn round_robin_rotates_per_service_id() {
        let instances = vec![
            ServiceInstance {
                host: "10.0.0.1".into(),
                port: 1,
                metadata: HashMap::new(),
            },
            ServiceInstance {
                host: "10.0.0.2".into(),
                port: 2,
                metadata: HashMap::new(),
            },
        ];
        let m = DashMap::new();
        assert_eq!(
            select_service_instance(
                &instances,
                "svc",
                InstanceSelection::RoundRobin,
                &m
            )
            .host,
            "10.0.0.1"
        );
        assert_eq!(
            select_service_instance(
                &instances,
                "svc",
                InstanceSelection::RoundRobin,
                &m
            )
            .host,
            "10.0.0.2"
        );
        assert_eq!(
            select_service_instance(
                &instances,
                "svc",
                InstanceSelection::RoundRobin,
                &m
            )
            .host,
            "10.0.0.1"
        );
    }

    #[test]
    fn first_is_stable() {
        let instances = vec![
            ServiceInstance {
                host: "a".into(),
                port: 1,
                metadata: HashMap::new(),
            },
            ServiceInstance {
                host: "b".into(),
                port: 2,
                metadata: HashMap::new(),
            },
        ];
        let m = DashMap::new();
        assert_eq!(
            select_service_instance(&instances, "svc", InstanceSelection::First, &m).host,
            "a"
        );
        assert_eq!(
            select_service_instance(&instances, "svc", InstanceSelection::First, &m).host,
            "a"
        );
    }

    #[tokio::test]
    async fn proxy_handler_applies_route_response_headers_end_to_end() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let mut total = 0usize;
            loop {
                let n = stream.read(&mut buf[total..]).await.expect("read request");
                assert!(n > 0, "client closed before headers");
                total += n;
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                assert!(total < buf.len(), "request too large");
            }
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nX-Upstream: kept\r\nX-Override: upstream\r\n\r\nok",
                )
                .await
                .unwrap();
        });

        let config = AppConfig {
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes: vec![RoutingRule {
                id: "orders".to_string(),
                path: PathMatcher::Exact {
                    value: "/api/orders/123".to_string(),
                },
                methods: Some(vec!["GET".to_string()]),
                headers: None,
                service_id: None,
                upstream_url: Some(format!("http://{}", upstream_addr)),
                strip_prefix: None,
                response_headers: Some(HashMap::from([
                    ("x-added".to_string(), "from-route".to_string()),
                    ("x-override".to_string(), "from-route".to_string()),
                ])),
                priority: 10,
            }],
            log_level: "info".to_string(),
        };
        let state = AppState::new(
            Arc::new(ArcSwap::from_pointee(
                RouterSnapshot::from_config(&config).expect("router snapshot"),
            )),
            Arc::new(ArcSwap::from_pointee(MultiRegistryResolver::new(
                Vec::new(),
                config.registries.query_mode.clone(),
            ))),
            Arc::new(ArcSwap::from_pointee(config.clone())),
            config.server.upstream_timeout_secs,
            Arc::new(ProxyMetrics::default()),
        );

        let req = Request::builder()
            .method("GET")
            .uri("/api/orders/123")
            .body(axum::body::Body::empty())
            .unwrap();

        let resp = proxy_handler(State(state), req).await.expect("proxy response");
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get("x-upstream").unwrap(), "kept");
        assert_eq!(resp.headers().get("x-added").unwrap(), "from-route");
        assert_eq!(resp.headers().get("x-override").unwrap(), "from-route");
    }
}
