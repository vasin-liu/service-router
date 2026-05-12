# ADR 002: FR-6 dynamic plugin SDK — design review

## Status

Proposed (design review, no implementation yet).

## Context

ADR 001 shipped a **configuration-only** extension (`response_headers`) and deferred dynamic plugin loading until security, observability, and upgrade requirements were reviewed. This ADR captures that review.

### Requirements derived from PRD FR-6

| Sub-FR | Requirement | Notes |
|:-------|:------------|:------|
| FR-6.1 | Plugin lifecycle: load, init, reload, unload | Must survive config hot-reload |
| FR-6.2 | Official plugin examples | At least request/response header manipulation |
| FR-6.3 | Template/plugin distribution | Discoverable, versionable |

### Constraints

- The proxy is written in Rust (Axum + Tokio). Plugins must not block the async runtime.
- Config hot-reload via `ArcSwap` must remain atomic — plugins must tolerate snapshot swaps.
- `response_headers` (ADR 001) is the config-only baseline. Dynamic plugins augment it, not replace it.

## Decision

### 1. Plugin interface: Tower middleware (Rust trait)

Plugins implement a `PluginMiddleware` trait with two hooks:

```rust
#[async_trait]
pub trait PluginMiddleware: Send + Sync {
    /// Called once after loading. Return Err to prevent activation.
    async fn init(&mut self, config: serde_json::Value) -> Result<(), String>;

    /// Intercept a request before upstream forwarding.
    /// Return Ok(None) to pass through, Ok(Some(response)) to short-circuit.
    async fn on_request(&self, req: &mut Request) -> Result<Option<Response>, String>;

    /// Intercept a response before returning to the caller.
    async fn on_response(&self, resp: &mut Response) -> Result<(), String>;
}
```

**Rationale**: Tower-compatible middleware is idiomatic for Axum. Trait objects (`Box<dyn PluginMiddleware>`) loaded at startup keep the async runtime clean. WASM was evaluated but rejected for now due to async FFI complexity and limited ecosystem maturity for HTTP middleware in `wasmtime`.

### 2. Plugin lifecycle

```
                   ┌────────┐
          load()   │ Loaded │
    ──────────────>│        │
                   └───┬────┘
                       │ init(config)
                       ▼
                   ┌────────┐
                   │ Active │◄────── hot-reload: re-init with new config
                   └───┬────┘
                       │ unload() or error
                       ▼
                   ┌──────────┐
                   │ Inactive │
                   └──────────┘
```

- **load**: `dlopen` a shared library (`.so` / `.dll` / `.dylib`) exporting a `create_plugin() -> Box<dyn PluginMiddleware>` symbol.
- **init**: Called with the plugin's config section (`server.plugins[].config` in YAML). Failure prevents activation.
- **hot-reload**: On config change, if the plugin config section changed, `init` is called again. The old plugin instance handles in-flight requests until the new one is ready.
- **unload**: Plugin is dropped. No explicit destructor hook beyond Rust `Drop`.

### 3. Configuration shape

```yaml
server:
  plugins:
    - name: "rate-limiter"
      path: "./plugins/librate_limiter.so"
      order: 10           # lower = earlier in chain
      config:
        max_requests_per_sec: 100
    - name: "auth-check"
      path: "./plugins/libauth_check.so"
      order: 5
      config:
        jwks_url: "https://auth.example.com/.well-known/jwks.json"
```

- `server.plugins[]` is optional (empty by default — no behavioral change).
- `order` determines execution sequence (lower first).
- `config` is passed as `serde_json::Value` to `init`.

### 4. Security

- Plugins are **trusted code** (loaded via `dlopen`, same address space). This is intentional for a developer-oriented proxy. WASM sandboxing may be added in a future ADR if untrusted plugins become a requirement.
- Plugin libraries must be on the local filesystem; no remote download at runtime.
- `check-config --strict` will validate that declared plugin paths exist and are loadable.

### 5. Observability

- Plugin `on_request` / `on_response` calls are wrapped with `tracing::span` (plugin name + order).
- Failures in plugin hooks are logged as `warn!` and increment `failure_reasons["plugin_error"]` in `/metrics`.
- Plugins do **not** short-circuit the circuit breaker — upstream failures are still tracked independently.

### 6. Distribution (FR-6.3)

Deferred to a separate ADR. Initial plugins will be built in-tree as Rust crate examples:

- `plugins/example-headers/` — adds custom response headers (demonstrating `on_response`)
- `plugins/example-auth/` — validates a bearer token (demonstrating `on_request` short-circuit)

A template market or registry is out of scope for this ADR.

## Consequences

- Teams can write Rust-native plugins and `dlopen` them without modifying the proxy binary.
- The `response_headers` config-only path (ADR 001) remains the simplest option for header injection.
- WASM plugins are explicitly deferred — this decision should be revisited if cross-language plugin authoring becomes a requirement.
- Plugin interface versioning will be needed when the trait changes; initial version is `plugin_api_version = 1`.

## Implementation plan

1. Define `plugin_api` crate with `PluginMiddleware` trait + `create_plugin` symbol convention.
2. Add `server.plugins[]` config parsing to `model.rs`.
3. Add plugin loading in `run_server` (after config load, before Axum app build).
4. Wire plugin chain into `proxy_handler` between route match and upstream forwarding.
5. Ship two example plugins as in-tree Cargo workspace members.
6. Update `check-config --strict` to validate plugin paths.

**Estimated effort**: Medium — primarily plumbing; the trait and lifecycle are straightforward. The main risk is `dlopen` cross-platform testing (Linux `.so`, macOS `.dylib`, Windows `.dll`).
