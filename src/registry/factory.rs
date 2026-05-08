use reqwest::Client;

use crate::config::model::{AppConfig, RegistryConfig};
use crate::error::RegistryError;
use crate::registry::{
    eureka::EurekaRegistry, k8s::K8sRegistry, mock::MockRegistry, nacos::NacosRegistry, ArcRegistry,
    MultiRegistryResolver,
};

/// Build a `MultiRegistryResolver` from the registry configuration.
///
/// Nacos registries require async init (token auth), so this function is async.
pub async fn build_resolver(config: &AppConfig) -> Result<MultiRegistryResolver, RegistryError> {
    let http = build_http_client(config.server.upstream_timeout_secs);

    let mut registries: Vec<(u32, ArcRegistry)> = Vec::new();

    for registry_cfg in &config.registries.sources {
        let priority = registry_cfg.priority();
        let arc: ArcRegistry = match registry_cfg {
            RegistryConfig::Nacos(nacos_cfg) => {
                let registry = NacosRegistry::new(nacos_cfg.clone(), http.clone()).await?;
                std::sync::Arc::new(registry)
            }
            RegistryConfig::Eureka(eureka_cfg) => {
                std::sync::Arc::new(EurekaRegistry::new(eureka_cfg.clone(), http.clone()))
            }
            RegistryConfig::Kubernetes(k8s_cfg) => {
                std::sync::Arc::new(K8sRegistry::new(k8s_cfg.clone(), http.clone()))
            }
            RegistryConfig::Mock(mock_cfg) => {
                std::sync::Arc::new(MockRegistry::new(mock_cfg.clone(), http.clone()))
            }
        };
        registries.push((priority, arc));
    }

    Ok(MultiRegistryResolver::new(registries, config.registries.query_mode.clone()))
}

fn build_http_client(timeout_secs: u64) -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .expect("Failed to build registry HTTP client")
}
