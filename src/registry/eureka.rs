use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use crate::config::model::EurekaConfig;
use crate::error::RegistryError;
use crate::registry::{RegistryHealth, RegistryKind, ServiceInstance, ServiceRegistry};

/// Eureka service registry client with optional Basic Auth.
pub struct EurekaRegistry {
    config: EurekaConfig,
    http: Client,
}

#[derive(Deserialize)]
struct EurekaApp {
    application: EurekaApplication,
}

#[derive(Deserialize)]
struct EurekaApplication {
    instance: Vec<EurekaInstance>,
}

#[derive(Deserialize)]
struct EurekaInstance {
    #[serde(rename = "ipAddr")]
    ip_addr: String,
    port: EurekaPort,
    status: String,
    metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize)]
struct EurekaPort {
    #[serde(rename = "$")]
    value: u16,
}

impl EurekaRegistry {
    pub fn new(config: EurekaConfig, http: Client) -> Self {
        Self { config, http }
    }

    fn add_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(auth) = &self.config.auth {
            req.basic_auth(&auth.username, Some(&auth.password))
        } else {
            req
        }
    }
}

#[async_trait]
impl ServiceRegistry for EurekaRegistry {
    fn kind(&self) -> RegistryKind {
        RegistryKind::Eureka
    }

    async fn get_healthy_instances(
        &self,
        service_id: &str,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        // Eureka uses uppercase service names.
        let service_name = service_id.to_uppercase();
        let url = format!("{}/apps/{}", self.config.server_url.trim_end_matches('/'), service_name);

        let req = self
            .http
            .get(&url)
            .header("Accept", "application/json");
        let req = self.add_auth(req);

        let resp = req.send().await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RegistryError::AuthFailed);
        }
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        if !resp.status().is_success() {
            return Err(RegistryError::UnexpectedResponse(format!(
                "Eureka returned {}",
                resp.status()
            )));
        }

        let app: EurekaApp = resp
            .json()
            .await
            .map_err(|e| RegistryError::UnexpectedResponse(e.to_string()))?;

        Ok(app
            .application
            .instance
            .into_iter()
            .filter(|i| i.status == "UP")
            .map(|i| ServiceInstance {
                host: i.ip_addr,
                port: i.port.value,
                metadata: i.metadata.unwrap_or_default(),
            })
            .collect())
    }

    async fn health(&self) -> RegistryHealth {
        let info_url = format!("{}/info", self.config.server_url.trim_end_matches('/'));
        let req = self.http.get(&info_url);
        let req = self.add_auth(req);

        match req.send().await {
            Ok(r) if r.status().is_success() => RegistryHealth::Healthy,
            Ok(r) => RegistryHealth::Degraded(format!("HTTP {}", r.status())),
            Err(e) => RegistryHealth::Unhealthy(e.to_string()),
        }
    }
}
