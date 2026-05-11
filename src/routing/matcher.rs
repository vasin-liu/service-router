use std::collections::HashMap;
use glob::Pattern as GlobPattern;
use regex::Regex;

use crate::config::model::{PathMatcher, RoutingRule};
use crate::error::ConfigError;
use crate::proxy::http_proxy::is_forbidden_config_response_header;

/// A routing rule with path matchers pre-compiled for efficient matching.
#[derive(Debug)]
pub struct CompiledRoutingRule {
    pub id: String,
    pub compiled_path: CompiledPath,
    pub methods: Option<Vec<String>>,
    pub headers: Option<HashMap<String, String>>,
    pub service_id: Option<String>,
    pub upstream_url: Option<String>,
    pub strip_prefix: Option<String>,
    /// Compiled `response_headers` for HTTP downstream responses (`None` if unset).
    pub response_headers: Option<Vec<(http::HeaderName, http::HeaderValue)>>,
    pub priority: u32,
}

#[derive(Debug)]
pub enum CompiledPath {
    Exact(String),
    Prefix(String),
    Glob(GlobPattern),
    Regex(Regex),
}

impl CompiledPath {
    pub fn matches(&self, path: &str) -> bool {
        match self {
            CompiledPath::Exact(s) => s == path,
            CompiledPath::Prefix(prefix) => path.starts_with(prefix.as_str()),
            CompiledPath::Glob(pattern) => pattern.matches(path),
            CompiledPath::Regex(re) => re.is_match(path),
        }
    }
}

impl CompiledRoutingRule {
    /// Compile a raw `RoutingRule` from the config model.
    pub fn compile(rule: &RoutingRule) -> Result<Self, ConfigError> {
        let compiled_path = match &rule.path {
            PathMatcher::Exact { value } => CompiledPath::Exact(value.clone()),
            PathMatcher::Prefix { value } => CompiledPath::Prefix(value.clone()),
            PathMatcher::Glob { value } => {
                let pattern =
                    GlobPattern::new(value).map_err(|e| ConfigError::InvalidGlob {
                        route_id: rule.id.clone(),
                        pattern: value.clone(),
                        reason: e.to_string(),
                    })?;
                CompiledPath::Glob(pattern)
            }
            PathMatcher::Regex { value } => {
                let re = Regex::new(value).map_err(|e| ConfigError::InvalidRegex {
                    route_id: rule.id.clone(),
                    pattern: value.clone(),
                    reason: e.to_string(),
                })?;
                CompiledPath::Regex(re)
            }
        };

        let response_headers = match &rule.response_headers {
            None => None,
            Some(m) if m.is_empty() => None,
            Some(m) => Some(compile_response_headers(&rule.id, m)?),
        };

        Ok(Self {
            id: rule.id.clone(),
            compiled_path,
            methods: rule.methods.clone(),
            headers: rule.headers.clone(),
            service_id: rule.service_id.clone(),
            upstream_url: rule.upstream_url.clone(),
            strip_prefix: rule.strip_prefix.clone(),
            response_headers,
            priority: rule.priority,
        })
    }

    /// Returns `true` if this rule matches the given request context.
    pub fn matches(
        &self,
        path: &str,
        method: &str,
        request_headers: &http::HeaderMap,
    ) -> bool {
        // 1. Path match
        if !self.compiled_path.matches(path) {
            return false;
        }

        // 2. Method match (if specified)
        if let Some(methods) = &self.methods {
            let upper = method.to_uppercase();
            if !methods.iter().any(|m| m.to_uppercase() == upper) {
                return false;
            }
        }

        // 3. Header match (all configured headers must be present with matching values)
        if let Some(req_headers) = &self.headers {
            for (name, expected_value) in req_headers {
                let header_name = match http::header::HeaderName::from_bytes(name.as_bytes()) {
                    Ok(n) => n,
                    Err(_) => return false,
                };
                match request_headers.get(&header_name) {
                    Some(actual) => {
                        if actual.to_str().unwrap_or("") != expected_value {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }

        true
    }

    /// Rewrite the incoming path according to `strip_prefix`, if configured.
    pub fn rewrite_path<'a>(&self, path: &'a str) -> std::borrow::Cow<'a, str> {
        if let Some(prefix) = &self.strip_prefix {
            if let Some(stripped) = path.strip_prefix(prefix.as_str()) {
                let rewritten = if stripped.is_empty() { "/" } else { stripped };
                return std::borrow::Cow::Owned(rewritten.to_string());
            }
        }
        std::borrow::Cow::Borrowed(path)
    }
}

fn compile_response_headers(
    route_id: &str,
    map: &HashMap<String, String>,
) -> Result<Vec<(http::HeaderName, http::HeaderValue)>, ConfigError> {
    let mut out = Vec::with_capacity(map.len());
    for (name_raw, val_raw) in map {
        if is_forbidden_config_response_header(name_raw) {
            return Err(ConfigError::Validation(format!(
                "route '{route_id}' response_headers: '{name_raw}' is not allowed (hop-by-hop or body-framing)"
            )));
        }
        let name = http::header::HeaderName::from_bytes(name_raw.as_bytes()).map_err(|_| {
            ConfigError::Validation(format!(
                "route '{route_id}' response_headers: invalid header name '{name_raw}'"
            ))
        })?;
        let value = http::header::HeaderValue::from_str(val_raw).map_err(|_| {
            ConfigError::Validation(format!(
                "route '{route_id}' response_headers: invalid value for '{name_raw}'"
            ))
        })?;
        out.push((name, value));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::PathMatcher;

    fn make_rule(id: &str, path: PathMatcher) -> RoutingRule {
        RoutingRule {
            id: id.to_string(),
            path,
            methods: None,
            headers: None,
            service_id: Some("svc".to_string()),
            upstream_url: None,
            strip_prefix: None,
            response_headers: None,
            priority: 100,
        }
    }

    #[test]
    fn exact_match() {
        let rule = make_rule("r1", PathMatcher::Exact { value: "/api/v1".to_string() });
        let compiled = CompiledRoutingRule::compile(&rule).unwrap();
        assert!(compiled.compiled_path.matches("/api/v1"));
        assert!(!compiled.compiled_path.matches("/api/v1/extra"));
    }

    #[test]
    fn prefix_match() {
        let rule = make_rule("r2", PathMatcher::Prefix { value: "/api".to_string() });
        let compiled = CompiledRoutingRule::compile(&rule).unwrap();
        assert!(compiled.compiled_path.matches("/api/users"));
        assert!(compiled.compiled_path.matches("/api"));
        assert!(!compiled.compiled_path.matches("/other"));
    }

    #[test]
    fn glob_match() {
        let rule = make_rule("r3", PathMatcher::Glob { value: "/api/*/list".to_string() });
        let compiled = CompiledRoutingRule::compile(&rule).unwrap();
        assert!(compiled.compiled_path.matches("/api/users/list"));
        assert!(!compiled.compiled_path.matches("/api/users/detail"));
    }

    #[test]
    fn regex_match() {
        let rule = make_rule(
            "r4",
            PathMatcher::Regex { value: r"^/api/users/\d+$".to_string() },
        );
        let compiled = CompiledRoutingRule::compile(&rule).unwrap();
        assert!(compiled.compiled_path.matches("/api/users/123"));
        assert!(!compiled.compiled_path.matches("/api/users/abc"));
    }

    #[test]
    fn invalid_regex_returns_error() {
        let rule = make_rule("r5", PathMatcher::Regex { value: "[invalid".to_string() });
        assert!(CompiledRoutingRule::compile(&rule).is_err());
    }

    #[test]
    fn strip_prefix_rewrite() {
        let mut rule = make_rule("r6", PathMatcher::Prefix { value: "/api".to_string() });
        rule.strip_prefix = Some("/api".to_string());
        let compiled = CompiledRoutingRule::compile(&rule).unwrap();
        assert_eq!(compiled.rewrite_path("/api/users"), "/users");
        assert_eq!(compiled.rewrite_path("/api"), "/");
        assert_eq!(compiled.rewrite_path("/other"), "/other");
    }

    #[test]
    fn response_headers_compiled() {
        let mut rule = make_rule("rh", PathMatcher::Prefix { value: "/".to_string() });
        rule.response_headers = Some(HashMap::from([(
            "x-out".to_string(),
            "ok".to_string(),
        )]));
        let compiled = CompiledRoutingRule::compile(&rule).unwrap();
        let pairs = compiled.response_headers.as_ref().unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0.as_str(), "x-out");
        assert_eq!(pairs[0].1.to_str().unwrap(), "ok");
    }

    #[test]
    fn forbidden_response_header_rejected() {
        let mut rule = make_rule("bad-rh", PathMatcher::Prefix { value: "/".to_string() });
        rule.response_headers = Some(HashMap::from([(
            "Content-Length".to_string(),
            "0".to_string(),
        )]));
        assert!(CompiledRoutingRule::compile(&rule).is_err());
    }
}
