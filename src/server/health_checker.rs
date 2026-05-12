use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use reqwest::Client;
use tracing::{debug, info, warn};

use crate::config::model::HealthCheckConfig;

/// Thread-safe set of upstream base URLs currently marked unhealthy by active
/// health checks. Instances whose ase_url() appears in this set are skipped
/// by select_service_instance.
#[derive(Debug)]
pub struct HealthStatus {
    /// Key: "host:port", present = unhealthy.
    unhealthy: DashMap<String, ()>,
    /// Consecutive failure counter per host.
    fail_counts: DashMap<String, u32>,
    /// Consecutive success counter per host (used for healthy_threshold).
    ok_counts: DashMap<String, u32>,
}

impl HealthStatus {
    pub fn new() -> Self {
        Self {
            unhealthy: DashMap::new(),
            fail_counts: DashMap::new(),
            ok_counts: DashMap::new(),
        }
    }

    /// Returns true if the given host:port key is healthy (not in the unhealthy set).
    pub fn is_healthy(&self, key: &str) -> bool {
        !self.unhealthy.contains_key(key)
    }

    fn record_success(&self, key: &str, healthy_threshold: u32) {
        self.fail_counts.remove(key);
        let count = {
            let mut entry = self.ok_counts.entry(key.to_string()).or_insert(0);
            *entry.value_mut() += 1;
            *entry.value()
        };
        if count >= healthy_threshold && self.unhealthy.remove(key).is_some() {
            info!(host = %key, "Health check: instance recovered");
        }
    }

    fn record_failure(&self, key: &str, unhealthy_threshold: u32) {
        self.ok_counts.remove(key);
        let count = {
            let mut entry = self.fail_counts.entry(key.to_string()).or_insert(0);
            *entry.value_mut() += 1;
            *entry.value()
        };
        if count >= unhealthy_threshold {
            if self.unhealthy.insert(key.to_string(), ()).is_none() {
                warn!(host = %key, consecutive_failures = count, "Health check: instance marked unhealthy");
            }
        }
    }
}

/// Spawns a background task that periodically probes all known upstream
/// instances and updates HealthStatus.
pub fn spawn_health_checker(
    config: HealthCheckConfig,
    resolver: Arc<arc_swap::ArcSwap<crate::registry::MultiRegistryResolver>>,
    app_config: Arc<arc_swap::ArcSwap<crate::config::model::AppConfig>>,
    status: Arc<HealthStatus>,
) -> tokio::task::JoinHandle<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .expect("health check HTTP client");

    tokio::spawn(async move {
        let interval = Duration::from_secs(config.interval_secs);
        loop {
            tokio::time::sleep(interval).await;
            probe_all(
                &client,
                &config,
                &resolver,
                &app_config,
                &status,
            )
            .await;
        }
    })
}

async fn probe_all(
    client: &Client,
    config: &HealthCheckConfig,
    resolver: &arc_swap::ArcSwap<crate::registry::MultiRegistryResolver>,
    app_config: &arc_swap::ArcSwap<crate::config::model::AppConfig>,
    status: &HealthStatus,
) {
    let cfg = app_config.load();
    let resolver = resolver.load();

    let mut seen_keys = HashSet::new();

    for route in &cfg.routes {
        let svc_id = match &route.service_id {
            Some(s) => s.clone(),
            None => continue,
        };
        let instances = match resolver.resolve(&svc_id).await {
            Ok(i) => i,
            Err(_) => continue,
        };
        for inst in &instances {
            let key = format!("{}:{}", inst.host, inst.port);
            if !seen_keys.insert(key.clone()) {
                continue;
            }
            let url = format!("http://{}:{}{}", inst.host, inst.port, config.path);
            let healthy = match client.get(&url).send().await {
                Ok(resp) => resp.status().is_success(),
                Err(_) => false,
            };
            if healthy {
                status.record_success(&key, config.healthy_threshold);
                debug!(host = %key, "Health check: OK");
            } else {
                status.record_failure(&key, config.unhealthy_threshold);
                debug!(host = %key, "Health check: FAIL");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_by_default() {
        let hs = HealthStatus::new();
        assert!(hs.is_healthy("10.0.0.1:8080"));
    }

    #[test]
    fn marks_unhealthy_after_threshold() {
        let hs = HealthStatus::new();
        let key = "10.0.0.1:8080";
        hs.record_failure(key, 3);
        assert!(hs.is_healthy(key));
        hs.record_failure(key, 3);
        assert!(hs.is_healthy(key));
        hs.record_failure(key, 3);
        assert!(!hs.is_healthy(key));
    }

    #[test]
    fn recovers_after_success_threshold() {
        let hs = HealthStatus::new();
        let key = "10.0.0.1:8080";
        for _ in 0..3 {
            hs.record_failure(key, 3);
        }
        assert!(!hs.is_healthy(key));
        hs.record_success(key, 2);
        assert!(!hs.is_healthy(key));
        hs.record_success(key, 2);
        assert!(hs.is_healthy(key));
    }

    #[test]
    fn success_resets_failure_counter() {
        let hs = HealthStatus::new();
        let key = "10.0.0.1:8080";
        hs.record_failure(key, 3);
        hs.record_failure(key, 3);
        hs.record_success(key, 1);
        hs.record_failure(key, 3);
        assert!(hs.is_healthy(key));
    }
}