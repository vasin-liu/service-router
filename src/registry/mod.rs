pub mod nacos;
pub mod eureka;
pub mod k8s;
pub mod mock;
pub mod resolver;
pub mod factory;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::RegistryError;

/// A single service instance returned by a registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInstance {
    pub host: String,
    pub port: u16,
    /// Optional metadata tags (weight, zone, version, …)
    pub metadata: std::collections::HashMap<String, String>,
}

impl ServiceInstance {
    /// Build the base URL for this instance.
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Health status of a registry connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryHealth {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Kind / name of a registry, used for logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryKind {
    Nacos,
    Eureka,
    Kubernetes,
    Mock,
}

impl std::fmt::Display for RegistryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryKind::Nacos => write!(f, "Nacos"),
            RegistryKind::Eureka => write!(f, "Eureka"),
            RegistryKind::Kubernetes => write!(f, "Kubernetes"),
            RegistryKind::Mock => write!(f, "Mock"),
        }
    }
}

/// Abstraction over a service registry.
#[async_trait]
pub trait ServiceRegistry: Send + Sync + 'static {
    fn kind(&self) -> RegistryKind;

    /// Return healthy instances for the given service ID.
    async fn get_healthy_instances(
        &self,
        service_id: &str,
    ) -> Result<Vec<ServiceInstance>, RegistryError>;

    /// Liveness check for the registry connection itself.
    async fn health(&self) -> RegistryHealth;
}

/// Type-erased, reference-counted registry handle.
pub type ArcRegistry = Arc<dyn ServiceRegistry>;

pub use resolver::MultiRegistryResolver;
