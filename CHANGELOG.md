# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Added

- Mock registry simulation hooks: `error_services`, `health_behavior` (healthy/degraded/unhealthy).
- Sample config `config/mock-scenarios-sample.yaml` for empty-list and error pathways.
- `GET /metrics` JSON snapshot: `route_hits` per rule id, `failure_reasons` by stable code (`docs/metrics-json.md`).

### Changed

- README: CLI conventions, exit code table, and mock scenario documentation.
- `check-config --strict`: shadowing uses router evaluation order (sorted by priority); flags `upstream_url` + `service_id` together on a rule and Prefix matchers whose `strip_prefix` cannot apply.
- `route-explain` (unmatched): path/method/header-specific suggestion text, copy-pasteable `cargo run …` commands using the active `--config`, `RULE_HEADER_NAME_INVALID` for bad rule header keys, and JSON `remediation_outline` (one entry per suggestion `code`).
- CI: GitHub `ci.yml` adds smoke `route-explain`; `.gitlab-ci.yml` + `docs/ci-copy-paste.sh` + expanded `docs/ci-template.md` (GitHub/GitLab copy-paste).
- `doctor --probe-upstream`: TCP-probes Nacos/Eureka/Kubernetes registry endpoints (`registry_endpoint_probe` in JSON; `TCP_UNREACHABLE` / `ENDPOINT_PARSE_ERROR`); `parse_host_port_for_probe` accepts URL or `host:port`.
- `route-explain --request-file`: YAML/JSON request sample (`path`, optional `method`, optional `headers`); CLI headers override file; JSON includes `request_file` path when used; sample `config/route-explain-request-sample.yaml`.
- `doctor-probe` workflow now starts/stops dockerized mock upstreams (`.github/compose/doctor-probe.compose.yml`) so `doctor --probe-upstream` is deterministic on hosted runners.

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
