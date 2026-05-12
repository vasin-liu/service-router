# Getting Started

This guide walks you through a complete end-to-end workflow: from generating a config to successfully proxying your first request. Estimated time: **5 minutes**.

## Prerequisites

- Rust toolchain (`cargo` and `rustc`)
- No external services required (uses built-in mock registry)

## Step 1: Generate a starter config

```bash
cargo run -- init --template mock
```

This creates `config/config.yaml` with a mock registry and sample routes. If you already have a config file, skip this step.

## Step 2: Validate the config

```bash
cargo run -- check-config config/config.yaml --strict
```

Expected output: no errors, `strict_passed: true`. Add `--json` for machine-readable output.

## Step 3: Test route matching (dry run)

Before starting the proxy, verify your routes resolve correctly:

```bash
cargo run -- route-explain /api/orders/123 GET --config config/config.yaml
```

This shows which rule matched, its priority, upstream target, and any response headers. If no rule matches, it explains why (path/method/header mismatch) and suggests fixes.

## Step 4: Start the proxy

```bash
cargo run -- run config/config.yaml
```

The proxy starts on `127.0.0.1:8080` (default). You should see:

```
INFO Listening on 127.0.0.1:8080
```

## Step 5: Send a request

In another terminal:

```bash
curl -i http://127.0.0.1:8080/api/orders/123
```

The proxy matches the request to a route, resolves the upstream via the mock registry, forwards the request, and returns the response. The `x-request-id` header in the response can be used to trace the request in logs.

## What's next?

### Local debugging

```bash
# Check why a request doesn't match any route
cargo run -- route-explain /unknown/path GET --config config/config.yaml --verbose

# View environment and registry health
cargo run -- doctor --config config/config.yaml --json
```

### Redirect a route to your local service

Create a file `local-override.yaml`:

```yaml
overrides:
  - route_id: orders-api
    upstream_url: http://localhost:3000
```

Then start with the override:

```bash
cargo run -- run config/config.yaml --local-override local-override.yaml
```

### CI smoke test

Run a single request through the proxy and verify the status code, all in one command:

```bash
cargo run -- smoke-proxy config/config.yaml --request /api/orders/123 --expect-status 200
```

### Compare configs before deploying

```bash
cargo run -- config-diff config/config-staging.yaml config/config-prod.yaml --markdown
```

### Connect to a real registry

Switch from mock to a real registry by editing `config/config.yaml` or generating a new template:

```bash
cargo run -- init --template nacos    # or eureka, k8s
```

Then set the required environment variables (e.g. `NACOS_PASSWORD`) and run.

## Key concepts

| Concept | Description |
|:--------|:------------|
| **Route** | A rule matching requests by path, method, and headers to an upstream |
| **Registry** | Service discovery backend (Mock, Nacos, Eureka, Kubernetes) |
| **Plugin** | Request/response interceptor configured via `server.plugins[]` |
| **Hot reload** | Config changes are picked up automatically without restart |

## Further reading

- [CLI Commands](../README.md) -- full command reference
- [Plugin Extension](plugin-extension.md) -- plugin SDK and built-in plugins
- [Operations Runbook](operations-runbook.md) -- monitoring, rollback, troubleshooting
- [Developer Roadmap](developer-roadmap-1-2y.md) -- 1-2 year product roadmap
