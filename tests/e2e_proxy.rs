use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::routing::{any, get};
use axum::Router;
use tokio::net::TcpListener;

use service_router::config::model::*;
use service_router::registry::build_resolver;
use service_router::routing::RouterSnapshot;
use service_router::server::handlers::{health_handler, proxy_handler, ready_handler};
use service_router::server::{AppState, ProxyMetrics};

async fn spawn_upstream(body: &'static str) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let body = body.to_string();
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = vec![0u8; 4096];
                let _ = stream.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });
    (addr, handle)
}

fn mock_config(upstream_addr: SocketAddr) -> AppConfig {
    let mut services = HashMap::new();
    services.insert(
        "test-svc".to_string(),
        vec![MockServiceInstance {
            host: upstream_addr.ip().to_string(),
            port: upstream_addr.port(),
            metadata: HashMap::new(),
        }],
    );

    AppConfig {
        server: ServerConfig::default(),
        registries: RegistriesConfig {
            sources: vec![RegistryConfig::Mock(MockRegistryConfig {
                priority: 1,
                services,
                error_services: HashMap::new(),
                health_behavior: MockRegistryHealthBehavior::default(),
            })],
            ..Default::default()
        },
        routes: vec![RoutingRule {
            id: "catch-all".to_string(),
            path: PathMatcher::Prefix { value: "/".to_string() },
            methods: None,
            headers: None,
            service_id: Some("test-svc".to_string()),
            upstream_url: None,
            strip_prefix: None,
            response_headers: None,
            priority: 100,
        }],
        ..Default::default()
    }
}

async fn start_proxy(config: AppConfig) -> (SocketAddr, tokio::sync::oneshot::Sender<()>) {
    let resolver = build_resolver(&config).await.unwrap();
    let snapshot = RouterSnapshot::from_config(&config).unwrap();
    let metrics = Arc::new(ProxyMetrics::default());

    let router = Arc::new(ArcSwap::from_pointee(snapshot));
    let resolver_swap = Arc::new(ArcSwap::from_pointee(resolver));
    let config_swap = Arc::new(ArcSwap::from_pointee(config.clone()));

    let state = AppState::new(
        router,
        resolver_swap,
        config_swap,
        config.server.upstream_timeout_secs,
        metrics,
    );

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .fallback(any(proxy_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async { rx.await.ok(); })
            .await
            .unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    (addr, tx)
}

#[tokio::test]
async fn proxy_forwards_get_to_upstream() {
    let (upstream_addr, _h) = spawn_upstream("hello from upstream").await;
    let config = mock_config(upstream_addr);
    let (proxy_addr, shutdown) = start_proxy(config).await;

    let resp = reqwest::get(format!("http://{}/test", proxy_addr))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "hello from upstream");

    let _ = shutdown.send(());
}

#[tokio::test]
async fn proxy_returns_x_request_id_header() {
    let (upstream_addr, _h) = spawn_upstream("ok").await;
    let config = mock_config(upstream_addr);
    let (proxy_addr, shutdown) = start_proxy(config).await;

    let resp = reqwest::get(format!("http://{}/any", proxy_addr))
        .await
        .unwrap();
    assert!(resp.headers().contains_key("x-request-id"));

    let _ = shutdown.send(());
}

#[tokio::test]
async fn health_endpoint_returns_200() {
    let (upstream_addr, _h) = spawn_upstream("ignored").await;
    let config = mock_config(upstream_addr);
    let (proxy_addr, shutdown) = start_proxy(config).await;

    let resp = reqwest::get(format!("http://{}/health", proxy_addr))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let _ = shutdown.send(());
}
