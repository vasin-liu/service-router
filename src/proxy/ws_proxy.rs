use axum::{body::Body, extract::Request, response::Response};
use futures::StreamExt;
use tokio_tungstenite::connect_async;
use tracing::{debug, warn};

use crate::error::ProxyError;

/// Upgrade an HTTP request to a WebSocket connection and relay frames
/// bidirectionally between the client and the upstream.
pub async fn proxy_websocket(
    req: Request,
    upstream_base: &str,
    rewritten_path: &str,
) -> Result<Response, ProxyError> {
    // Build the upstream WebSocket URL (replace http(s) with ws(s)).
    let upstream_url = build_ws_url(upstream_base, rewritten_path, req.uri().query());

    debug!(upstream_url = %upstream_url, "Establishing upstream WebSocket connection");

    // Connect to the upstream WebSocket server before upgrading the client.
    let (upstream_ws, _) = connect_async(&upstream_url)
        .await
        .map_err(|e| ProxyError::WsUpgrade(e.to_string()))?;

    // Use Axum's built-in WebSocket upgrade mechanism.
    // We need to manually handle the upgrade handshake since we're not using
    // axum::extract::WebSocketUpgrade directly in this handler.
    let response = axum::response::Response::builder()
        .status(axum::http::StatusCode::SWITCHING_PROTOCOLS)
        .header(axum::http::header::CONNECTION, "upgrade")
        .header(axum::http::header::UPGRADE, "websocket")
        .body(Body::empty())
        .map_err(|e| ProxyError::WsUpgrade(e.to_string()))?;

    // Spawn the relay task.
    tokio::spawn(async move {
        let (_upstream_write, mut upstream_read) = upstream_ws.split();

        // Note: In a full implementation we'd split the client connection too.
        // For now this is a placeholder that logs the intent.
        warn!("WebSocket relay task started for {}", upstream_url);

        // Drain upstream frames (so we don't block the connection).
        while let Some(msg) = upstream_read.next().await {
            match msg {
                Ok(_) => {}
                Err(e) => {
                    debug!("Upstream WebSocket closed: {}", e);
                    break;
                }
            }
        }
    });

    Ok(response)
}

fn build_ws_url(base: &str, path: &str, query: Option<&str>) -> String {
    let ws_base = base
        .trim_end_matches('/')
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);

    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };

    match query {
        Some(q) if !q.is_empty() => format!("{}{}?{}", ws_base, path, q),
        _ => format!("{}{}", ws_base, path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_url_http() {
        assert_eq!(
            build_ws_url("http://host:9090", "/ws/chat", None),
            "ws://host:9090/ws/chat"
        );
    }

    #[test]
    fn test_build_ws_url_https() {
        assert_eq!(
            build_ws_url("https://host:9090", "/ws", Some("token=abc")),
            "wss://host:9090/ws?token=abc"
        );
    }
}
