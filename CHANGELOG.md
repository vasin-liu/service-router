# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Added

- **`scripts/check-text-encoding.py`** + GitHub CI step: fails on NUL bytes in text-like files to prevent accidental UTF-16LE source/doc/script commits; failure output includes a UTF-16LE-to-UTF-8 repair example.
- **CI parity**: GitLab, `docs/ci-copy-paste.sh`, and `scripts/verify-m2-baseline.*` also run the text encoding guard.
- **Release acceptance parity**: `docs/release-acceptance.sh` / `.ps1` run the text encoding guard as part of global gates; matrix docs list it.
- **`docs/regression-archive/`**: §9 summary template + workflow README for M2 audit trail.
- **`scripts/post-deploy-smoke.sh`** / **`.ps1`**: minimal **`GET /health`** + **`/ready`** after rollout (`SERVICE_ROUTER_BASE_URL`).
- **`docs/config-snapshot-workflow.md`**, **`docs/next-engineering-priorities.md`**, **`docs/adr/`** (ADR 001: FR-6 dynamic plugins deferred).
- **`release-acceptance.sh`** / **`.ps1`**: emit **`config-snapshot.json`** next to other §7 artifacts.
- **CI parity**: `.github/workflows/ci.yml`, **`.gitlab-ci.yml`**, **`docs/ci-copy-paste.sh`**, **`scripts/verify-m2-baseline.*`** run **`config-snapshot -o -`** on the mock profile after `route-explain`; **`docs/ci-template.md`** updated (table + GitHub/GitLab excerpts + `ci.yml` step list).

- **Library tests** (`routing::matcher`): YAML load + **`RouterSnapshot::from_config`** covers forbidden vs valid **`response_headers`** (same compile path as **`check-config`** / **`run`**).

- **`proxy_http` integration test** (`tokio` + local TCP stub): asserts **`extra_response_headers`** are applied and override same-name upstream headers.

- **`config_snapshot_export`** test: route **`response_header_keys`** in JSON must not echo header **values**.
- Mock profile **`config/mock-config.yaml`**: `orders-api` sets example **`response_headers`** for CI smoke / copy-paste demos.
- **`route-explain`** text mode (matched): prints **Outbound response headers** when the rule defines any.

- Routing rules: optional **`response_headers`** (YAML map); merged onto downstream **HTTP** responses after upstream headers, same-name entries override upstream; forbidden hop-by-hop / framing headers rejected at compile time; **ignored for WebSocket** upgrades (`docs/plugin-extension.md`). **`route-explain --json`** (matched envelope) adds **`response_headers`**; **`config-snapshot`** route rows add **`response_header_keys`** when set.
- CLI **`config-snapshot`**: emit redacted JSON (`diagnostic_version` **1.0** + UUID `snapshot_id`) for issue/PR paste; **`--output` / `-o`** file or stdout with **`-`**; logic in **`service_router::config_snapshot_export`** (`src/lib.rs`) to avoid UTF-16 source pitfalls on Windows editors.
- CLI **`config-diff`**: compare two YAML configs after load (`server`, `log_level`, `registries`, routes by `id`); **`--json`** (`diagnostic_version` **1.0**) or **`--markdown`** for PR blurbs; exit **1** on differences.
- **`tools/emit_diff_rs.py`**: regenerates UTF-8 `src/config/diff.rs` on environments where UTF-16-encoded sources break `rustc` (optional maintenance aid).

### Documentation

- **`.github/workflows/release-acceptance.yml`**: header comment lists artifact bundle; **README** Quick Start adds optional **`config-snapshot`**; **`release-acceptance-matrix.md`** notes **`release-acceptance-json`** includes **`config-snapshot.json`**.
- **`ci-template.md`**, **`.gitlab-ci.yml`**, **`config-snapshot-workflow.md`**: spell out **`release-acceptance`** / manual job artifact filenames (incl. **`config-snapshot.json`**).
- **`docs/regression-archive/`**: README lists five JSON files + GitHub **`release-acceptance-json`** / GitLab paths; §9 template adds file checklist.
- **`m2-release-readiness.md`**: optional JSON bundle subsection lists five §7 filenames + **`RUN_GLOBAL=0`** examples; **`verify-m2-baseline.*`** prints a tip to run **`release-acceptance`** for artifacts; anchor **`#m2-json-bundle-s9`** for deep links.
- **`implementation-status.md`**: Markdown links to **`m2-release-readiness.md#m2-json-bundle-s9`** from current状态、M2 表与待业务侧完成段落。
- **`plugin-extension.md`**: FR-6 config-only slice (`response_headers`) vs future dynamic plugins; link **ADR 001**.
- **`route-explain-json-schema.md`**: matched envelope **`response_headers`** field.
- **`diagnostic-codes.md`**: note on matched **`response_headers`**.
- **`ci-template.md`**: smoke `route-explain` row notes mock **`response_headers`** on `orders-api`.
- **`product-prd-developer.md`**: FR-6 节增加与本仓库 **`response_headers`** 工程切片及 **`plugin-extension.md`** 的对照说明。
- **`check-config-strict-schema.md`**, **`operations-runbook.md`**, **`plugin-extension.md`**, **README**: clarify that **`check-config` always compiles routes** (including **`response_headers`** validation).
- **`implementation-status.md`**: M3 table — FR-5.1–FR-5.3 engineering slice (FR-5.3 de-scoped to redacted JSON export; hosted share links out of repo); FR-6 partial row; **「本次已落地」/「验证结果」/「下一版本迭代进展」** 与当前 CLI 及 **`cargo test`** 对齐；新增 **「M3 已交付能力清单」** 表；**「下一阶段建议」** 指向 **`regression-archive/`** 与 **`config-snapshot.json`**。
- **`next-iteration-backlog.md`**: replaced stale sprint text with post-M3 backlog pointers.
- **`release-acceptance-matrix.md`**, **`m2-release-readiness.md`**, **`operations-runbook.md`**, **README**: link regression archive, post-deploy smoke, **`config-snapshot`** artifact.
- **`.gitmessage`**: UTF-8 commit template discouraging IDE/tool footers (optional `git config commit.template .gitmessage`).
- `docs/m2-release-readiness.md`: **Engineering closure (M2)** subsection and criterion mapping.
- README: **`config-diff`**; optional **`git config commit.template .gitmessage`**.
- Consul deferred: noted in `developer-roadmap-1-2y.md` §4.1, `implementation-status.md` (远期注册中心), `product-design-one-pager.md`, `release-acceptance-matrix.md` (out of scope), `README.md`.
- `docs/operations-runbook.md`: post-deployment checklist (§7), Prometheus alerting hooks vs `failure_reasons` (§8), binary upgrade notes under config rollback (§3); UTF-8 encoding normalized.
- `docs/ci-template.md`: document compose-backed `doctor --probe-upstream` steps in `ci.yml`.
- `docs/release-acceptance-matrix.md`: §9 regression archive summary table (M2 / audit trail) and cross-link from implementation-status.
- README: index note for release-acceptance §9 archive template.

### Changed

- Kubernetes registry: `debug!` logs on resolve path (Core Endpoints vs EndpointSlice fallback, per-`service_id` counts) when `RUST_LOG=service_router::registry::k8s=debug`; `trace!` logs each discovery GET URL (Service, Endpoints, EndpointSlice list); `docs/operations-runbook.md` §6 notes `debug` vs `trace`.
- `KubernetesConfig` / `K8sAuth` rustdoc: remove outdated stub wording (registry is implemented).
- CI: `.github/workflows/ci.yml` runs Docker Compose mock upstreams then `doctor --probe-upstream --json` after smoke `route-explain`, matching release acceptance networking gates for mock profile.
- GitLab CI: `.gitlab-ci.yml` `rust-validate` adds Docker-in-Docker and the same compose + `doctor --probe-upstream` sequence (`after_script` tears down compose).
- `doctor --probe-upstream --json` / `upstream_probe` rows: when a probe or resolution fails, include stable **`failure_code`** (`TCP_UNREACHABLE`, `ENDPOINT_PARSE_ERROR`, `no_instances`, or `registry_*` from `metrics::failure_code_for_registry`) so automation can align with `GET /metrics` `failure_reasons`.
- `docs/diagnostic-codes.md`: triage cross-reference table and `upstream_probe` code list; file encoding normalized to UTF-8 for reliable viewing.
- `docs/doctor-json-schema.md`: document optional `failure_code` on `upstream_probe` items.
- Kubernetes EndpointSlice parsing: omit backends with `conditions.serving: false` (in addition to `ready: false`); aligns with discovery.k8s.io/v1 readiness for new connections.
- Doctor schema doc: EndpointSlice row filter notes `serving` alongside `ready`.
- Operations runbook: short table mapping `failure_reasons` spikes to checks; file normalized to UTF-8.
- README / product one-pager: Kubernetes EndpointSlice `serving: false` omission documented.

### Documentation

- README: Roadmap & product docs section linking `developer-roadmap` (§4.1 config UI), product design, and implementation status.
- Roadmap: `docs/developer-roadmap-1-2y.md` §4.2 — keep explicit proxy-port entry (model A); treat transparent/local port relay into the proxy (model B) as long-term optional. Product one-pager, `product-design.md`, `implementation-status` cross-reference.

### Added

- **`server.instance_selection`**: `first` (default, prior behaviour) or `round_robin` when a `service_id` resolves to multiple instances; per-service atomic counter in `AppState`.
- GitHub Actions: `.github/workflows/release-acceptance.yml` (`workflow_dispatch`) runs `docs/release-acceptance.sh` with optional compose-backed mock upstreams, uploads JSON artifacts (`release-acceptance-json`), optional `skip_compose` (`yes` skips Docker and relaxes probe failure); aligns docs/README references that previously described a missing workflow.
- Kubernetes config: optional `endpoint_slice_label_selector` — comma-separated label requirements AND-ed with `kubernetes.io/service-name=<service_id>` when listing EndpointSlices (Core `Endpoints` discovery unchanged).
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
