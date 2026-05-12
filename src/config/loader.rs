use std::path::Path;
use crate::config::env_resolver::resolve_env_vars_in_yaml;
use crate::config::model::{AppConfig, LocalOverrideFile, LocalOverrideEntry};
use crate::error::ConfigError;

/// Load and validate `AppConfig` from a YAML file.
///
/// Steps:
/// 1. Read the raw YAML text.
/// 2. Expand `${ENV_VAR}` placeholders.
/// 3. Deserialise with serde_yaml.
/// 4. Run semantic validation.
pub fn load_config(path: impl AsRef<Path>) -> Result<AppConfig, ConfigError> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ConfigError::FileNotFound(path.display().to_string())
        } else {
            ConfigError::ReadError(e)
        }
    })?;

    let expanded = resolve_env_vars_in_yaml(&raw)?;

    let config: AppConfig = serde_yaml::from_str(&expanded)
        .map_err(|e| ConfigError::ParseError(e.to_string()))?;

    validate_config(&config)?;

    Ok(config)
}

/// Load local-override entries from a YAML file.
pub fn load_local_overrides(path: impl AsRef<Path>) -> Result<Vec<LocalOverrideEntry>, ConfigError> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ConfigError::FileNotFound(path.display().to_string())
        } else {
            ConfigError::ReadError(e)
        }
    })?;
    let expanded = resolve_env_vars_in_yaml(&raw)?;
    let file: LocalOverrideFile = serde_yaml::from_str(&expanded)
        .map_err(|e| ConfigError::ParseError(format!("local-override: {e}")))?;
    Ok(file.overrides)
}

fn validate_config(config: &AppConfig) -> Result<(), ConfigError> {
    use crate::config::model::PathMatcher;

    for rule in &config.routes {
        // Each rule must have a target.
        if rule.service_id.is_none() && rule.upstream_url.is_none() {
            return Err(ConfigError::MissingTarget(rule.id.clone()));
        }

        // Pre-validate regex / glob patterns so that errors surface at startup,
        // not at request time.
        match &rule.path {
            PathMatcher::Regex { value } => {
                regex::Regex::new(value).map_err(|e| ConfigError::InvalidRegex {
                    route_id: rule.id.clone(),
                    pattern: value.clone(),
                    reason: e.to_string(),
                })?;
            }
            PathMatcher::Glob { value } => {
                glob::Pattern::new(value).map_err(|e| ConfigError::InvalidGlob {
                    route_id: rule.id.clone(),
                    pattern: value.clone(),
                    reason: e.to_string(),
                })?;
            }
            _ => {}
        }
    }

    Ok(())
}
