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

    fn service_url(&self, namespace: &str, service_id: &str) -> String {
        format!(
            "{}/api/v1/namespaces/{}/services/{}",
            self.config.api_server_url.trim_end_matches('/'),
            namespace,
            service_id
        )
    }

    fn endpoint_slices_list_url(&self, namespace: &str, service_name: &str) -> String {
        let base = format!(
            "{}/apis/discovery.k8s.io/v1/namespaces/{}/endpointslices",
            self.config.api_server_url.trim_end_matches('/'),
            namespace
        );
        let selector = format!("kubernetes.io/service-name={}", service_name);
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("labelSelector", &selector)
            .finish();
        format!("{base}?{query}")
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

    /// Load Service `spec.ports` targets so we only keep backend ports that match TCP `targetPort`
    /// (numeric or port name), reducing spurious combinations when a Service exposes multiple ports.
    async fn fetch_service_tcp_filter(&self, namespace: &str, name: &str) -> Result<ServiceTcpFilter, RegistryError> {
        let url = self.service_url(namespace, name);
        let req = self.with_auth(self.http.get(url.clone()));
        let resp = req.send().await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(RegistryError::AuthFailed);
        }
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(ServiceTcpFilter::default());
        }
        if !resp.status().is_success() {
            return Err(RegistryError::UnexpectedResponse(format!(
                "Kubernetes service {} returned {}",
                url,
                resp.status()
            )));
        }

        let svc: K8sService = resp
            .json()
            .await
            .map_err(|e| RegistryError::UnexpectedResponse(e.to_string()))?;
        Ok(ServiceTcpFilter::from_service_ports(&svc.spec.ports))
    }

    async fn fetch_core_endpoints(
        &self,
        namespace: &str,
        name: &str,
        tcp_filter: &ServiceTcpFilter,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        let url = self.endpoint_url(namespace, name);
        let req = self.with_auth(self.http.get(url.clone()));
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
                "Kubernetes endpoints {} returned {}",
                url,
                resp.status()
            )));
        }

        let endpoints: K8sEndpoints = resp
            .json()
            .await
            .map_err(|e| RegistryError::UnexpectedResponse(e.to_string()))?;
        Ok(endpoints_to_instances(endpoints, tcp_filter))
    }

    async fn fetch_endpoint_slices(
        &self,
        namespace: &str,
        service_name: &str,
        tcp_filter: &ServiceTcpFilter,
    ) -> Result<Vec<ServiceInstance>, RegistryError> {
        let url = self.endpoint_slices_list_url(namespace, service_name);
        let req = self.with_auth(self.http.get(url.clone()));
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
                "Kubernetes endpointSlices {} returned {}",
                url,
                resp.status()
            )));
        }

        let list: EndpointSliceList = resp
            .json()
            .await
            .map_err(|e| RegistryError::UnexpectedResponse(e.to_string()))?;
        Ok(endpoint_slices_to_instances(list, tcp_filter))
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
        let tcp_filter = self.fetch_service_tcp_filter(namespace, service_id).await?;
        let from_core = self
            .fetch_core_endpoints(namespace, service_id, &tcp_filter)
            .await?;
        if !from_core.is_empty() {
            return Ok(from_core);
        }
        self.fetch_endpoint_slices(namespace, service_id, &tcp_filter).await
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

#[derive(Debug, Default)]
struct ServiceTcpFilter {
    /// Backend port numbers gathered from numeric `spec.ports[].targetPort`.
    numeric_targets: HashSet<u16>,
    /// Port names from string `spec.ports[].targetPort` (subset / slice entry `name` must match).
    named_targets: HashSet<String>,
}

impl ServiceTcpFilter {
    fn allows(&self, port: u16, port_entry_name: Option<&str>) -> bool {
        if self.numeric_targets.is_empty() && self.named_targets.is_empty() {
            return true;
        }
        if self.numeric_targets.contains(&port) {
            return true;
        }
        if let Some(name) = port_entry_name {
            if self.named_targets.contains(name) {
                return true;
            }
        }
        false
    }

    fn from_service_ports(ports: &[K8sServicePort]) -> Self {
        let mut numeric_targets = HashSet::new();
        let mut named_targets = HashSet::new();
        for p in ports {
            if protocol_excluded_from_http_proxy(p.protocol.as_deref()) {
                continue;
            }
            match &p.target_port {
                Some(TargetPort::Int(v)) => {
                    if let Ok(pp) = u16::try_from(*v) {
                        numeric_targets.insert(pp);
                    }
                }
                Some(TargetPort::Name(n)) => {
                    if !n.is_empty() {
                        named_targets.insert(n.clone());
                    }
                }
                None => {}
            }
        }
        Self {
            numeric_targets,
            named_targets,
        }
    }
}

fn protocol_excluded_from_http_proxy(protocol: Option<&str>) -> bool {
    matches!(
        protocol.map(|p| p.eq_ignore_ascii_case("UDP") || p.eq_ignore_ascii_case("SCTP")),
        Some(true)
    )
}

#[derive(Debug, Deserialize)]
struct K8sService {
    #[serde(default)]
    spec: K8sServiceSpec,
}

#[derive(Debug, Default, Deserialize)]
struct K8sServiceSpec {
    #[serde(default)]
    ports: Vec<K8sServicePort>,
}

#[derive(Debug, Deserialize)]
struct K8sServicePort {
    #[serde(rename = "targetPort")]
    target_port: Option<TargetPort>,
    #[serde(default)]
    protocol: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TargetPort {
    Int(u64),
    Name(String),
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
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    protocol: Option<String>,
}

/// `EndpointSliceList` (`discovery.k8s.io/v1`) — fallback when Core `Endpoints` is empty/unavailable.
#[derive(Debug, Deserialize)]
struct EndpointSliceList {
    #[serde(default)]
    items: Vec<EndpointSlice>,
}

#[derive(Debug, Deserialize)]
struct EndpointSlice {
    #[serde(default)]
    endpoints: Vec<SliceEndpoint>,
    #[serde(default)]
    ports: Vec<SlicePort>,
}

#[derive(Debug, Deserialize)]
struct SliceEndpoint {
    #[serde(default)]
    addresses: Vec<String>,
    #[serde(default)]
    conditions: SliceEndpointConditions,
}

#[derive(Debug, Deserialize, Default)]
struct SliceEndpointConditions {
    /// When `Some(false)`, drop (not ready / terminating).
    #[serde(default)]
    ready: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SlicePort {
    port: Option<u16>,
    #[serde(default)]
    name: Option<String>,
    protocol: Option<String>,
}

fn endpoint_slices_to_instances(list: EndpointSliceList, tcp_filter: &ServiceTcpFilter) -> Vec<ServiceInstance> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for slice in list.items {
        if slice.ports.is_empty() && slice.endpoints.is_empty() {
            continue;
        }
        for ep in slice.endpoints {
            if ep.conditions.ready == Some(false) {
                continue;
            }
            for addr in &ep.addresses {
                for port_ent in &slice.ports {
                    if protocol_excluded_from_http_proxy(port_ent.protocol.as_deref()) {
                        continue;
                    }
                    let Some(port_num) = port_ent.port else {
                        continue;
                    };
                    if !tcp_filter.allows(port_num, port_ent.name.as_deref()) {
                        continue;
                    }
                    let key = format!("{addr}:{port_num}");
                    if seen.insert(key) {
                        out.push(ServiceInstance {
                            host: addr.clone(),
                            port: port_num,
                            metadata: std::collections::HashMap::new(),
                        });
                    }
                }
            }
        }
    }
    out
}

fn endpoints_to_instances(endpoints: K8sEndpoints, tcp_filter: &ServiceTcpFilter) -> Vec<ServiceInstance> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for subset in endpoints.subsets.unwrap_or_default() {
        let addresses = subset.addresses.unwrap_or_default();
        let ports = subset.ports.unwrap_or_default();
        for address in &addresses {
            for port in &ports {
                if protocol_excluded_from_http_proxy(port.protocol.as_deref()) {
                    continue;
                }
                if !tcp_filter.allows(port.port, port.name.as_deref()) {
                    continue;
                }
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
                        K8sEndpointPort {
                            port: 8080,
                            name: None,
                            protocol: None,
                        },
                        K8sEndpointPort {
                            port: 8080,
                            name: None,
                            protocol: None,
                        },
                    ]),
                },
                K8sEndpointSubset {
                    addresses: Some(vec![K8sEndpointAddress {
                        ip: "10.0.0.2".to_string(),
                    }]),
                    ports: Some(vec![K8sEndpointPort {
                        port: 9090,
                        name: None,
                        protocol: None,
                    }]),
                },
            ]),
        };
        let filter = ServiceTcpFilter::default();
        let instances = endpoints_to_instances(endpoints, &filter);
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].host, "10.0.0.1");
        assert_eq!(instances[0].port, 8080);
        assert_eq!(instances[1].host, "10.0.0.2");
        assert_eq!(instances[1].port, 9090);
    }

    #[test]
    fn endpoints_to_instances_filtered_by_service_numeric_target() {
        let endpoints = K8sEndpoints {
            subsets: Some(vec![K8sEndpointSubset {
                addresses: Some(vec![K8sEndpointAddress {
                    ip: "10.0.0.1".to_string(),
                }]),
                ports: Some(vec![
                    K8sEndpointPort {
                        port: 8080,
                        name: Some("web".into()),
                        protocol: Some("TCP".into()),
                    },
                    K8sEndpointPort {
                        port: 8443,
                        name: Some("tls".into()),
                        protocol: Some("TCP".into()),
                    },
                ]),
            }]),
        };
        let svc_ports = [K8sServicePort {
            target_port: Some(TargetPort::Int(8080)),
            protocol: Some("TCP".into()),
        }];
        let filter = ServiceTcpFilter::from_service_ports(&svc_ports);
        let instances = endpoints_to_instances(endpoints, &filter);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].port, 8080);
    }

    #[test]
    fn endpoints_to_instances_filtered_by_service_named_target() {
        let endpoints = K8sEndpoints {
            subsets: Some(vec![K8sEndpointSubset {
                addresses: Some(vec![K8sEndpointAddress {
                    ip: "10.0.0.1".to_string(),
                }]),
                ports: Some(vec![
                    K8sEndpointPort {
                        port: 8443,
                        name: Some("https".into()),
                        protocol: Some("TCP".into()),
                    },
                    K8sEndpointPort {
                        port: 8080,
                        name: Some("http".into()),
                        protocol: Some("TCP".into()),
                    },
                ]),
            }]),
        };
        let svc_ports = [K8sServicePort {
            target_port: Some(TargetPort::Name("https".into())),
            protocol: Some("TCP".into()),
        }];
        let filter = ServiceTcpFilter::from_service_ports(&svc_ports);
        let instances = endpoints_to_instances(endpoints, &filter);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].port, 8443);
    }

    #[test]
    fn service_tcp_filter_skips_udp_and_sctp_spec_ports() {
        let svc_ports = [
            K8sServicePort {
                target_port: Some(TargetPort::Int(53)),
                protocol: Some("UDP".into()),
            },
            K8sServicePort {
                target_port: Some(TargetPort::Int(9900)),
                protocol: Some("SCTP".into()),
            },
            K8sServicePort {
                target_port: Some(TargetPort::Int(8080)),
                protocol: Some("TCP".into()),
            },
        ];
        let f = ServiceTcpFilter::from_service_ports(&svc_ports);
        assert_eq!(f.numeric_targets.len(), 1);
        assert!(f.numeric_targets.contains(&8080));
        assert!(f.named_targets.is_empty());
    }

    #[test]
    fn endpoint_slices_to_instances_filters_not_ready_and_udp() {
        let list = EndpointSliceList {
            items: vec![EndpointSlice {
                endpoints: vec![
                    SliceEndpoint {
                        addresses: vec!["10.1.1.1".to_string()],
                        conditions: SliceEndpointConditions {
                            ready: Some(true),
                        },
                    },
                    SliceEndpoint {
                        addresses: vec!["10.1.1.2".to_string()],
                        conditions: SliceEndpointConditions {
                            ready: Some(false),
                        },
                    },
                ],
                ports: vec![
                    SlicePort {
                        port: Some(443),
                        name: None,
                        protocol: Some("TCP".to_string()),
                    },
                    SlicePort {
                        port: Some(53),
                        name: None,
                        protocol: Some("UDP".to_string()),
                    },
                ],
            }],
        };
        let filter = ServiceTcpFilter::default();
        let instances = endpoint_slices_to_instances(list, &filter);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].host, "10.1.1.1");
        assert_eq!(instances[0].port, 443);
    }

    #[test]
    fn endpoint_slices_to_instances_defaults_ready_unknown_to_included() {
        let list = EndpointSliceList {
            items: vec![EndpointSlice {
                endpoints: vec![SliceEndpoint {
                    addresses: vec!["fc00::1".to_string()],
                    conditions: SliceEndpointConditions { ready: None },
                }],
                ports: vec![SlicePort {
                    port: Some(6443),
                    name: None,
                    protocol: None,
                }],
            }],
        };
        let filter = ServiceTcpFilter::default();
        let instances = endpoint_slices_to_instances(list, &filter);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].host, "fc00::1");
        assert_eq!(instances[0].port, 6443);
    }

    #[test]
    fn endpoint_slices_respect_named_service_target_filter() {
        let list = EndpointSliceList {
            items: vec![EndpointSlice {
                endpoints: vec![SliceEndpoint {
                    addresses: vec!["192.168.1.10".into()],
                    conditions: SliceEndpointConditions { ready: Some(true) },
                }],
                ports: vec![
                    SlicePort {
                        port: Some(8080),
                        name: Some("http".into()),
                        protocol: Some("TCP".into()),
                    },
                    SlicePort {
                        port: Some(8443),
                        name: Some("webhook".into()),
                        protocol: Some("TCP".into()),
                    },
                ],
            }],
        };
        let svc_ports = [K8sServicePort {
            target_port: Some(TargetPort::Name("webhook".into())),
            protocol: Some("TCP".into()),
        }];
        let filter = ServiceTcpFilter::from_service_ports(&svc_ports);
        let instances = endpoint_slices_to_instances(list, &filter);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].port, 8443);
    }
}
