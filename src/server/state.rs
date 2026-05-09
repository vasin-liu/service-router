use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use reqwest::Client;

use crate::config::model::AppConfig;
use crate::registry::MultiRegistryResolver;
use crate::routing::SharedRouter;
use crate::server::metrics::ProxyMetrics;

/// Shared application state injected into every Axum handler.
#[derive(Clone)]
pub struct AppState {
    /// Current routing snapshot (hot-swappable).
    pub router: SharedRouter,
    /// Multi-registry resolver for service discovery (hot-swappable).
    pub resolver: Arc<ArcSwap<MultiRegistryResolver>>,
    /// Current application config (hot-swappable).
    pub config: Arc<ArcSwap<AppConfig>>,
    /// Shared HTTP client for proxying requests to upstream services.
    pub http_client: Client,
    /// In-memory request/failure counters (B08).
    pub metrics: Arc<ProxyMetrics>,
}

impl AppState {
    pub fn new(
        router: SharedRouter,
        resolver: Arc<ArcSwap<MultiRegistryResolver>>,
        config: Arc<ArcSwap<AppConfig>>,
        upstream_timeout_secs: u64,
        metrics: Arc<ProxyMetrics>,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(upstream_timeout_secs))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            router,
            resolver,
            config,
            http_client,
            metrics,
        }
    }
}
