use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    FileNotFound(String),

    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config YAML: {0}")]
    ParseError(String),

    #[error("Missing required environment variable: ${0}")]
    MissingEnvVar(String),

    #[error("Invalid regex pattern '{pattern}' in route '{route_id}': {reason}")]
    InvalidRegex {
        route_id: String,
        pattern: String,
        reason: String,
    },

    #[error("Invalid glob pattern '{pattern}' in route '{route_id}': {reason}")]
    InvalidGlob {
        route_id: String,
        pattern: String,
        reason: String,
    },

    #[error("Route '{0}' must specify either service_id or upstream_url")]
    MissingTarget(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("HTTP error communicating with registry: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Registry returned unexpected response: {0}")]
    UnexpectedResponse(String),

    #[error("Service not found in registry: {0}")]
    ServiceNotFound(String),

    #[error("Registry authentication failed")]
    AuthFailed,

    #[error("All registries failed to resolve service '{0}'")]
    AllFailed(String),
}

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("No healthy instances found for service: {0}")]
    NoInstances(String),

    #[error("Upstream connection error: {0}")]
    UpstreamConnection(String),

    #[error("Registry error: {0}")]
    Registry(#[from] RegistryError),

    #[error("WebSocket upgrade error: {0}")]
    WsUpgrade(String),

    #[error("Request body read error: {0}")]
    BodyRead(String),

    #[error("Plugin error: {0}")]
    PluginError(String),
}

impl axum::response::IntoResponse for ProxyError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        let (status, msg) = match &self {
            ProxyError::NoInstances(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ProxyError::Registry(RegistryError::ServiceNotFound(_)) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            _ => (StatusCode::BAD_GATEWAY, self.to_string()),
        };
        (status, msg).into_response()
    }
}
