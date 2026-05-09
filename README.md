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

- `run [config]` — omit `config` to use `config/config.yaml`
- `check-config [<config>] [--json] [--strict]` — any non-flag token is treated as the config path; if you pass multiple, the **last** one wins (flags can be mixed before/after the path depending on iteration order; prefer `[--json] [--strict] config.yaml`). With `--strict`, findings include overshadowing computed in router evaluation order (`priority`, then YAML order ties), simultaneous `upstream_url` + `service_id` on a rule, and Prefix rules whose `strip_prefix` cannot apply to matched paths.
- `doctor [<config>] [--config <path>] [--probe-upstream] [--json]` — prefer `--config path` for clarity; a bare path positional is accepted
- `route-explain <path> [method] [--config path] [--header name:value …] [--json] [--verbose]` — unmatched runs print per-rule reasons and suggestions; text mode ends with a de-duplicated “Suggested actions” block; `--json` adds `remediation_outline` (one suggestion per `code`).

### Exit codes (`std::process::ExitCode`)

| Exit | When |
|:-----|:-----|
| `0` | Command succeeded (`run` exited cleanly, `help`, checks passed, `route-explain` matched a route, `doctor` overall PASS) |
| `1` | Any failure handled in-process: invalid/missing config, init errors, `--strict` findings, unmatched `route-explain`, upstream/registry probe failures in `doctor`, top-level anyhow error |

Unknown subcommands print usage and exit `0` (same as explicit `help`).

`--json` mode still exits with code `1` when the logical outcome is failure (e.g. `doctor.status == "fail"`, `route-explain` unmatched). Scripts should parse JSON when stable signals are needed.

### Mock registry test scenarios

- Explicit **empty** instances: YAML `services: { mysvc: [] }`.
- Synthetic **resolver errors**: `error_services:` maps `service_id` → human message; resolves as `UnexpectedResponse`.
- **`health_behavior`**: `type: healthy` (default), `type: degraded` + `message`, or `type: unhealthy` + `message`; drives `doctor` registry health rows.

Example file: **`config/mock-scenarios-sample.yaml`**.

## Config Notes

- Default config path: `config/config.yaml`
- Mock development config: `config/mock-config.yaml`
- Mock behaviors sample: `config/mock-scenarios-sample.yaml` (patterns for empty/error/health overrides)
- `config/config.yaml` may require environment variables such as `NACOS_PASSWORD` before checks pass.

## JSON Diagnostics Docs

- `docs/route-explain-json-schema.md`
- `docs/doctor-json-schema.md`

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
