use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::{debug, info};

use crate::error::ProxyError;
use crate::proxy::{http_proxy, ws_proxy};
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
    let rule = router_snapshot
        .resolve(&path, method.as_str(), req.headers())
        .ok_or_else(|| {
            debug!(path = %path, "No matching route found");
            ProxyError::NoInstances(path.clone())
        })?;

    debug!(
        rule_id = %rule.id,
        path = %path,
        "Matched routing rule"
    );

    // Determine the base URL of the upstream.
    let upstream_base = if let Some(url) = &rule.upstream_url {
        url.clone()
    } else if let Some(svc_id) = &rule.service_id {
        let resolver = state.resolver.load();
        let instances = resolver.resolve(svc_id.as_str()).await?;

        // Simple round-robin: pick the first instance for now.
        // TODO: replace with a proper load balancer in Phase 3.
        instances
            .first()
            .map(|i| i.base_url())
            .ok_or_else(|| ProxyError::NoInstances(svc_id.clone()))?
    } else {
        return Err(ProxyError::NoInstances(path.clone()));
    };

    // Rewrite path (strip prefix if configured).
    let rewritten_path = rule.rewrite_path(&path);

    if is_upgrade {
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
    }
}

/// `/health` — liveness probe.
pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// `/ready` — readiness probe; checks whether at least one registry is reachable.
pub async fn ready_handler(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    if config.registries.sources.is_empty() {
        // No registries configured — still considered ready (direct URL routing works).
        return (StatusCode::OK, Json(json!({ "status": "ready", "registries": 0 })));
    }
    // For now just return OK; detailed registry health checks can be added later.
    (
        StatusCode::OK,
        Json(json!({ "status": "ready", "registries": config.registries.sources.len() })),
    )
}
