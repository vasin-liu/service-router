# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Documentation

- README: Roadmap & product docs section linking `developer-roadmap` (§4.1 config UI), product design, and implementation status.

### Added

- `check-config --strict` structured findings: `StrictFinding` (`code`, `message`, optional `details`) in `src/config/strict_check.rs`; generator script `tools/emit_strict_check.mjs`; schema doc `docs/check-config-strict-schema.md`.
- Documentation: `docs/diagnostic-codes.md` (stable proxy metrics / doctor / route-explain codes) and `docs/operations-runbook.md` (readiness, rollback via hot-reload, triage checklist).
- Mock registry simulation hooks: `error_services`, `health_behavior` (healthy/degraded/unhealthy).
- Sample config `config/mock-scenarios-sample.yaml` for empty-list and error pathways.
- `GET /metrics` JSON snapshot: `route_hits` per rule id, `failure_reasons` by stable code (`docs/metrics-json.md`).

### Changed

- `check-config --json --strict`: `strict_findings` is now an array of objects `{ "code", "message", "details"? }` instead of strings; see `docs/check-config-strict-schema.md`.
- README: index entries for `docs/diagnostic-codes.md` and `docs/operations-runbook.md`.
- `GET /ready`: aggregates per-registry health via `MultiRegistryResolver::health_report()`; returns **503** when every registry is `unhealthy`, with JSON `registry_health` rows matching `doctor --json` (breaking for deployments that assumed `/ready` was always 200 with registries configured).
- README: CLI conventions, exit code table, and mock scenario documentation.
- `check-config --strict`: shadowing uses router evaluation order (sorted by priority); flags `upstream_url` + `service_id` together on a rule and Prefix matchers whose `strip_prefix` cannot apply.
- `route-explain` (unmatched): path/method/header-specific suggestion text, copy-pasteable `cargo run …` commands using the active `--config`, `RULE_HEADER_NAME_INVALID` for bad rule header keys, and JSON `remediation_outline` (one entry per suggestion `code`).
- CI: GitHub `ci.yml` adds smoke `route-explain`; `.gitlab-ci.yml` + `docs/ci-copy-paste.sh` + expanded `docs/ci-template.md` (GitHub/GitLab copy-paste).
- `doctor --probe-upstream`: TCP-probes Nacos/Eureka/Kubernetes registry endpoints (`registry_endpoint_probe` in JSON; `TCP_UNREACHABLE` / `ENDPOINT_PARSE_ERROR`); `parse_host_port_for_probe` accepts URL or `host:port`.
- `route-explain --request-file`: YAML/JSON request sample (`path`, optional `method`, optional `headers`); CLI headers override file; JSON includes `request_file` path when used; sample `config/route-explain-request-sample.yaml`.
- `doctor-probe` workflow now starts/stops dockerized mock upstreams (`.github/compose/doctor-probe.compose.yml`) so `doctor --probe-upstream` is deterministic on hosted runners.
- `doctor-probe` workflow adds manual inputs (`config_path`, `compose_file`) while preserving mock defaults.
- Added local helper `docs/doctor-probe-compose.sh` to run the same compose-backed probe flow outside CI.
- `doctor-probe` now uploads docker diagnostics artifact (`compose-ps.txt`, `compose-logs.txt`) when probe fails.
- Added `/metrics/prometheus` text endpoint (`service_router_route_hits_total`, `service_router_failures_total`) alongside JSON `/metrics`.
- Eureka registry health check now probes `/apps` (with auth) instead of `/info`, aligning health status with actual discovery availability.
- Eureka config adds optional `health_path` (default `/apps`) so health probing can adapt to custom endpoints.
- Nacos registry now normalizes `server_addr` so both `http://host:port` and `http://host:port/nacos` work without duplicate-path failures.
- `config/config.yaml` now documents both supported Nacos `server_addr` forms to prevent misconfiguration.
- Kubernetes registry now performs real endpoint discovery (`/api/v1/namespaces/{ns}/endpoints/{service}`) instead of returning empty instances.
- Kubernetes registry supports kubeconfig-backed TLS/auth (`kubeconfig_path`, optional `kubeconfig_context`) plus optional `insecure_skip_tls_verify`.
- Kubernetes registry falls back to `EndpointSlice` (`discovery.k8s.io/v1`, label `kubernetes.io/service-name`) when Core `Endpoints` yields no instances.
- Kubernetes discovery loads `Service.spec.ports` and filters Core/Slice backend ports to TCP `targetPort` (numeric or named), reducing cross-product noise for multi-port Services.
- Kubernetes port handling skips `SCTP` alongside `UDP` for Service targets and endpoint rows (HTTP proxy scope).
- `docs/doctor-json-schema.md` documents how Kubernetes resolution affects `upstream_probe` for `service_id`.
- GitHub Actions: `release-acceptance` manual workflow (`.github/workflows/release-acceptance.yml`) runs `docs/release-acceptance.sh` with optional compose-backed mock upstreams and uploads JSON artifacts.
- GitLab CI: optional manual job `release-acceptance-manual` runs the same release script and saves JSON artifacts (`allow_failure: true`; `SERVICE_ROUTER_ACCEPTANCE_ALLOW_PROBE_FAIL=1` without Docker compose).
- Docs: `product-design.md` / `product-design-one-pager.md` aligned with Kubernetes Endpoints-based discovery and next-step EndpointSlice work.
- Mock profile: `api-gateway` mock instance now uses `127.0.0.1:9001` (same as `order-service`) so `doctor --probe-upstream` passes with a single local upstream; CI template note updated accordingly.

## [0.1.0] - 2026-05-08

### Added

- Developer-oriented CLI commands:
  - `check-config [config] [--json] [--strict]`
  - `doctor [config] [--config <path>]`
  - `route-explain <path> [method] --config <path> --header "key:value" [--json] [--verbose]`
- Strict config checks:
  - Empty routes warning
  - Duplicate route ID detection
  - Identical matcher detection
  - Catch-all shadowing hint (`prefix "/"`)
- Route explanation diagnostics:
  - Match output with target and rewritten path
  - Non-match reason details for path/method/header checks
  - JSON output support for automation
- Mock registry (minimal viable):
  - New registry source type `mock`
  - In-config service instance mapping (`service_id -> instances`)
  - Local test/dev profile file: `config/mock-config.yaml`
- Registry health reporting in `doctor`:
  - Per-registry health detail (`healthy` / `degraded` / `unhealthy`)
- Test coverage additions:
  - Mock registry unit tests
  - Strict check tests in CLI entry
  - Route mismatch explanation test

### Changed

- CLI command help text expanded to include JSON/strict/verbose options.
- Build dependency updated with `async-trait`.

### Fixed

- Fixed async `Send` issue in router hot-reload task.
- Fixed unsupported `Hash` derive for `ServiceInstance` metadata map.
- Cleaned up warnings (unused imports/variables) in touched modules.
