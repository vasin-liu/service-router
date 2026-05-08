use crate::error::ConfigError;
use once_cell::sync::Lazy;
use regex::Regex;

static ENV_VAR_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{([^}]+)\}").expect("valid env-var regex"));

/// Replace all `${VAR_NAME}` placeholders in `value` with the corresponding
/// environment variable values.
///
/// Returns `Err(ConfigError::MissingEnvVar)` if any referenced variable is
/// not set in the environment.
pub fn resolve_env_vars(value: &str) -> Result<String, ConfigError> {
    let mut result = value.to_string();
    for cap in ENV_VAR_PATTERN.captures_iter(value) {
        let placeholder = &cap[0]; // e.g. "${NACOS_PASSWORD}"
        let var_name = &cap[1];    // e.g. "NACOS_PASSWORD"
        let var_value = std::env::var(var_name)
            .map_err(|_| ConfigError::MissingEnvVar(var_name.to_string()))?;
        result = result.replace(placeholder, &var_value);
    }
    Ok(result)
}

/// Walk the entire config YAML string and expand all env-var placeholders.
/// This is called before deserialisation so that the Serde layer never sees
/// raw `${...}` tokens.
pub fn resolve_env_vars_in_yaml(yaml: &str) -> Result<String, ConfigError> {
    resolve_env_vars(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_single_variable() {
        std::env::set_var("TEST_VAR_SINGLE", "hello");
        let result = resolve_env_vars("value is ${TEST_VAR_SINGLE}").unwrap();
        assert_eq!(result, "value is hello");
    }

    #[test]
    fn resolves_multiple_variables() {
        std::env::set_var("TEST_USER", "alice");
        std::env::set_var("TEST_PASS", "secret");
        let result = resolve_env_vars("${TEST_USER}:${TEST_PASS}").unwrap();
        assert_eq!(result, "alice:secret");
    }

    #[test]
    fn passthrough_without_placeholders() {
        let result = resolve_env_vars("plain-value").unwrap();
        assert_eq!(result, "plain-value");
    }

    #[test]
    fn missing_env_var_returns_error() {
        std::env::remove_var("TEST_MISSING_VAR");
        let err = resolve_env_vars("${TEST_MISSING_VAR}").unwrap_err();
        assert!(matches!(err, ConfigError::MissingEnvVar(name) if name == "TEST_MISSING_VAR"));
    }
}
