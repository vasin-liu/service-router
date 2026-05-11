pub mod config;
pub mod error;
pub mod proxy;
pub mod registry;
pub mod routing;
pub mod server;

/// FR-5.3: redacted YAML-derived JSON for attaching to tickets (no secrets in output).
pub mod config_snapshot_export {
    pub use inner::{build_config_snapshot_export, ConfigSnapshotExport};

    mod inner {
        use super::super::config::model::{
            AppConfig, MockRegistryConfig, MockRegistryHealthBehavior, PathMatcher, RegistryConfig,
            RegistriesConfig, RoutingRule, ServerConfig,
        };
        use std::collections::HashMap;
        use std::path::Path;

        use serde::Serialize;
        use uuid::Uuid;

        #[derive(Debug, Clone, Serialize)]
        pub struct ConfigSnapshotExport {
            pub diagnostic_version: &'static str,
            pub snapshot_id: String,
            pub config_basename: String,
            pub log_level: String,
            pub server: ServerConfig,
            pub registries: RegistriesSnapshot,
            pub routes: Vec<RouteSnapshotRow>,
        }

        #[derive(Debug, Clone, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub struct RegistriesSnapshot {
            pub query_mode: super::super::config::model::RegistryQueryMode,
            pub sources: Vec<RegistrySnapshotRow>,
        }

        #[derive(Debug, Clone, Serialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        pub enum RegistrySnapshotRow {
            Nacos {
                priority: u32,
                server_addr: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                namespace: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                group: Option<String>,
                auth_configured: bool,
            },
            Eureka {
                priority: u32,
                server_url: String,
                health_path: String,
                auth_configured: bool,
            },
            Kubernetes {
                priority: u32,
                api_server_url: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                namespace: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                kubeconfig_basename: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                kubeconfig_context: Option<String>,
                insecure_skip_tls_verify: bool,
                #[serde(skip_serializing_if = "Option::is_none")]
                endpoint_slice_label_selector: Option<String>,
                bearer_auth_configured: bool,
                token_file_configured: bool,
            },
            Mock {
                priority: u32,
                #[serde(skip_serializing_if = "HashMap::is_empty")]
                service_instance_counts: HashMap<String, usize>,
                #[serde(skip_serializing_if = "Vec::is_empty")]
                error_service_ids: Vec<String>,
                health_behavior: String,
            },
        }

        #[derive(Debug, Clone, Serialize)]
        pub struct RouteSnapshotRow {
            pub id: String,
            pub priority: u32,
            pub path_kind: String,
            pub path_pattern: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub methods: Option<Vec<String>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub header_keys: Option<Vec<String>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub service_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub upstream_url_redacted: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub strip_prefix: Option<String>,
        }

        pub fn build_config_snapshot_export(config: &AppConfig, config_yaml_path: &Path) -> ConfigSnapshotExport {
            let basename = config_yaml_path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "(unknown)".to_string());

            ConfigSnapshotExport {
                diagnostic_version: "1.0",
                snapshot_id: Uuid::new_v4().to_string(),
                config_basename: basename,
                log_level: config.log_level.clone(),
                server: config.server.clone(),
                registries: registries_snapshot(&config.registries),
                routes: config.routes.iter().map(route_snapshot_row).collect(),
            }
        }

        fn registries_snapshot(rc: &RegistriesConfig) -> RegistriesSnapshot {
            RegistriesSnapshot {
                query_mode: rc.query_mode.clone(),
                sources: rc.sources.iter().map(registry_snapshot_row).collect(),
            }
        }

        fn registry_snapshot_row(src: &RegistryConfig) -> RegistrySnapshotRow {
            match src {
                RegistryConfig::Nacos(c) => RegistrySnapshotRow::Nacos {
                    priority: c.priority,
                    server_addr: redact_http_credentials(&c.server_addr),
                    namespace: c.namespace.clone(),
                    group: c.group.clone(),
                    auth_configured: c.auth.is_some(),
                },
                RegistryConfig::Eureka(c) => RegistrySnapshotRow::Eureka {
                    priority: c.priority,
                    server_url: redact_http_credentials(&c.server_url),
                    health_path: c.health_path.clone(),
                    auth_configured: c.auth.is_some(),
                },
                RegistryConfig::Kubernetes(c) => RegistrySnapshotRow::Kubernetes {
                    priority: c.priority,
                    api_server_url: redact_http_credentials(&c.api_server_url),
                    namespace: c.namespace.clone(),
                    kubeconfig_basename: c.kubeconfig_path.as_ref().and_then(|p| {
                        Path::new(p)
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                    }),
                    kubeconfig_context: c.kubeconfig_context.clone(),
                    insecure_skip_tls_verify: c.insecure_skip_tls_verify,
                    endpoint_slice_label_selector: c.endpoint_slice_label_selector.clone(),
                    bearer_auth_configured: c.auth.as_ref().and_then(|a| a.token.as_ref()).is_some(),
                    token_file_configured: c.auth.as_ref().and_then(|a| a.token_file.as_ref()).is_some(),
                },
                RegistryConfig::Mock(c) => RegistrySnapshotRow::Mock {
                    priority: c.priority,
                    service_instance_counts: mock_service_counts(c),
                    error_service_ids: {
                        let mut ids: Vec<String> = c.error_services.keys().cloned().collect();
                        ids.sort();
                        ids
                    },
                    health_behavior: mock_health_kind(&c.health_behavior),
                },
            }
        }

        fn mock_service_counts(c: &MockRegistryConfig) -> HashMap<String, usize> {
            c.services
                .iter()
                .map(|(k, v)| (k.clone(), v.len()))
                .collect()
        }

        fn mock_health_kind(h: &MockRegistryHealthBehavior) -> String {
            match h {
                MockRegistryHealthBehavior::Healthy => "healthy".to_string(),
                MockRegistryHealthBehavior::Degraded { .. } => "degraded".to_string(),
                MockRegistryHealthBehavior::Unhealthy { .. } => "unhealthy".to_string(),
            }
        }

        fn route_snapshot_row(rule: &RoutingRule) -> RouteSnapshotRow {
            let (path_kind, path_pattern) = path_parts(&rule.path);
            let header_keys = rule.headers.as_ref().map(|h| {
                let mut ks: Vec<String> = h.keys().cloned().collect();
                ks.sort();
                ks
            });
            RouteSnapshotRow {
                id: rule.id.clone(),
                priority: rule.priority,
                path_kind,
                path_pattern,
                methods: rule.methods.clone(),
                header_keys,
                service_id: rule.service_id.clone(),
                upstream_url_redacted: rule
                    .upstream_url
                    .as_ref()
                    .map(|u| redact_http_credentials(u)),
                strip_prefix: rule.strip_prefix.clone(),
            }
        }

        fn path_parts(pm: &PathMatcher) -> (String, String) {
            match pm {
                PathMatcher::Exact { value } => ("exact".to_string(), value.clone()),
                PathMatcher::Prefix { value } => ("prefix".to_string(), value.clone()),
                PathMatcher::Glob { value } => ("glob".to_string(), value.clone()),
                PathMatcher::Regex { value } => ("regex".to_string(), value.clone()),
            }
        }

        fn redact_http_credentials(s: &str) -> String {
            match url::Url::parse(s) {
                Ok(mut u) if matches!(u.scheme(), "http" | "https" | "ws" | "wss") => {
                    let _ = u.set_username("");
                    let _ = u.set_password(None);
                    u.to_string()
                }
                _ => s.to_string(),
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::config::load_config;
            use std::io::Write;

            #[test]
            fn snapshot_omits_nacos_password_and_url_userinfo() {
                let yaml = r#"
server:
  host: "127.0.0.1"
  port: 8080
registries:
  sources:
    - type: nacos
      priority: 1
      server_addr: "http://localhost:8848"
      auth:
        username: u
        password: SUPER_SECRET_XY
routes:
  - id: r1
    path:
      type: exact
      value: /
    upstream_url: "http://user:passw99@127.0.0.1:9200/ping"
"#;
                let mut t = tempfile::NamedTempFile::new().unwrap();
                write!(t, "{yaml}").unwrap();
                t.flush().unwrap();
                let c = load_config(t.path()).unwrap();
                let s = build_config_snapshot_export(&c, t.path());
                let js = serde_json::to_string(&s).unwrap();
                assert!(!js.contains("SUPER_SECRET"), "{js}");
                assert!(!js.contains("passw99"));
                assert!(js.contains("\"auth_configured\":true"));
            }
        }
    }
}
