use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use arc_swap::ArcSwap;
use dashmap::DashMap;
use reqwest::Client;

use crate::config::model::AppConfig;
use crate::registry::MultiRegistryResolver;
use crate::routing::SharedRouter;
use crate::server::circuit_breaker::CircuitBreakerMap;
use crate::server::metrics::ProxyMetrics;
use crate::server::health_checker::HealthStatus;
use crate::server::plugin::PluginChain;

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
    /// Per-`service_id` counter for [`crate::config::model::InstanceSelection::RoundRobin`].
    pub instance_rr_counters: Arc<DashMap<String, AtomicUsize>>,
    /// Per-upstream circuit breaker state.
    pub circuit_breaker: Arc<CircuitBreakerMap>,
    /// Plugin pipeline for request/response interception.
    pub plugin_chain: Arc<PluginChain>,
    /// Active health check status (instances marked unhealthy are skipped).
    pub health_status: Arc<HealthStatus>,
}

impl AppState {
    pub fn new(
        router: SharedRouter,
        resolver: Arc<ArcSwap<MultiRegistryResolver>>,
        config: Arc<ArcSwap<AppConfig>>,
        upstream_timeout_secs: u64,
        metrics: Arc<ProxyMetrics>,
    ) -> Self {
        let cfg = config.load();
        let http_client = Client::builder()
            .timeout(Duration::from_secs(upstream_timeout_secs))
            .build()
            .expect("Failed to build HTTP client");
        let circuit_breaker = Arc::new(CircuitBreakerMap::new(
            cfg.server.circuit_breaker_threshold,
            cfg.server.circuit_breaker_recovery_secs,
        ));

        Self {
            router,
            resolver,
            config,
            http_client,
            metrics,
            instance_rr_counters: Arc::new(DashMap::new()),
            circuit_breaker,
            plugin_chain: Arc::new(PluginChain::new(Vec::new())),
            health_status: Arc::new(HealthStatus::new()),
        }
    }
}
