# service-router

Microservice registry proxy with configurable routing, hot-reload, and developer-focused CLI diagnostics.

**Registry integrations (shipping):** Mock, Nacos, Eureka, Kubernetes. **HashiCorp Consul** is on the long-term backlog ([`docs/developer-roadmap-1-2y.md`](docs/developer-roadmap-1-2y.md) §4.1).

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
- `config-diff <left-config> <right-config> [--json | --markdown]` — structural diff after the same load rules as `check-config` (including `${ENV}` expansion). Exit **1** when there are differences (useful for CI gates comparing golden vs candidate YAML).
- `config-snapshot [config] [--config <path>] [-o|--output <path>]` — pretty-printed **redacted** JSON (`diagnostic_version` **1.0**): no registry secrets, strips HTTP(S)/WS URL userinfo, route header matcher **keys only**; default stdout, **`-o -`** stdout, otherwise write file.
- `doctor [<config>] [--config <path>] [--probe-upstream] [--json]` — prefer `--config path` for clarity; a bare path positional is accepted. **`--probe-upstream`** runs TCP checks on remote registry API endpoints (when not mock) and on each route’s upstream URL or registry-resolved instances; see `docs/doctor-json-schema.md` for `registry_endpoint_probe`.
- `route-explain [path] [method] [--config path] [--request-file path] [--header name:value …] [--json] [--verbose]` — with `--request-file`, load `path` / `method` / `headers` from YAML or `.json` (see `docs/route-explain-json-schema.md`, sample `config/route-explain-request-sample.yaml`); CLI `--header` overrides file keys. Unmatched runs print per-rule reasons and suggestions; text mode ends with a de-duplicated “Suggested actions” block; `--json` adds `remediation_outline` and optional `request_file`.

### Exit codes (`std::process::ExitCode`)

| Exit | When |
|:-----|:-----|
| `0` | Command succeeded (`run` exited cleanly, `help`, checks passed, `route-explain` matched a route, `doctor` overall PASS, **`config-diff` finds no differences**) |
| `1` | Any failure handled in-process: invalid/missing config, init errors, `--strict` findings, unmatched `route-explain`, upstream/registry probe failures in `doctor`, **`config-diff` finds differences**, top-level anyhow error |

Unknown subcommands print usage and exit `0` (same as explicit `help`).

`--json` mode still exits with code `1` when the logical outcome is failure (e.g. `doctor.status == "fail"`, `route-explain` unmatched). Scripts should parse JSON when stable signals are needed.

### Mock registry test scenarios

- Explicit **empty** instances: YAML `services: { mysvc: [] }`.
- Synthetic **resolver errors**: `error_services:` maps `service_id` → human message; resolves as `UnexpectedResponse`.
- **`health_behavior`**: `type: healthy` (default), `type: degraded` + `message`, or `type: unhealthy` + `message`; drives `doctor` registry health rows.

Example file: **`config/mock-scenarios-sample.yaml`**.

## Config Notes

- **`routes[].response_headers`**: optional map of extra **HTTP response** headers added after upstream response headers (same name overrides); validated at startup; **not** applied on WebSocket upgrades — see **`docs/plugin-extension.md`**.
- **`server.instance_selection`**: `first` (default) uses the first resolved instance for `service_id`; `round_robin` rotates per service id (in-memory counter; hot-reload can change mode).
- Default config path: `config/config.yaml`
- Mock development config: `config/mock-config.yaml`
- Mock behaviors sample: `config/mock-scenarios-sample.yaml` (patterns for empty/error/health overrides)
- `config/config.yaml` may require environment variables such as `NACOS_PASSWORD` before checks pass.
- Eureka note: `registries.sources[].health_path` is optional (default `/apps`) for health probing on non-standard deployments.
- Nacos note: `server_addr` accepts both base host (`http://host:port`) and suffix form (`http://host:port/nacos`).
- Kubernetes note: loads `Service` TCP `spec.ports[].targetPort` to narrow backend ports (skips UDP/SCTP for HTTP proxy), then resolves from Core `/api/v1/.../endpoints/{service}` first, then falls back to `/apis/discovery.k8s.io/v1/.../endpointslices?labelSelector=...` when Core returns empty (if the Service exists; otherwise no filter). The EndpointSlice `labelSelector` is always `kubernetes.io/service-name={service}`; optional config `endpoint_slice_label_selector` adds comma-separated `key=value` requirements AND-ed with that (EndpointSlice list only; Core `Endpoints` unchanged). EndpointSlice backends with `conditions.ready: false` or `conditions.serving: false` are skipped (unknown/`null` stays eligible). Configure `kubeconfig_path` (+ optional `kubeconfig_context`) for external clusters; keep `insecure_skip_tls_verify: false` unless troubleshooting.

## Roadmap & product docs

- `docs/developer-roadmap-1-2y.md` — developer-focused 1–2 year roadmap; **§4.1** long-term **config UI**; **§4.2** optional **traffic entry B** (port relay / OS forward into proxy) while keeping **explicit proxy port (A)** as default
- `docs/product-design-one-pager.md` — short product summary and near-term vs **long-term** bullets (config UI + optional entry B)
- `docs/product-design.md` — fuller design and improvement backlog
- `docs/implementation-status.md` — milestone alignment (M1/M2) and what shipped
- `docs/plugin-extension.md` — config-only **`response_headers`** slice vs future plugins (FR-6)
- `docs/m2-release-readiness.md` — M2 completion criteria vs repo evidence; **`bash scripts/verify-m2-baseline.sh`** or **`powershell -File scripts/verify-m2-baseline.ps1`** for Mock baseline (optional **`M2_WITH_DOCKER_PROBE`** / **`$env:M2_WITH_DOCKER_PROBE='1'`** to mirror CI compose probe)
- Optional Git commit template (no IDE footers): **`git config commit.template .gitmessage`** — see `.gitmessage`

## JSON Diagnostics Docs

- `docs/diagnostic-codes.md` — stable strings for metrics, doctor probes, route-explain, `/ready`
- `docs/check-config-strict-schema.md` — `--strict` finding `code` / `details` for `check-config --json`
- `docs/operations-runbook.md` — probes, metrics, config rollback, triage (no cluster naming assumptions)
- `docs/route-explain-json-schema.md`
- `docs/doctor-json-schema.md`
- Optional CI: `.github/workflows/release-acceptance.yml` (`workflow_dispatch`, JSON artifacts)
- `docs/metrics-json.md` — `GET /metrics` JSON + `/metrics/prometheus` text exposition
- `docs/release-acceptance-matrix.md` — pre-release regression checklist for Mock/Nacos/Eureka/Kubernetes (`bash docs/release-acceptance.sh` or `powershell -File docs/release-acceptance.ps1`); §9 regression archive summary template
- CI copy-paste: `docs/ci-template.md` · `docs/ci-copy-paste.sh` · `docs/doctor-probe-compose.sh`

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
