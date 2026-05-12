use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    /// Schema version of this config file. Omitted configs are treated as "1".
    #[serde(default = "default_config_version")]
    pub config_version: String,
    pub server: ServerConfig,
    pub registries: RegistriesConfig,
    pub routes: Vec<RoutingRule>,
    #[serde(default)]
    pub log_level: String,
}

fn default_config_version() -> String { "1".to_string() }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: default_config_version(),
            server: ServerConfig::default(),
            registries: RegistriesConfig::default(),
            routes: Vec::new(),
            log_level: "info".to_string(),
        }
    }
}

impl AppConfig {
    /// Apply local overrides: for each override entry, find the route with the
    /// matching `id` and replace its target with the override `upstream_url`.
    pub fn apply_local_overrides(&mut self, overrides: &[LocalOverrideEntry]) {
        for entry in overrides {
            if let Some(route) = self.routes.iter_mut().find(|r| r.id == entry.route_id) {
                route.upstream_url = Some(entry.upstream_url.clone());
                route.service_id = None;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Timeout in seconds for upstream connections.
    #[serde(default = "default_upstream_timeout")]
    pub upstream_timeout_secs: u64,
    /// When a route uses `service_id` and the registry returns multiple instances.
    #[serde(default)]
    pub instance_selection: InstanceSelection,
    /// Max retry attempts on upstream failure (0 = no retry). Only retries on
    /// connection errors, not on successful HTTP error status codes.
    #[serde(default)]
    pub max_retries: u32,
    /// Circuit breaker: trips open after this many consecutive upstream
    /// failures per upstream host, then rejects requests for
    /// `circuit_breaker_recovery_secs`. 0 = disabled.
    #[serde(default)]
    pub circuit_breaker_threshold: u32,
    /// Seconds to keep the circuit breaker open before allowing a probe request.
    #[serde(default = "default_cb_recovery")]
    pub circuit_breaker_recovery_secs: u64,
    /// Ordered list of plugins to load into the proxy pipeline.
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
    /// Active health checking for upstream instances resolved via registries.
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
    /// Optional TLS configuration for HTTPS termination.
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            upstream_timeout_secs: default_upstream_timeout(),
            instance_selection: InstanceSelection::default(),
            max_retries: 0,
            circuit_breaker_threshold: 0,
            circuit_breaker_recovery_secs: default_cb_recovery(),
            plugins: Vec::new(),
            health_check: None,
            tls: None,
        }
    }
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_upstream_timeout() -> u64 { 30 }
fn default_cb_recovery() -> u64 { 30 }

/// How to choose one upstream when a `service_id` resolves to multiple instances.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceSelection {
    /// Always use the first instance in resolver order (previous behaviour).
    #[default]
    First,
    /// Rotate among instances per `service_id` using an in-memory counter.
    RoundRobin,
    /// Pick a random instance (uniform distribution).
    Random,
    /// Weighted round-robin using the `weight` metadata key on each instance
    /// (defaults to 1 when absent). Higher weight = more traffic share.
    WeightedRoundRobin,
}

// ---------------------------------------------------------------------------
// Registry configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct RegistriesConfig {
    /// How multiple registries are queried.
    #[serde(default)]
    pub query_mode: RegistryQueryMode,
    #[serde(default)]
    pub sources: Vec<RegistryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryQueryMode {
    /// Try registries in priority order; return first non-empty result.
    #[default]
    Priority,
    /// Query all registries concurrently and merge results.
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RegistryConfig {
    Nacos(NacosConfig),
    Eureka(EurekaConfig),
    Kubernetes(KubernetesConfig),
    Mock(MockRegistryConfig),
}

impl RegistryConfig {
    pub fn priority(&self) -> u32 {
        match self {
            RegistryConfig::Nacos(c) => c.priority,
            RegistryConfig::Eureka(c) => c.priority,
            RegistryConfig::Kubernetes(c) => c.priority,
            RegistryConfig::Mock(c) => c.priority,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NacosConfig {
    #[serde(default = "default_priority")]
    pub priority: u32,
    pub server_addr: String,
    pub namespace: Option<String>,
    pub group: Option<String>,
    pub auth: Option<NacosAuth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NacosAuth {
    pub username: String,
    pub password: String,
    #[serde(default = "default_token_refresh")]
    pub token_refresh_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EurekaConfig {
    #[serde(default = "default_priority")]
    pub priority: u32,
    pub server_url: String,
    /// Health check path appended to `server_url` for `doctor`/registry liveness.
    /// Defaults to `/apps` for Eureka-native availability checks.
    #[serde(default = "default_eureka_health_path")]
    pub health_path: String,
    pub auth: Option<BasicAuth>,
}

fn default_eureka_health_path() -> String {
    "/apps".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BasicAuth {
    pub username: String,
    pub password: String,
}

/// Kubernetes registry: Core `Endpoints`, fallback `EndpointSlice`, optional kubeconfig/TLS/auth.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct KubernetesConfig {
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Kubernetes API server URL. Defaults to the in-cluster address.
    ///
    /// When `kubeconfig_path` is set and this field keeps default value,
    /// server URL will be taken from the selected kubeconfig context.
    #[serde(default = "default_k8s_api_server")]
    pub api_server_url: String,
    pub namespace: Option<String>,
    /// Optional kubeconfig path used to load cluster CA, client cert/key, and token.
    pub kubeconfig_path: Option<String>,
    /// Optional kubeconfig context name. Defaults to kubeconfig `current-context`.
    pub kubeconfig_context: Option<String>,
    /// If true, disable TLS certificate validation for Kubernetes API calls.
    #[serde(default)]
    pub insecure_skip_tls_verify: bool,
    /// Comma-separated `key=value` label requirements AND-ed with
    /// `kubernetes.io/service-name=<service_id>` when listing **EndpointSlices** only.
    /// Core `Endpoints` discovery is unchanged. Example: `topology.kubernetes.io/zone=us-east-1a`.
    #[serde(default)]
    pub endpoint_slice_label_selector: Option<String>,
    pub auth: Option<K8sAuth>,
}

fn default_k8s_api_server() -> String {
    "https://kubernetes.default.svc".to_string()
}

/// Optional bearer token authentication for the Kubernetes API (`token` or `token_file`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct K8sAuth {
    /// Path to a ServiceAccount token file.
    pub token_file: Option<String>,
    /// Explicit bearer token (alternative to token_file).
    pub token: Option<String>,
}

/// How [`MockRegistry`] reports its own liveness in [`ServiceRegistry::health`].
///
/// Lets tests and local workflows simulate degraded/unhealthy registry without Nacos/Eureka.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MockRegistryHealthBehavior {
    #[default]
    Healthy,
    Degraded {
        #[serde(default)]
        message: String,
    },
    Unhealthy {
        #[serde(default)]
        message: String,
    },
}

/// In-memory mock registry for local development and CI tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MockRegistryConfig {
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Map of service_id -> instances
    #[serde(default)]
    pub services: HashMap<String, Vec<MockServiceInstance>>,
    /// Service IDs that resolve with a simulated registry failure (never return instances).
    #[serde(default)]
    pub error_services: HashMap<String, String>,
    #[serde(default)]
    pub health_behavior: MockRegistryHealthBehavior,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MockServiceInstance {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_priority() -> u32 { 100 }
fn default_token_refresh() -> u64 { 1800 }

// ---------------------------------------------------------------------------
// Routing rules
// ---------------------------------------------------------------------------

/// Path matching strategy for a routing rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PathMatcher {
    Exact { value: String },
    Prefix { value: String },
    Glob { value: String },
    Regex { value: String },
}

/// A single routing rule as loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RoutingRule {
    pub id: String,
    pub path: PathMatcher,
    /// HTTP methods this rule applies to. `None` means all methods.
    #[serde(default)]
    pub methods: Option<Vec<String>>,
    /// Request header matchers (all must match).
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    /// Target: a service ID resolved via the registry, OR a direct URL.
    pub service_id: Option<String>,
    /// Direct upstream URL, bypassing registry discovery.
    pub upstream_url: Option<String>,
    /// Strip this prefix from the path before forwarding.
    pub strip_prefix: Option<String>,
    /// Extra response headers for plain HTTP proxies only (applied after upstream
    /// headers; same-name entries override upstream). Ignored for WebSocket upgrades.
    #[serde(default)]
    pub response_headers: Option<HashMap<String, String>>,
    /// Higher priority rules are evaluated first (lower number = higher priority).
    #[serde(default = "default_rule_priority")]
    pub priority: u32,
}

/// Per-route override loaded from a local overlay file (`--local-override`).
/// Replaces the target of a matching route by `id` without modifying the main config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocalOverrideEntry {
    pub route_id: String,
    pub upstream_url: String,
}

/// Top-level shape of a local-override YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LocalOverrideFile {
    pub overrides: Vec<LocalOverrideEntry>,
}

fn default_rule_priority() -> u32 { 100 }

// ---------------------------------------------------------------------------
// Plugin configuration
// ---------------------------------------------------------------------------

/// Declares a single plugin to be loaded into the proxy pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PluginConfig {
    /// Unique name, also used as the lookup key for built-in plugins.
    pub name: String,
    /// Execution order (lower = earlier). Defaults to 100.
    #[serde(default = "default_plugin_order")]
    pub order: u32,
    /// Whether this plugin entry is active. Defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Opaque config blob forwarded to `PluginMiddleware::init`.
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_plugin_order() -> u32 { 100 }
fn default_enabled() -> bool { true }

// ---------------------------------------------------------------------------
// Health check configuration
// ---------------------------------------------------------------------------

/// Active health checking: periodically probes upstream instances and marks
/// them healthy/unhealthy so that `select_service_instance` can skip bad ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HealthCheckConfig {
    /// Seconds between probe rounds.
    #[serde(default = "default_hc_interval")]
    pub interval_secs: u64,
    /// HTTP path to GET on each instance (e.g. "/health").
    #[serde(default = "default_hc_path")]
    pub path: String,
    /// Per-probe timeout in seconds.
    #[serde(default = "default_hc_timeout")]
    pub timeout_secs: u64,
    /// Mark unhealthy after this many consecutive failed probes.
    #[serde(default = "default_hc_unhealthy_threshold")]
    pub unhealthy_threshold: u32,
    /// Mark healthy again after this many consecutive successful probes.
    #[serde(default = "default_hc_healthy_threshold")]
    pub healthy_threshold: u32,
}

// ---------------------------------------------------------------------------
// TLS configuration
// ---------------------------------------------------------------------------

/// HTTPS termination: provide paths to a PEM certificate chain and private key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

fn default_hc_interval() -> u64 { 10 }
fn default_hc_path() -> String { "/health".to_string() }
fn default_hc_timeout() -> u64 { 5 }
fn default_hc_unhealthy_threshold() -> u32 { 3 }
fn default_hc_healthy_threshold() -> u32 { 1 }
