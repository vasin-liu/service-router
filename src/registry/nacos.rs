use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::config::model::NacosConfig;
use crate::error::RegistryError;
use crate::registry::{RegistryHealth, RegistryKind, ServiceInstance, ServiceRegistry};

/// Nacos service registry client with Bearer-token authentication.
pub struct NacosRegistry {
    config: NacosConfig,
    http: Client,
    /// Current access token; refreshed proactively.
    token: Arc<ArcSwap<String>>,
}

#[derive(Deserialize)]
struct NacosLoginResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "tokenTtl")]
    token_ttl: u64,
}

#[derive(Deserialize)]
struct NacosInstanceList {
    hosts: Vec<NacosHost>,
}

#[derive(Deserialize)]
struct NacosHost {
    ip: String,
    port: u16,
    healthy: bool,
    metadata: Option<std::collections::HashMap<String, String>>,
}

impl NacosRegistry {
    pub async fn new(config: NacosConfig, http: Client) -> Result<Self, RegistryError> {
        let token = Arc::new(ArcSwap::from_pointee(String::new()));

        let registry = Self { config, http, token };

        // Perform initial auth if credentials are configured.
        if registry.config.auth.is_some() {
            registry.authenticate().await?;
            registry.start_token_refresh_task();
        }

        Ok(registry)
    }

    async fn authenticate(&self) -> Result<(), RegistryError> {
        let auth = match &self.config.auth {
            Some(a) => a,
            None => return Ok(()),
        };

        let login_url = format!("{}/nacos/v1/auth/login", self.config.server_addr);
        let resp = self
            .http
            .post(&login_url)
            .form(&[
                ("username", auth.username.as_str()),
                ("password", auth.password.as_str()),
            ])
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::FORBIDDEN
            || resp.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            return Err(RegistryError::AuthFailed);
        }

        let login_resp: NacosLoginResponse = resp
            .json()
            .await
            .map_err(|e| RegistryError::UnexpectedResponse(e.to_string()))?;

        self.token.store(Arc::new(login_resp.access_token));

        let refresh_interval = Duration::from_secs(
            (login_resp.token_ttl as f64 * 0.8) as u64,
        );
        info!(
            ttl = login_resp.token_ttl,
            refresh_in_secs = refresh_interval.as_secs(),
            "Nacos token acquired"
        );

        Ok(())
    }

    fn start_token_refresh_task(&self) {
        let interval_secs = self
            .config
            .auth
            .as_ref()
            .map(|a| a.token_refresh_interval_secs)
            .unwrap_or(1800);

        let http = self.http.clone();
        let config = self.config.clone();
        let token_slot = Arc::clone(&self.token);

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(interval_secs));
            interval.tick().await; // skip the first immediate tick

            loop {
                interval.tick().await;

                let auth = match &config.auth {
                    Some(a) => a,
                    None => break,
                };

                let login_url = format!("{}/nacos/v1/auth/login", config.server_addr);
                match http
                    .post(&login_url)
                    .form(&[("username", &auth.username), ("password", &auth.password)])
                    .send()
                    .await
                {
                    Ok(resp) => match resp.json::<NacosLoginResponse>().await {
                        Ok(data) => {
                            token_slot.store(Arc::new(data.access_token));
                            info!("Nacos token refreshed");
                        }
                        Err(e) => warn!("Nacos token refresh parse error: {}", e),
                    },
                    Err(e) => error!("Nacos token refresh request failed: {}", e),
                }
            }
        });
    }

    fn current_token(&self) -> String {
        self.token.load_full().as_ref().clone()
    }
}

#[async_trait]
impl ServiceRegistry for NacosRegistry {
    fn kind(&self) -> RegistryKind {
        RegistryKind::Nacos
    }

    async fn get_healthy_instances(
        &self,
        service_id: &str,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        let mut url = format!(
            "{}/nacos/v1/ns/instance/list?serviceName={}&healthyOnly=true",
            self.config.server_addr, service_id
        );

        if let Some(ns) = &self.config.namespace {
            url.push_str(&format!("&namespaceId={}", ns));
        }
        if let Some(group) = &self.config.group {
            url.push_str(&format!("&groupName={}", group));
        }

        let token = self.current_token();
        let mut req = self.http.get(&url);
        if !token.is_empty() {
            req = req.bearer_auth(&token);
        }

        let resp = req.send().await?;

        // Re-authenticate on 401/403 and retry once.
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            warn!("Nacos returned 401/403; re-authenticating");
            self.authenticate().await?;
            let token = self.current_token();
            let resp = self.http.get(&url).bearer_auth(&token).send().await?;
            return self.parse_instances(resp).await;
        }

        self.parse_instances(resp).await
    }

    async fn health(&self) -> RegistryHealth {
        let ping_url = format!("{}/nacos/v1/console/health/liveness", self.config.server_addr);
        match self.http.get(&ping_url).send().await {
            Ok(r) if r.status().is_success() => RegistryHealth::Healthy,
            Ok(r) => RegistryHealth::Degraded(format!("HTTP {}", r.status())),
            Err(e) => RegistryHealth::Unhealthy(e.to_string()),
        }
    }
}

impl NacosRegistry {
    async fn parse_instances(
        &self,
        resp: reqwest::Response,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        if !resp.status().is_success() {
            return Err(RegistryError::UnexpectedResponse(format!(
                "Nacos returned {}",
                resp.status()
            )));
        }

        let list: NacosInstanceList = resp
            .json()
            .await
            .map_err(|e| RegistryError::UnexpectedResponse(e.to_string()))?;

        Ok(list
            .hosts
            .into_iter()
            .filter(|h| h.healthy)
            .map(|h| ServiceInstance {
                host: h.ip,
                port: h.port,
                metadata: h.metadata.unwrap_or_default(),
            })
            .collect())
    }
}
