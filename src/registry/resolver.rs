use std::sync::Arc;
use futures::future::join_all;
use tracing::{debug, warn};

use crate::config::model::RegistryQueryMode;
use crate::error::RegistryError;
use crate::registry::{ArcRegistry, RegistryHealth, ServiceInstance};

/// Queries multiple registries either in priority order or concurrently,
/// depending on `RegistryQueryMode`.
pub struct MultiRegistryResolver {
    /// Sorted ascending by priority (lower number = higher priority).
    registries: Vec<(u32, ArcRegistry)>,
    query_mode: RegistryQueryMode,
}

impl MultiRegistryResolver {
    pub fn new(mut registries: Vec<(u32, ArcRegistry)>, query_mode: RegistryQueryMode) -> Self {
        registries.sort_by_key(|(priority, _)| *priority);
        Self { registries, query_mode }
    }

    pub fn is_empty(&self) -> bool {
        self.registries.is_empty()
    }

    /// Resolve healthy instances for `service_id` according to the query mode.
    pub async fn resolve(&self, service_id: &str) -> Result<Vec<ServiceInstance>, RegistryError> {
        match self.query_mode {
            RegistryQueryMode::Priority => self.resolve_priority(service_id).await,
            RegistryQueryMode::Merge => self.resolve_merge(service_id).await,
        }
    }

    /// Returns per-registry health status in priority order.
    pub async fn health_report(&self) -> Vec<(u32, String, RegistryHealth)> {
        let futures: Vec<_> = self
            .registries
            .iter()
            .map(|(priority, registry)| {
                let priority = *priority;
                let kind = registry.kind().to_string();
                let reg = Arc::clone(registry);
                async move { (priority, kind, reg.health().await) }
            })
            .collect();
        join_all(futures).await
    }

    /// Try registries in priority order; return the first non-empty result.
    async fn resolve_priority(
        &self,
        service_id: &str,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        for (_, registry) in &self.registries {
            match registry.get_healthy_instances(service_id).await {
                Ok(instances) if !instances.is_empty() => {
                    debug!(
                        kind = %registry.kind(),
                        service = %service_id,
                        count = instances.len(),
                        "Priority resolver found instances"
                    );
                    return Ok(instances);
                }
                Ok(_) => {
                    debug!(
                        kind = %registry.kind(),
                        service = %service_id,
                        "Priority resolver: no instances, trying next"
                    );
                }
                Err(e) => {
                    warn!(
                        kind = %registry.kind(),
                        service = %service_id,
                        error = %e,
                        "Priority resolver: registry error, trying next"
                    );
                }
            }
        }
        Err(RegistryError::AllFailed(service_id.to_string()))
    }

    /// Query all registries concurrently and merge results (deduplicated by host:port).
    async fn resolve_merge(
        &self,
        service_id: &str,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        let futures: Vec<_> = self
            .registries
            .iter()
            .map(|(_, registry)| {
                let svc = service_id.to_string();
                let reg = Arc::clone(registry);
                async move { reg.get_healthy_instances(&svc).await }
            })
            .collect();

        let results = join_all(futures).await;

        // Collect all instances; deduplicate by (host, port).
        let mut seen = std::collections::HashSet::new();
        let mut merged: Vec<ServiceInstance> = Vec::new();

        for result in results {
            match result {
                Ok(instances) => {
                    for inst in instances {
                        let key = (inst.host.clone(), inst.port);
                        if seen.insert(key) {
                            merged.push(inst);
                        }
                    }
                }
                Err(e) => {
                    warn!(service = %service_id, error = %e, "Merge resolver: registry error");
                }
            }
        }

        if merged.is_empty() {
            Err(RegistryError::AllFailed(service_id.to_string()))
        } else {
            Ok(merged)
        }
    }
}
