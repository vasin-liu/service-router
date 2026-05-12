use std::sync::Arc;
use arc_swap::ArcSwap;
use tracing::debug;

use crate::config::model::AppConfig;
use crate::error::ConfigError;
use crate::routing::matcher::CompiledRoutingRule;

/// An immutable snapshot of all compiled routing rules, sorted by priority.
///
/// Stored inside an `ArcSwap` so it can be replaced atomically during
/// hot-reload without blocking in-flight requests.
#[derive(Debug)]
pub struct RouterSnapshot {
    /// Rules sorted ascending by `priority` (lower number = higher priority).
    pub rules: Vec<CompiledRoutingRule>,
}

impl RouterSnapshot {
    /// Build a snapshot from the current application config.
    pub fn from_config(config: &AppConfig) -> Result<Self, ConfigError> {
        let mut rules: Vec<CompiledRoutingRule> = config
            .routes
            .iter()
            .map(CompiledRoutingRule::compile)
            .collect::<Result<Vec<_>, _>>()?;

        // Stable sort: ties in priority preserve declaration order.
        rules.sort_by_key(|r| r.priority);

        Ok(Self { rules })
    }

    /// Find the first matching rule for the given request.
    ///
    /// Iterates rules in priority order (lowest priority number first) and
    /// returns the first one where all matchers pass.
    pub fn resolve<'a>(
        &'a self,
        path: &str,
        method: &str,
        headers: &http::HeaderMap,
    ) -> Option<&'a CompiledRoutingRule> {
        for rule in &self.rules {
            if rule.matches(path, method, headers) {
                debug!(rule_id = %rule.id, path = %path, "Route matched");
                return Some(rule);
            }
        }
        None
    }
}

/// Shared, hot-swappable router.
pub type SharedRouter = Arc<ArcSwap<RouterSnapshot>>;

/// Rebuild the router snapshot from a new config and swap it in atomically.
pub fn rebuild_router(
    shared_router: &SharedRouter,
    config: &AppConfig,
) -> Result<(), ConfigError> {
    let snapshot = RouterSnapshot::from_config(config)?;
    shared_router.store(Arc::new(snapshot));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{PathMatcher, RoutingRule, ServerConfig, RegistriesConfig};

    fn config_with_rules(rules: Vec<RoutingRule>) -> AppConfig {
        AppConfig {
            routes: rules,
            ..Default::default()
        }
    }

    #[test]
    fn resolves_by_priority() {
        let rules = vec![
            RoutingRule {
                id: "low".to_string(),
                path: PathMatcher::Prefix { value: "/api".to_string() },
                methods: None,
                headers: None,
                service_id: Some("svc-low".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 200,
            },
            RoutingRule {
                id: "high".to_string(),
                path: PathMatcher::Prefix { value: "/api/users".to_string() },
                methods: None,
                headers: None,
                service_id: Some("svc-high".to_string()),
                upstream_url: None,
                strip_prefix: None,
                response_headers: None,
                priority: 10,
            },
        ];

        let snapshot = RouterSnapshot::from_config(&config_with_rules(rules)).unwrap();
        let matched = snapshot.resolve("/api/users/123", "GET", &http::HeaderMap::new());
        assert_eq!(matched.unwrap().id, "high");
    }

    #[test]
    fn no_match_returns_none() {
        let snapshot = RouterSnapshot::from_config(&config_with_rules(vec![])).unwrap();
        assert!(snapshot.resolve("/unknown", "GET", &http::HeaderMap::new()).is_none());
    }
}
