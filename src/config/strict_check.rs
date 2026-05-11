//! Strict configuration checks for `check-config --strict`: structured findings with stable codes.

use serde::Serialize;
use serde_json::json;

use crate::config::model::{AppConfig, PathMatcher, RoutingRule};

pub const ROUTES_EMPTY: &str = "ROUTES_EMPTY";
pub const DUPLICATE_ROUTE_ID: &str = "DUPLICATE_ROUTE_ID";
pub const IDENTICAL_MATCHERS: &str = "IDENTICAL_MATCHERS";
pub const RULE_SHADOWED: &str = "RULE_SHADOWED";
pub const UPSTREAM_AND_SERVICE_ID: &str = "UPSTREAM_AND_SERVICE_ID";
pub const STRIP_PREFIX_UNREACHABLE: &str = "STRIP_PREFIX_UNREACHABLE";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StrictFinding {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl StrictFinding {
    fn new(code: &'static str, message: String, details: Option<serde_json::Value>) -> Self {
        Self {
            code: code.to_string(),
            message,
            details,
        }
    }
}

pub fn run_strict_config_checks(config: &AppConfig) -> Vec<StrictFinding> {
    let mut findings = Vec::new();

    if config.routes.is_empty() {
        findings.push(StrictFinding::new(
            ROUTES_EMPTY,
            "routes list is empty".to_string(),
            None,
        ));
    }

    let mut id_count: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for rule in &config.routes {
        *id_count.entry(rule.id.as_str()).or_insert(0) += 1;
    }
    for (id, count) in id_count {
        if count > 1 {
            findings.push(StrictFinding::new(
                DUPLICATE_ROUTE_ID,
                format!("duplicate route id '{}' appears {} times", id, count),
                Some(json!({
                    "route_id": id,
                    "count": count
                })),
            ));
        }
    }

    for (i, left) in config.routes.iter().enumerate() {
        for right in config.routes.iter().skip(i + 1) {
            let same_matcher = format!("{:?}", left.path) == format!("{:?}", right.path)
                && left.methods == right.methods
                && left.headers == right.headers;
            if same_matcher {
                findings.push(StrictFinding::new(
                    IDENTICAL_MATCHERS,
                    format!(
                        "rules '{}' and '{}' have identical match conditions",
                        left.id, right.id
                    ),
                    Some(json!({
                        "rule_ids": [left.id.as_str(), right.id.as_str()],
                    })),
                ));
            }
        }
    }

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
                findings.push(StrictFinding::new(
                    RULE_SHADOWED,
                    format!(
                        "rule '{}' is evaluated before '{}' and covers its path; overlapping requests cannot reach '{}'",
                        left.id, right.id, right.id
                    ),
                    Some(json!({
                        "covering_rule_id": left.id.as_str(),
                        "shadowed_rule_id": right.id.as_str(),
                    })),
                ));
            }
        }
    }

    for rule in &config.routes {
        if rule.upstream_url.is_some() && rule.service_id.is_some() {
            findings.push(StrictFinding::new(
                UPSTREAM_AND_SERVICE_ID,
                format!(
                    "rule '{}' sets both upstream_url and service_id (upstream wins; registry lookup is unreachable)",
                    rule.id
                ),
                Some(json!({
                    "rule_id": rule.id.as_str(),
                })),
            ));
        }
        match (&rule.path, &rule.strip_prefix) {
            (PathMatcher::Prefix { value: p }, Some(strip))
                if !strip.is_empty() && p.as_str() != "/" =>
            {
                if !strip_prefix_applies_to_matched_requests(p.as_str(), strip.as_str()) {
                    findings.push(StrictFinding::new(
                        STRIP_PREFIX_UNREACHABLE,
                        format!(
                            "rule '{}' strip_prefix '{}' never applies (prefix matcher '{}') \u{2014} path matches never begin with '{}'",
                            rule.id, strip, p, strip
                        ),
                        Some(json!({
                            "rule_id": rule.id.as_str(),
                            "strip_prefix": strip.as_str(),
                            "prefix": p.as_str(),
                        })),
                    ));
                }
            }
            _ => {}
        }
    }

    findings
}

fn routing_evaluation_order_indices(routes: &[RoutingRule]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..routes.len()).collect();
    indices.sort_by_key(|&i| routes[i].priority);
    indices
}

fn strip_prefix_applies_to_matched_requests(matcher_prefix: &str, strip: &str) -> bool {
    matcher_prefix.starts_with(strip)
}

fn path_matcher_covers(
    left: &crate::config::model::PathMatcher,
    right: &crate::config::model::PathMatcher,
) -> bool {
    use crate::config::model::PathMatcher;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{RegistriesConfig, ServerConfig};

    fn base_config(routes: Vec<crate::config::model::RoutingRule>) -> AppConfig {
        AppConfig {
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes,
            log_level: "info".to_string(),
        }
    }

    #[test]
    fn strict_check_reports_duplicate_route_ids() {
        let config = base_config(vec![
            crate::config::model::RoutingRule {
                id: "dup".to_string(),
                path: PathMatcher::Prefix {
                    value: "/a".to_string(),
                },
                methods: None,
                headers: None,
                service_id: Some("svc-a".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 10,
            },
            crate::config::model::RoutingRule {
                id: "dup".to_string(),
                path: PathMatcher::Prefix {
                    value: "/b".to_string(),
                },
                methods: None,
                headers: None,
                service_id: Some("svc-b".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 20,
            },
        ]);
        let findings = run_strict_config_checks(&config);
        assert!(findings.iter().any(|f| {
            f.code == DUPLICATE_ROUTE_ID
                && f.details.as_ref().and_then(|d| d.get("route_id").and_then(|v| v.as_str()))
                    == Some("dup")
        }));
    }

    #[test]
    fn strict_check_reports_catch_all_shadowing() {
        let config = base_config(vec![
            crate::config::model::RoutingRule {
                id: "catch-all".to_string(),
                path: PathMatcher::Prefix {
                    value: "/".to_string(),
                },
                methods: Some(vec!["GET".to_string()]),
                headers: None,
                service_id: Some("svc-all".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 1,
            },
            crate::config::model::RoutingRule {
                id: "orders".to_string(),
                path: PathMatcher::Prefix {
                    value: "/api/orders".to_string(),
                },
                methods: Some(vec!["GET".to_string()]),
                headers: None,
                service_id: Some("svc-orders".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 10,
            },
        ]);
        let findings = run_strict_config_checks(&config);
        assert!(findings.iter().any(|f| {
            f.code == RULE_SHADOWED
                && f.message.contains("evaluated before")
                && f.details.as_ref().and_then(|d| {
                    d.get("shadowed_rule_id").and_then(|v| v.as_str())
                }) == Some("orders")
        }));
    }

    #[test]
    fn strict_check_priority_order_masks_narrow_when_broad_runs_first() {
        let config = base_config(vec![
            crate::config::model::RoutingRule {
                id: "detail".to_string(),
                path: PathMatcher::Prefix {
                    value: "/api/item".to_string(),
                },
                methods: None,
                headers: None,
                service_id: Some("svc-detail".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 80,
            },
            crate::config::model::RoutingRule {
                id: "site".to_string(),
                path: PathMatcher::Prefix {
                    value: "/".to_string(),
                },
                methods: None,
                headers: None,
                service_id: Some("svc-site".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 40,
            },
        ]);
        let findings = run_strict_config_checks(&config);
        assert!(findings.iter().any(|f| {
            f.code == RULE_SHADOWED
                && f.message.contains("'site'")
                && f.message.contains("'detail'")
                && f.message.contains("evaluated before")
        }));
    }

    #[test]
    fn strict_check_reports_upstream_plus_service_ambiguity() {
        let config = base_config(vec![crate::config::model::RoutingRule {
            id: "dup-target".to_string(),
            path: PathMatcher::Prefix {
                value: "/hook".to_string(),
            },
            methods: None,
            headers: None,
            service_id: Some("ignored-registry".to_string()),
            upstream_url: Some("http://127.0.0.1:9090".to_string()),
            strip_prefix: None,
            response_headers: None,
            priority: 10,
        }]);
        let findings = run_strict_config_checks(&config);
        assert!(findings.iter().any(|f| {
            f.code == UPSTREAM_AND_SERVICE_ID && f.message.contains("dup-target")
        }));
    }

    #[test]
    fn strict_check_reports_strip_prefix_never_applies() {
        let config = base_config(vec![crate::config::model::RoutingRule {
            id: "bad-strip".to_string(),
            path: PathMatcher::Prefix {
                value: "/api".to_string(),
            },
            methods: None,
            headers: None,
            service_id: Some("svc".to_string()),
            upstream_url: None,
            strip_prefix: Some("/nope".to_string()),
            response_headers: None,
            priority: 10,
        }]);
        let findings = run_strict_config_checks(&config);
        assert!(findings.iter().any(|f| {
            f.code == STRIP_PREFIX_UNREACHABLE
                && f.message.contains("bad-strip")
                && f.details.as_ref().and_then(|d| {
                    d.get("strip_prefix").and_then(|v| v.as_str())
                }) == Some("/nope")
        }));
    }
}
