# Plugin Development Guide

This guide explains how to write and configure plugins for `service-router`.

## Architecture Overview

Plugins hook into the proxy pipeline at two points:

1. **`on_request`** -- runs before the request is forwarded upstream (in plugin order)
2. **`on_response`** -- runs after the upstream response is received (in reverse plugin order)

```
Client --> [plugin-1 on_request] --> [plugin-2 on_request] --> upstream
                                                                 |
Client <-- [plugin-1 on_response] <-- [plugin-2 on_response] <--'
```

## The `PluginMiddleware` Trait

Every plugin implements the `PluginMiddleware` async trait:

```rust
#[async_trait]
pub trait PluginMiddleware: Send + Sync {
    /// Unique name for this plugin.
    fn name(&self) -> &str;

    /// Called once when the plugin is loaded. Receives the `config` blob
    /// from the YAML entry.
    async fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        let _ = config;
        Ok(())
    }

    /// Called for every incoming request. Return `Continue(req)` to pass
    /// the (possibly modified) request to the next plugin, or `Respond(resp)`
    /// to short-circuit and skip upstream entirely.
    async fn on_request(&self, req: Request) -> Result<RequestAction, String> {
        Ok(RequestAction::Continue(req))
    }

    /// Called for every response before it reaches the client. Plugins
    /// run in reverse order here (last plugin first).
    async fn on_response(&self, resp: Response) -> Result<Response, String> {
        Ok(resp)
    }
}
```

## Writing a Plugin

### Minimal Example

```rust
use async_trait::async_trait;
use axum::extract::Request;
use axum::response::Response;
use service_router::server::{PluginMiddleware, RequestAction};

pub struct MyPlugin;

#[async_trait]
impl PluginMiddleware for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }

    async fn on_request(&self, req: Request) -> Result<RequestAction, String> {
        tracing::info!(path = %req.uri().path(), "my-plugin: request intercepted");
        Ok(RequestAction::Continue(req))
    }
}
```

### Short-Circuiting Requests

Return `RequestAction::Respond(resp)` to skip upstream forwarding entirely:

```rust
async fn on_request(&self, req: Request) -> Result<RequestAction, String> {
    if req.uri().path() == "/blocked" {
        let resp = Response::builder()
            .status(403)
            .body(axum::body::Body::from("Forbidden by plugin"))
            .unwrap();
        return Ok(RequestAction::Respond(resp));
    }
    Ok(RequestAction::Continue(req))
}
```

### Using Plugin Config

The `config` field from YAML is passed as `serde_json::Value` to `init()`:

```rust
struct RateLimiter {
    max_rps: u64,
}

#[async_trait]
impl PluginMiddleware for RateLimiter {
    fn name(&self) -> &str { "rate-limiter" }

    async fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        self.max_rps = config.get("max_rps")
            .and_then(|v| v.as_u64())
            .unwrap_or(100);
        Ok(())
    }
}
```

YAML config:

```yaml
server:
  plugins:
    - name: rate-limiter
      order: 10
      config:
        max_rps: 50
```

## Configuration Reference

Each plugin entry in `server.plugins[]`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | required | Plugin lookup key |
| `order` | integer | 100 | Execution priority (lower = earlier) |
| `enabled` | boolean | true | Set `false` to disable without removing |
| `config` | object | `{}` | Opaque config passed to `init()` |

## Built-in Plugins

### `request-logger`

Logs each request (method, path, URI) and response (status code) at INFO level.

```yaml
server:
  plugins:
    - name: request-logger
      order: 1
```

### `request-headers`

Injects configured headers into every upstream request. Use for auth tokens, tracing IDs, etc.

```yaml
server:
  plugins:
    - name: request-headers
      order: 10
      config:
        headers:
          x-api-key: "secret-token"
          x-trace-source: "service-router"
```

### `response-headers`

Adds or overrides headers on every response to the client. Use for security headers, CORS, etc.

```yaml
server:
  plugins:
    - name: response-headers
      order: 90
      config:
        headers:
          x-powered-by: "service-router"
          strict-transport-security: "max-age=31536000"
```

## Safety Guarantees

- **Panic isolation**: If a plugin panics, `PluginChain` catches it via `catch_unwind` and returns an error instead of crashing the proxy. The request returns 502 and the panic is logged.
- **Execution order**: `on_request` runs in ascending `order`; `on_response` runs in descending `order` (like middleware unwinding).
- **Error propagation**: Returning `Err(String)` from any hook aborts the chain and results in a 502 response with the error logged.

## Future: External Plugins (FR-6.3)

The current SDK supports built-in (compiled-in) plugins only. External plugin loading via `dlopen` (`.so`/`.dll`) is designed in [ADR 002](adr/002-fr6-plugin-sdk-design.md) and will be implemented when there is demand. The `PluginMiddleware` trait will remain the stable API contract for external plugins.
