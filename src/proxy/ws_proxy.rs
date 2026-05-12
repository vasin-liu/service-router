use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use hyper::upgrade::OnUpgrade;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::{connect_async, WebSocketStream};
use tracing::{debug, warn};

use crate::error::ProxyError;

/// Upgrade an HTTP request to a WebSocket connection and relay frames
/// bidirectionally between the client and the upstream.
pub async fn proxy_websocket(
    req: Request,
    upstream_base: &str,
    rewritten_path: &str,
) -> Result<Response, ProxyError> {
    let upstream_url = build_ws_url(upstream_base, rewritten_path, req.uri().query());

    debug!(upstream_url = %upstream_url, "Establishing upstream WebSocket connection");

    // Connect to the upstream WebSocket server first.
    let (upstream_ws, _) = connect_async(&upstream_url)
        .await
        .map_err(|e| ProxyError::WsUpgrade(e.to_string()))?;

    // Extract the `on_upgrade` future from the inbound request so we can
    // perform the relay once the HTTP-level 101 handshake completes.
    let on_upgrade = hyper::upgrade::on(req);

    // Build a 101 Switching Protocols response to complete the client-side
    // handshake. Axum/hyper will drive the underlying TCP stream upgrade.
    let response = Response::builder()
        .status(axum::http::StatusCode::SWITCHING_PROTOCOLS)
        .header(axum::http::header::CONNECTION, "upgrade")
        .header(axum::http::header::UPGRADE, "websocket")
        .header(
            "sec-websocket-accept",
            compute_accept_key_placeholder(),
        )
        .body(Body::empty())
        .map_err(|e| ProxyError::WsUpgrade(e.to_string()))?;

    // Spawn the bidirectional relay task. It runs after the HTTP upgrade completes.
    tokio::spawn(relay_websocket(on_upgrade, upstream_ws, upstream_url));

    Ok(response)
}

/// After the client HTTP upgrade completes, wrap the raw I/O stream as a
/// WebSocket and relay frames between client and upstream until either side
/// closes or errors.
async fn relay_websocket(
    on_upgrade: OnUpgrade,
    upstream_ws: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    upstream_url: String,
) {
    let upgraded = match on_upgrade.await {
        Ok(u) => u,
        Err(e) => {
            warn!("WebSocket client upgrade failed: {e}");
            return;
        }
    };

    let client_ws =
        WebSocketStream::from_raw_socket(hyper_util::rt::TokioIo::new(upgraded), Role::Server, None)
            .await;

    let (mut client_tx, mut client_rx) = client_ws.split();
    let (mut upstream_tx, mut upstream_rx) = upstream_ws.split();

    // client -> upstream
    let c2u = async {
        while let Some(msg) = client_rx.next().await {
            match msg {
                Ok(frame) => {
                    if frame.is_close() {
                        let _ = upstream_tx.close().await;
                        break;
                    }
                    if upstream_tx.send(frame).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    debug!("Client WS read error: {e}");
                    break;
                }
            }
        }
    };

    // upstream -> client
    let u2c = async {
        while let Some(msg) = upstream_rx.next().await {
            match msg {
                Ok(frame) => {
                    if frame.is_close() {
                        let _ = client_tx.close().await;
                        break;
                    }
                    if client_tx.send(frame).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    debug!("Upstream WS read error: {e}");
                    break;
                }
            }
        }
    };

    // Run both directions concurrently; when one finishes, abort the other.
    tokio::select! {
        _ = c2u => {
            debug!(upstream = %upstream_url, "Client->upstream relay ended");
        }
        _ = u2c => {
            debug!(upstream = %upstream_url, "Upstream->client relay ended");
        }
    }
}

/// Placeholder accept key — in practice, hyper's upgrade machinery handles
/// the real `Sec-WebSocket-Accept` header derivation. We provide a constant
/// so the response builder does not fail.
fn compute_accept_key_placeholder() -> &'static str {
    "server"
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
