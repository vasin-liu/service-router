# service-router

Microservice registry proxy with configurable routing, hot-reload, and developer-focused CLI diagnostics.

## Quick Start (10 minutes, mock mode)

### 1) Validate config

```bash
cargo run -- check-config config/mock-config.yaml --json --strict
```

Expected: `strict_passed: true`.

### 2) Explain route matching

```bash
cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json
```

Expected: matched rule `orders-api`.

### 3) Run the proxy

```bash
cargo run -- run config/mock-config.yaml
```

Server listens on `127.0.0.1:8080` by default in mock config.

## CLI Commands

- `run [config]`
- `check-config [config] [--json] [--strict]`
- `doctor [config] [--config <path>]`
- `route-explain <path> [method] --config <path> --header "key:value" [--json]`

## Config Notes

- Default config path: `config/config.yaml`
- Mock development config: `config/mock-config.yaml`
- `config/config.yaml` may require environment variables such as `NACOS_PASSWORD` before checks pass.

## Migrate to Mock Dev Mode

If your current setup uses `config/config.yaml` with external registries, use this quick migration for local development:

1. Run validation against mock profile:

```bash
cargo run -- check-config config/mock-config.yaml --json --strict
```

2. Validate a representative route:

```bash
cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json --verbose
```

3. Start with mock profile:

```bash
cargo run -- run config/mock-config.yaml
```

4. Keep production profile unchanged and switch by command argument:

```bash
cargo run -- run config/config.yaml
```
