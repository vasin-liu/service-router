use axum::{
    extract::{Request, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::{debug, info};

use crate::error::ProxyError;
use crate::proxy::{http_proxy, ws_proxy};
use crate::registry::{any_registry_operational, registry_health_json_row};
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
        instances
            .first()
            .expect("non-empty instances")
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
        http_proxy::proxy_http(req, &state.http_client, &upstream_base, &rewritten_path).await
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
