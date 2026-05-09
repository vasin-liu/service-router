use async_trait::async_trait;
use reqwest::Client;

use crate::config::model::{MockRegistryConfig, MockRegistryHealthBehavior};
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
        if let Some(msg) = self.config.error_services.get(service_id) {
            return Err(RegistryError::UnexpectedResponse(msg.clone()));
        }
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
        match &self.config.health_behavior {
            MockRegistryHealthBehavior::Healthy => RegistryHealth::Healthy,
            MockRegistryHealthBehavior::Degraded { message } => {
                if message.is_empty() {
                    RegistryHealth::Degraded("mock degraded".to_string())
                } else {
                    RegistryHealth::Degraded(message.clone())
                }
            }
            MockRegistryHealthBehavior::Unhealthy { message } => {
                if message.is_empty() {
                    RegistryHealth::Unhealthy("mock unhealthy".to_string())
                } else {
                    RegistryHealth::Unhealthy(message.clone())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::model::{MockRegistryConfig, MockServiceInstance};
    use crate::error::RegistryError;

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
            error_services: HashMap::new(),
            health_behavior: MockRegistryHealthBehavior::Healthy,
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

    #[tokio::test]
    async fn returns_empty_for_explicit_empty_instances() {
        let mut services = HashMap::new();
        services.insert("empty".to_string(), vec![]);
        let cfg = MockRegistryConfig {
            priority: 1,
            services,
            error_services: HashMap::new(),
            health_behavior: MockRegistryHealthBehavior::Healthy,
        };
        let http = reqwest::Client::builder().build().unwrap();
        let registry = MockRegistry::new(cfg, http);
        let instances = registry
            .get_healthy_instances("empty")
            .await
            .expect("resolve empty service");
        assert!(instances.is_empty());
    }

    #[tokio::test]
    async fn error_service_returns_registry_error() {
        let mut error_services = HashMap::new();
        error_services.insert("down".to_string(), "simulated outage".to_string());
        let cfg = MockRegistryConfig {
            priority: 1,
            services: HashMap::new(),
            error_services,
            health_behavior: MockRegistryHealthBehavior::Healthy,
        };
        let http = reqwest::Client::builder().build().unwrap();
        let registry = MockRegistry::new(cfg, http);
        let err = registry
            .get_healthy_instances("down")
            .await
            .expect_err("expected synthetic error");
        assert!(matches!(err, RegistryError::UnexpectedResponse(_)));
    }

    #[tokio::test]
    async fn degraded_health_reports_degraded() {
        let cfg = MockRegistryConfig {
            priority: 1,
            services: HashMap::new(),
            error_services: HashMap::new(),
            health_behavior: MockRegistryHealthBehavior::Degraded {
                message: "rollout".into(),
            },
        };
        let http = reqwest::Client::builder().build().unwrap();
        let registry = MockRegistry::new(cfg, http);
        assert!(matches!(
            registry.health().await,
            RegistryHealth::Degraded(ref m) if m == "rollout"
        ));
    }

    #[tokio::test]
    async fn unhealthy_health_reports_unhealthy() {
        let cfg = MockRegistryConfig {
            priority: 1,
            services: HashMap::new(),
            error_services: HashMap::new(),
            health_behavior: MockRegistryHealthBehavior::Unhealthy {
                message: "dead".into(),
            },
        };
        let http = reqwest::Client::builder().build().unwrap();
        let registry = MockRegistry::new(cfg, http);
        assert!(matches!(
            registry.health().await,
            RegistryHealth::Unhealthy(ref m) if m == "dead"
        ));
    }
}
