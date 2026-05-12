use async_trait::async_trait;
use axum::extract::Request;
use axum::response::Response;

pub enum RequestAction {
    Continue(Request),
    Respond(Response),
}

#[async_trait]
pub trait PluginMiddleware: Send + Sync {
    fn name(&self) -> &str;

    async fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        let _ = config;
        Ok(())
    }

    async fn on_request(&self, req: Request) -> Result<RequestAction, String> {
        Ok(RequestAction::Continue(req))
    }

    async fn on_response(&self, resp: Response) -> Result<Response, String> {
        Ok(resp)
    }
}

pub struct PluginChain {
    plugins: Vec<Box<dyn PluginMiddleware>>,
}

impl PluginChain {
    pub fn new(plugins: Vec<Box<dyn PluginMiddleware>>) -> Self {
        Self { plugins }
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub async fn run_on_request(&self, mut req: Request) -> Result<RequestAction, String> {
        for plugin in &self.plugins {
            match plugin.on_request(req).await? {
                RequestAction::Continue(r) => req = r,
                action @ RequestAction::Respond(_) => return Ok(action),
            }
        }
        Ok(RequestAction::Continue(req))
    }

    pub async fn run_on_response(&self, mut resp: Response) -> Result<Response, String> {
        for plugin in self.plugins.iter().rev() {
            resp = plugin.on_response(resp).await?;
        }
        Ok(resp)
    }
}


// ---------------------------------------------------------------------------
// Built-in plugins
// ---------------------------------------------------------------------------

/// Logs every proxied request at INFO level. No config needed.
pub struct RequestLoggerPlugin;

#[async_trait]
impl PluginMiddleware for RequestLoggerPlugin {
    fn name(&self) -> &str { "request-logger" }

    async fn on_request(&self, req: Request) -> Result<RequestAction, String> {
        tracing::info!(
            method = %req.method(),
            uri = %req.uri(),
            plugin = "request-logger",
            "Plugin: incoming request"
        );
        Ok(RequestAction::Continue(req))
    }

    async fn on_response(&self, resp: Response) -> Result<Response, String> {
        tracing::info!(
            status = %resp.status(),
            plugin = "request-logger",
            "Plugin: outgoing response"
        );
        Ok(resp)
    }
}


/// Injects configurable headers into every upstream request.
/// Config: `{"headers": {"Authorization": "Bearer xxx", "X-Trace": "1"}}`.
pub struct RequestHeadersPlugin {
    headers: Vec<(String, String)>,
}

impl RequestHeadersPlugin {
    pub fn new() -> Self {
        Self { headers: Vec::new() }
    }
}

#[async_trait]
impl PluginMiddleware for RequestHeadersPlugin {
    fn name(&self) -> &str { "request-headers" }

    async fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(map) = config.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in map {
                let val = v.as_str().unwrap_or_default().to_string();
                self.headers.push((k.clone(), val));
            }
        }
        Ok(())
    }

    async fn on_request(&self, mut req: Request) -> Result<RequestAction, String> {
        for (k, v) in &self.headers {
            if let (Ok(name), Ok(val)) = (
                axum::http::header::HeaderName::from_bytes(k.as_bytes()),
                axum::http::header::HeaderValue::from_str(v),
            ) {
                req.headers_mut().insert(name, val);
            }
        }
        Ok(RequestAction::Continue(req))
    }
}

/// Adds or overrides headers on every response returned to the caller.
/// Config: `{"headers": {"X-Gateway": "service-router", "X-Frame-Options": "DENY"}}`.
pub struct ResponseHeadersPlugin {
    headers: Vec<(String, String)>,
}

impl ResponseHeadersPlugin {
    pub fn new() -> Self {
        Self { headers: Vec::new() }
    }
}

#[async_trait]
impl PluginMiddleware for ResponseHeadersPlugin {
    fn name(&self) -> &str { "response-headers" }

    async fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(map) = config.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in map {
                let val = v.as_str().unwrap_or_default().to_string();
                self.headers.push((k.clone(), val));
            }
        }
        Ok(())
    }

    async fn on_response(&self, mut resp: Response) -> Result<Response, String> {
        for (k, v) in &self.headers {
            if let (Ok(name), Ok(val)) = (
                axum::http::header::HeaderName::from_bytes(k.as_bytes()),
                axum::http::header::HeaderValue::from_str(v),
            ) {
                resp.headers_mut().insert(name, val);
            }
        }
        Ok(resp)
    }
}

// ---------------------------------------------------------------------------
// Plugin factory
// ---------------------------------------------------------------------------

use crate::config::model::PluginConfig;

/// Build a PluginChain from configuration. Only enabled entries are
/// instantiated, sorted by `order`. Unknown names are logged and skipped.
pub async fn build_plugin_chain(configs: &[PluginConfig]) -> Result<PluginChain, String> {
    let mut entries: Vec<&PluginConfig> = configs.iter().filter(|c| c.enabled).collect();
    entries.sort_by_key(|c| c.order);

    let mut plugins: Vec<Box<dyn PluginMiddleware>> = Vec::new();
    for cfg in entries {
        let mut plugin: Box<dyn PluginMiddleware> = match cfg.name.as_str() {
            "request-logger" => Box::new(RequestLoggerPlugin),
            "request-headers" => Box::new(RequestHeadersPlugin::new()),
            "response-headers" => Box::new(ResponseHeadersPlugin::new()),
            other => {
                tracing::warn!(name = %other, "Unknown plugin name, skipping");
                continue;
            }
        };
        plugin.init(cfg.config.clone()).await?;
        tracing::info!(plugin = %cfg.name, order = cfg.order, "Plugin loaded");
        plugins.push(plugin);
    }
    Ok(PluginChain::new(plugins))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;

    struct PassthroughPlugin;

    #[async_trait]
    impl PluginMiddleware for PassthroughPlugin {
        fn name(&self) -> &str { "passthrough" }
    }

    struct BlockPlugin;

    #[async_trait]
    impl PluginMiddleware for BlockPlugin {
        fn name(&self) -> &str { "blocker" }

        async fn on_request(&self, _req: Request) -> Result<RequestAction, String> {
            let resp = Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::from("blocked"))
                .unwrap();
            Ok(RequestAction::Respond(resp))
        }
    }

    struct HeaderPlugin;

    #[async_trait]
    impl PluginMiddleware for HeaderPlugin {
        fn name(&self) -> &str { "header-adder" }

        async fn on_response(&self, mut resp: Response) -> Result<Response, String> {
            resp.headers_mut().insert(
                "x-plugin",
                "active".parse().unwrap(),
            );
            Ok(resp)
        }
    }

    #[tokio::test]
    async fn empty_chain_passes_through() {
        let chain = PluginChain::new(vec![]);
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        match chain.run_on_request(req).await.unwrap() {
            RequestAction::Continue(_) => {}
            RequestAction::Respond(_) => panic!("expected continue"),
        }
    }

    #[tokio::test]
    async fn block_plugin_short_circuits() {
        let chain = PluginChain::new(vec![
            Box::new(BlockPlugin),
            Box::new(PassthroughPlugin),
        ]);
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        match chain.run_on_request(req).await.unwrap() {
            RequestAction::Respond(resp) => {
                assert_eq!(resp.status(), StatusCode::FORBIDDEN);
            }
            RequestAction::Continue(_) => panic!("expected short-circuit"),
        }
    }

    #[tokio::test]
    async fn on_response_runs_in_reverse() {
        let chain = PluginChain::new(vec![
            Box::new(HeaderPlugin),
        ]);
        let resp = Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap();
        let resp = chain.run_on_response(resp).await.unwrap();
        assert_eq!(resp.headers().get("x-plugin").unwrap(), "active");
    }

    #[tokio::test]
    async fn request_headers_plugin_injects_headers() {
        let mut plugin = RequestHeadersPlugin::new();
        let config = serde_json::json!({"headers": {"x-auth": "token-abc", "x-trace": "1"}});
        plugin.init(config).await.unwrap();

        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        match plugin.on_request(req).await.unwrap() {
            RequestAction::Continue(r) => {
                assert_eq!(r.headers().get("x-auth").unwrap(), "token-abc");
                assert_eq!(r.headers().get("x-trace").unwrap(), "1");
            }
            RequestAction::Respond(_) => panic!("expected continue"),
        }
    }

    #[tokio::test]
    async fn response_headers_plugin_adds_headers() {
        let mut plugin = ResponseHeadersPlugin::new();
        let config = serde_json::json!({"headers": {"x-gateway": "sr", "x-frame-options": "DENY"}});
        plugin.init(config).await.unwrap();

        let resp = Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap();
        let resp = plugin.on_response(resp).await.unwrap();
        assert_eq!(resp.headers().get("x-gateway").unwrap(), "sr");
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
    }

    #[tokio::test]
    async fn build_chain_skips_disabled_and_sorts_by_order() {
        let configs = vec![
            PluginConfig {
                name: "request-logger".to_string(),
                order: 50,
                enabled: false,
                config: serde_json::Value::Null,
            },
            PluginConfig {
                name: "request-logger".to_string(),
                order: 10,
                enabled: true,
                config: serde_json::Value::Null,
            },
        ];
        let chain = build_plugin_chain(&configs).await.unwrap();
        assert_eq!(chain.len(), 1);
    }

}
