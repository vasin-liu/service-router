use std::collections::HashMap;

use dashmap::DashMap;
use serde::Serialize;

use crate::error::{ProxyError, RegistryError};

/// Minimal proxy counters: per-rule hits and aggregated failure reasons (B08).
#[derive(Debug, Default)]
pub struct ProxyMetrics {
    route_hits: DashMap<String, u64>,
    failure_reasons: DashMap<String, u64>,
}

impl ProxyMetrics {
    /// Counts a successful route table match for `rule_id` (before upstream forwarding).
    pub fn record_route_hit(&self, rule_id: &str) {
        self.route_hits
            .entry(rule_id.to_string())
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }

    /// Counts a terminal proxy failure using a stable code (e.g. `no_matching_route`).
    pub fn record_failure(&self, reason_code: &str) {
        self.failure_reasons
            .entry(reason_code.to_string())
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }

    /// Returns a snapshot suitable for JSON and tests.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            route_hits: self
                .route_hits
                .iter()
                .map(|e| (e.key().clone(), *e.value()))
                .collect(),
            failure_reasons: self
                .failure_reasons
                .iter()
                .map(|e| (e.key().clone(), *e.value()))
                .collect(),
        }
    }
}

/// Stable code for [`RegistryError`] (proxy metrics / JSON).
pub fn failure_code_for_registry(err: &RegistryError) -> &'static str {
    match err {
        RegistryError::Http(_) => "registry_http",
        RegistryError::UnexpectedResponse(_) => "registry_unexpected",
        RegistryError::ServiceNotFound(_) => "registry_not_found",
        RegistryError::AuthFailed => "registry_auth_failed",
        RegistryError::AllFailed(_) => "registry_all_failed",
    }
}

/// Stable code for [`ProxyError`] (proxy metrics / JSON).
pub fn failure_code_for_proxy(err: &ProxyError) -> &'static str {
    match err {
        ProxyError::NoInstances(_) => "no_instances",
        ProxyError::Registry(e) => failure_code_for_registry(e),
        ProxyError::UpstreamConnection(_) => "upstream_connection",
        ProxyError::WsUpgrade(_) => "ws_upgrade",
        ProxyError::BodyRead(_) => "body_read",
    }
}

/// Point-in-time copy of [`ProxyMetrics`] for HTTP/JSON export.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub route_hits: HashMap<String, u64>,
    pub failure_reasons: HashMap<String, u64>,
}

fn escape_label_value(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Render a Prometheus text exposition (v0.0.4) from the snapshot.
pub fn render_prometheus(snapshot: &MetricsSnapshot) -> String {
    let mut out = String::new();
    out.push_str("# HELP service_router_route_hits_total Total matched requests by route id.\n");
    out.push_str("# TYPE service_router_route_hits_total counter\n");
    let mut route_keys: Vec<_> = snapshot.route_hits.keys().cloned().collect();
    route_keys.sort();
    for route_id in route_keys {
        if let Some(v) = snapshot.route_hits.get(&route_id) {
            out.push_str(&format!(
                "service_router_route_hits_total{{route_id=\"{}\"}} {}\n",
                escape_label_value(&route_id),
                v
            ));
        }
    }

    out.push_str("# HELP service_router_failures_total Total proxy failures by reason code.\n");
    out.push_str("# TYPE service_router_failures_total counter\n");
    let mut reason_keys: Vec<_> = snapshot.failure_reasons.keys().cloned().collect();
    reason_keys.sort();
    for reason in reason_keys {
        if let Some(v) = snapshot.failure_reasons.get(&reason) {
            out.push_str(&format!(
                "service_router_failures_total{{reason=\"{}\"}} {}\n",
                escape_label_value(&reason),
                v
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RegistryError;

    #[test]
    fn route_hits_and_failures_increment() {
        let m = ProxyMetrics::default();
        m.record_route_hit("r1");
        m.record_route_hit("r1");
        m.record_failure("no_matching_route");
        let s = m.snapshot();
        assert_eq!(s.route_hits.get("r1"), Some(&2));
        assert_eq!(s.failure_reasons.get("no_matching_route"), Some(&1));
    }

    #[test]
    fn failure_codes_stable() {
        assert_eq!(
            failure_code_for_registry(&RegistryError::AuthFailed),
            "registry_auth_failed"
        );
        assert_eq!(
            failure_code_for_proxy(&ProxyError::NoInstances("x".into())),
            "no_instances"
        );
    }

    #[test]
    fn renders_prometheus_metrics() {
        let m = ProxyMetrics::default();
        m.record_route_hit("orders-api");
        m.record_failure("no_matching_route");
        let txt = render_prometheus(&m.snapshot());
        assert!(txt.contains("service_router_route_hits_total"));
        assert!(txt.contains("route_id=\"orders-api\""));
        assert!(txt.contains("service_router_failures_total"));
        assert!(txt.contains("reason=\"no_matching_route\""));
    }
}
