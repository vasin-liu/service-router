use async_trait::async_trait;
use reqwest::Client;

use crate::config::model::MockRegistryConfig;
use crate::error::RegistryError;
use crate::registry::{RegistryHealth, RegistryKind, ServiceInstance, ServiceRegistry};

/// In-memory registry implementation for local development and tests.
pub struct MockRegistry {
    config: MockRegistryConfig,
    _http: Client,
}

impl MockRegistry {
    pub fn new(config: MockRegistryConfig, http: Client) -> Self {
        Self { config, _http: http }
    }
}

#[async_trait]
impl ServiceRegistry for MockRegistry {
    fn kind(&self) -> RegistryKind {
        RegistryKind::Mock
    }

    async fn get_healthy_instances(
        &self,
        service_id: &str,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        Ok(self
            .config
            .services
            .get(service_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|inst| ServiceInstance {
                host: inst.host,
                port: inst.port,
                metadata: inst.metadata,
            })
            .collect())
    }

    async fn health(&self) -> RegistryHealth {
        RegistryHealth::Healthy
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::model::{MockRegistryConfig, MockServiceInstance};

    fn make_registry() -> MockRegistry {
        let mut services = HashMap::new();
        services.insert(
            "order-service".to_string(),
            vec![
                MockServiceInstance {
                    host: "127.0.0.1".to_string(),
                    port: 9001,
                    metadata: HashMap::new(),
                },
                MockServiceInstance {
                    host: "127.0.0.1".to_string(),
                    port: 9002,
                    metadata: HashMap::new(),
                },
            ],
        );
        let cfg = MockRegistryConfig {
            priority: 1,
            services,
        };
        let http = reqwest::Client::builder()
            .build()
            .expect("mock registry test client");
        MockRegistry::new(cfg, http)
    }

    #[tokio::test]
    async fn returns_instances_for_existing_service() {
        let registry = make_registry();
        let instances = registry
            .get_healthy_instances("order-service")
            .await
            .expect("resolve service");
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].host, "127.0.0.1");
        assert_eq!(instances[0].port, 9001);
    }

    #[tokio::test]
    async fn returns_empty_for_unknown_service() {
        let registry = make_registry();
        let instances = registry
            .get_healthy_instances("unknown")
            .await
            .expect("resolve unknown service");
        assert!(instances.is_empty());
    }
}
