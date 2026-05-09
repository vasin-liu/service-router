use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;

use crate::config::model::KubernetesConfig;
use crate::error::RegistryError;
use crate::registry::{RegistryHealth, RegistryKind, ServiceInstance, ServiceRegistry};

pub struct K8sRegistry {
    config: KubernetesConfig,
    http: Client,
    bearer_token: Option<String>,
}

impl K8sRegistry {
    pub fn new(
        mut config: KubernetesConfig,
        timeout_secs: u64,
    ) -> Result<Self, RegistryError> {
        let kubeconfig = load_kubeconfig_details(&config)?;

        if let Some(details) = &kubeconfig {
            if config.api_server_url.trim() == "https://kubernetes.default.svc" {
                config.api_server_url = details.server_url.clone();
            }
        }

        let bearer_token = resolve_bearer_token(&config, kubeconfig.as_ref())?;
        let http = build_http_client(&config, timeout_secs, kubeconfig.as_ref())?;

        Ok(Self {
            config,
            http,
            bearer_token,
        })
    }

    fn endpoint_url(&self, namespace: &str, service_id: &str) -> String {
        format!(
            "{}/api/v1/namespaces/{}/endpoints/{}",
            self.config.api_server_url.trim_end_matches('/'),
            namespace,
            service_id
        )
    }

    fn health_url(&self) -> String {
        format!("{}/readyz", self.config.api_server_url.trim_end_matches('/'))
    }

    fn with_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.bearer_token {
            if !token.trim().is_empty() {
                return req.bearer_auth(token);
            }
        }
        req
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
        let namespace = self.config.namespace.as_deref().unwrap_or("default");
        let url = self.endpoint_url(namespace, service_id);

        let req = self.with_auth(self.http.get(url));
        let resp = req.send().await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(RegistryError::AuthFailed);
        }
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }
        if !resp.status().is_success() {
            return Err(RegistryError::UnexpectedResponse(format!(
                "Kubernetes returned {}",
                resp.status()
            )));
        }

        let endpoints: K8sEndpoints = resp
            .json()
            .await
            .map_err(|e| RegistryError::UnexpectedResponse(e.to_string()))?;
        Ok(endpoints_to_instances(endpoints))
    }

    async fn health(&self) -> RegistryHealth {
        let req = self.with_auth(self.http.get(self.health_url()));
        match req.send().await {
            Ok(r) if r.status().is_success() => RegistryHealth::Healthy,
            Ok(r) => RegistryHealth::Degraded(format!("HTTP {}", r.status())),
            Err(e) => RegistryHealth::Unhealthy(e.to_string()),
        }
    }
}

fn resolve_bearer_token(
    config: &KubernetesConfig,
    kubeconfig: Option<&KubeconfigDetails>,
) -> Result<Option<String>, RegistryError> {
    if let Some(auth) = &config.auth {
        if let Some(token) = &auth.token {
            if !token.trim().is_empty() {
                return Ok(Some(token.clone()));
            }
        }
        if let Some(token_file) = &auth.token_file {
            let token = std::fs::read_to_string(token_file)
                .map_err(|e| RegistryError::UnexpectedResponse(format!("read token file failed: {e}")))?;
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }
    }

    Ok(kubeconfig.and_then(|k| k.user_token.clone()))
}

fn build_http_client(
    config: &KubernetesConfig,
    timeout_secs: u64,
    kubeconfig: Option<&KubeconfigDetails>,
) -> Result<Client, RegistryError> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(timeout_secs));
    if config.insecure_skip_tls_verify {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(details) = kubeconfig {
        if let Some(ca_pem) = &details.ca_pem {
            let cert = reqwest::Certificate::from_pem(ca_pem)
                .map_err(|e| RegistryError::UnexpectedResponse(format!("invalid kubeconfig CA data: {e}")))?;
            builder = builder.add_root_certificate(cert);
        }
        if let Some((cert_pem, key_pem)) = &details.client_identity_pem {
            let mut identity_pem = cert_pem.clone();
            identity_pem.extend_from_slice(key_pem);
            let identity = reqwest::Identity::from_pem(&identity_pem)
                .map_err(|e| RegistryError::UnexpectedResponse(format!("invalid kubeconfig client cert/key: {e}")))?;
            builder = builder.identity(identity);
        }
    }
    builder
        .build()
        .map_err(|e| RegistryError::UnexpectedResponse(format!("build k8s HTTP client failed: {e}")))
}

#[derive(Debug)]
struct KubeconfigDetails {
    server_url: String,
    ca_pem: Option<Vec<u8>>,
    client_identity_pem: Option<(Vec<u8>, Vec<u8>)>,
    user_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawKubeconfig {
    clusters: Option<Vec<NamedCluster>>,
    users: Option<Vec<NamedUser>>,
    contexts: Option<Vec<NamedContext>>,
    #[serde(rename = "current-context")]
    current_context: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NamedCluster {
    name: String,
    cluster: RawCluster,
}

#[derive(Debug, Deserialize)]
struct RawCluster {
    server: String,
    #[serde(rename = "certificate-authority-data")]
    certificate_authority_data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NamedUser {
    name: String,
    user: RawUser,
}

#[derive(Debug, Deserialize)]
struct RawUser {
    token: Option<String>,
    #[serde(rename = "client-certificate-data")]
    client_certificate_data: Option<String>,
    #[serde(rename = "client-key-data")]
    client_key_data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NamedContext {
    name: String,
    context: RawContext,
}

#[derive(Debug, Deserialize)]
struct RawContext {
    cluster: String,
    user: String,
}

fn load_kubeconfig_details(config: &KubernetesConfig) -> Result<Option<KubeconfigDetails>, RegistryError> {
    let Some(path) = &config.kubeconfig_path else {
        return Ok(None);
    };

    let raw = std::fs::read_to_string(path)
        .map_err(|e| RegistryError::UnexpectedResponse(format!("read kubeconfig failed: {e}")))?;
    let doc: RawKubeconfig = serde_yaml::from_str(&raw)
        .map_err(|e| RegistryError::UnexpectedResponse(format!("parse kubeconfig failed: {e}")))?;

    let context_name = config
        .kubeconfig_context
        .clone()
        .or(doc.current_context.clone())
        .ok_or_else(|| RegistryError::UnexpectedResponse("kubeconfig has no context".to_string()))?;

    let contexts = doc.contexts.unwrap_or_default();
    let selected_context = contexts
        .iter()
        .find(|c| c.name == context_name)
        .ok_or_else(|| RegistryError::UnexpectedResponse(format!("kubeconfig context not found: {context_name}")))?;

    let clusters = doc.clusters.unwrap_or_default();
    let cluster = clusters
        .iter()
        .find(|c| c.name == selected_context.context.cluster)
        .ok_or_else(|| {
            RegistryError::UnexpectedResponse(format!(
                "kubeconfig cluster not found: {}",
                selected_context.context.cluster
            ))
        })?;

    let users = doc.users.unwrap_or_default();
    let user = users
        .iter()
        .find(|u| u.name == selected_context.context.user)
        .ok_or_else(|| {
            RegistryError::UnexpectedResponse(format!(
                "kubeconfig user not found: {}",
                selected_context.context.user
            ))
        })?;

    let ca_pem = decode_base64_field(cluster.cluster.certificate_authority_data.as_deref())?;
    let client_cert = decode_base64_field(user.user.client_certificate_data.as_deref())?;
    let client_key = decode_base64_field(user.user.client_key_data.as_deref())?;
    let client_identity_pem = match (client_cert, client_key) {
        (Some(cert), Some(key)) => Some((cert, key)),
        _ => None,
    };

    Ok(Some(KubeconfigDetails {
        server_url: cluster.cluster.server.clone(),
        ca_pem,
        client_identity_pem,
        user_token: user.user.token.clone(),
    }))
}

fn decode_base64_field(input: Option<&str>) -> Result<Option<Vec<u8>>, RegistryError> {
    use base64::Engine as _;

    let Some(value) = input else {
        return Ok(None);
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|e| RegistryError::UnexpectedResponse(format!("invalid kubeconfig base64 data: {e}")))?;
    Ok(Some(bytes))
}

#[derive(Debug, Deserialize)]
struct K8sEndpoints {
    subsets: Option<Vec<K8sEndpointSubset>>,
}

#[derive(Debug, Deserialize)]
struct K8sEndpointSubset {
    addresses: Option<Vec<K8sEndpointAddress>>,
    ports: Option<Vec<K8sEndpointPort>>,
}

#[derive(Debug, Deserialize)]
struct K8sEndpointAddress {
    ip: String,
}

#[derive(Debug, Deserialize)]
struct K8sEndpointPort {
    port: u16,
}

fn endpoints_to_instances(endpoints: K8sEndpoints) -> Vec<ServiceInstance> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for subset in endpoints.subsets.unwrap_or_default() {
        let addresses = subset.addresses.unwrap_or_default();
        let ports = subset.ports.unwrap_or_default();
        for address in &addresses {
            for port in &ports {
                let key = format!("{}:{}", address.ip, port.port);
                if seen.insert(key) {
                    out.push(ServiceInstance {
                        host: address.ip.clone(),
                        port: port.port,
                        metadata: std::collections::HashMap::new(),
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_to_instances_flattens_and_deduplicates() {
        let endpoints = K8sEndpoints {
            subsets: Some(vec![
                K8sEndpointSubset {
                    addresses: Some(vec![K8sEndpointAddress {
                        ip: "10.0.0.1".to_string(),
                    }]),
                    ports: Some(vec![
                        K8sEndpointPort { port: 8080 },
                        K8sEndpointPort { port: 8080 },
                    ]),
                },
                K8sEndpointSubset {
                    addresses: Some(vec![K8sEndpointAddress {
                        ip: "10.0.0.2".to_string(),
                    }]),
                    ports: Some(vec![K8sEndpointPort { port: 9090 }]),
                },
            ]),
        };
        let instances = endpoints_to_instances(endpoints);
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].host, "10.0.0.1");
        assert_eq!(instances[0].port, 8080);
        assert_eq!(instances[1].host, "10.0.0.2");
        assert_eq!(instances[1].port, 9090);
    }
}
