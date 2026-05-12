# Extension model (FR-6 engineering slice)

This document describes **what exists today** in the proxy for “extensibility without dynamic plugins,” and what is **explicitly out of scope** until a separate design passes review.

## Config-only response headers (HTTP)

Routing rules may set optional **`response_headers`**: a map of header names to values. When a plain HTTP request is proxied (not a WebSocket upgrade), the router applies these entries **after** copying safe headers from the upstream response. If a name duplicates an upstream header, the **configured value wins**.

- **Compile-time validation**: invalid token names or values are rejected when the router snapshot is built (same phase as regex/glob compilation). Headers that would break framing or are hop-by-hop (for example `Content-Length`, `Transfer-Encoding`, `Connection`, …) are **not** allowed in `response_headers`. This runs on **`service-router run`** startup, on config hot-reload, and on every **`check-config`** invocation (even without `--strict`); see [`check-config-strict-schema.md`](./check-config-strict-schema.md#route-compilation-all-check-config-runs).
- **WebSocket**: upgrade traffic uses the WebSocket proxy path; **`response_headers` is not applied** there. Matching WebSocket routing still uses path/method/header rules as before.

YAML example:

```yaml
routes:
  - id: add-trace-flag
    path:
      type: prefix
      value: /api
    service_id: my-service
    response_headers:
      x-gateway: service-router
```

## Plugin SDK (Phase C — in-process plugins)

The proxy now supports an in-process plugin pipeline via the `PluginMiddleware` async trait (`src/server/plugin.rs`). Plugins intercept the request **before** upstream forwarding (`on_request`) and the response **after** it returns (`on_response`). They execute in `order` sequence on request and in **reverse** order on response (middleware-stack semantics). A plugin may short-circuit the chain by returning a `RequestAction::Respond(...)` directly.

### Configuration

```yaml
server:
  plugins:
    - name: request-logger
      order: 10
      enabled: true
```

Each entry has:

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `name` | string | *required* | Lookup key for built-in plugins |
| `order` | u32 | 100 | Execution order (lower = earlier) |
| `enabled` | bool | true | Set to false to deactivate without removing |
| `config` | JSON value | `null` | Opaque blob passed to `PluginMiddleware::init` |

### Built-in plugins

| Name | Description | Config |
|:-----|:------------|:-------|
| `request-logger` | Logs every proxied request (method + URI) and response (status) at INFO level | none |
| `request-headers` | Injects configurable headers into every upstream request (auth tokens, trace IDs) | `{"headers": {"Authorization": "Bearer xxx"}}` |
| `response-headers` | Adds or overrides headers on every response returned to the caller (security headers, gateway tags) | `{"headers": {"X-Frame-Options": "DENY"}}` |

Example with all three:

```yaml
server:
  plugins:
    - name: request-logger
      order: 1
    - name: request-headers
      order: 10
      config:
        headers:
          Authorization: "Bearer ${AUTH_TOKEN}"
          X-Trace-Id: "auto"
    - name: response-headers
      order: 20
      config:
        headers:
          X-Gateway: service-router
          X-Frame-Options: DENY
```

### Architecture references

- Trait definition & chain: `src/server/plugin.rs`
- ADR 001 (initial deferral): [`adr/001-fr6-dynamic-plugins-deferred.md`](./adr/001-fr6-dynamic-plugins-deferred.md)
- ADR 002 (SDK design review): [`adr/002-fr6-plugin-sdk-design.md`](./adr/002-fr6-plugin-sdk-design.md)

### Future: external / dynamic plugins

PRD FR-6 Wasm/scripts, `dlopen`, and marketplace remain future work. The current in-process trait model is the foundation for the `dlopen` approach described in ADR 002. **`response_headers` remains a deliberately simple baseline** so small deployments never need the plugin SDK.
