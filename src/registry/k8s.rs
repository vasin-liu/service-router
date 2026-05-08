use async_trait::async_trait;
use reqwest::Client;

use crate::config::model::KubernetesConfig;
use crate::error::RegistryError;
use crate::registry::{RegistryHealth, RegistryKind, ServiceInstance, ServiceRegistry};

/// Kubernetes service registry stub.
///
/// This is a placeholder implementation. A full implementation would use the
/// Kubernetes API server to discover services and endpoints.
pub struct K8sRegistry {
    config: KubernetesConfig,
    http: Client,
}

impl K8sRegistry {
    pub fn new(config: KubernetesConfig, http: Client) -> Self {
        Self { config, http }
    }
}

#[async_trait]
impl ServiceRegistry for K8sRegistry {
    fn kind(&self) -> RegistryKind {
        RegistryKind::Kubernetes
    }

    async fn get_healthy_instances(
        &self,
        service_id: &str,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        // TODO: Implement Kubernetes endpoint discovery via the API server.
        // For now this stub returns an empty list so other registries in the
        // resolver chain can be tried.
        tracing::warn!(
            service = %service_id,
            "Kubernetes registry is not yet implemented; returning empty instance list"
        );
        Ok(vec![])
    }

    async fn health(&self) -> RegistryHealth {
        // Attempt a simple request to the API server liveness endpoint.
        let url = format!(
            "{}/healthz",
            self.config.api_server_url.trim_end_matches('/')
        );
        match self.http.get(&url).send().await {
            Ok(r) if r.status().is_success() => RegistryHealth::Healthy,
            Ok(r) => RegistryHealth::Degraded(format!("HTTP {}", r.status())),
            Err(e) => RegistryHealth::Unhealthy(e.to_string()),
        }
    }
}
