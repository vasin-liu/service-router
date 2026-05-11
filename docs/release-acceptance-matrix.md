# Release Acceptance Matrix (M2 Baseline)

Use this checklist before release cut or environment rollout.  
Goal: verify the same command contracts across `Mock`, `Nacos`, `Eureka`, and `Kubernetes`.

Quick runner (generates JSON artifacts):

```bash
bash docs/release-acceptance.sh
```

PowerShell (Windows):

```powershell
powershell -ExecutionPolicy Bypass -File docs/release-acceptance.ps1
```

GitHub Actions: run **Actions → release-acceptance → Run workflow** (see `.github/workflows/release-acceptance.yml`; downloads `release-acceptance-json` artifact).

## 1) Global Gates (all profiles)

Run once per build artifact:

```bash
cargo check
cargo test -- --nocapture
```

Pass when both commands exit `0`.

When the router process is running (e.g. after rollout smoke), optionally assert readiness:

```bash
curl -sf "http://127.0.0.1:<port>/ready"
```

Expect HTTP **200** and JSON `"status":"ready"` when at least one registry is healthy or degraded. HTTP **503** with `"status":"not_ready"` means every registry reported unhealthy — do not send production traffic until registries recover or configuration is fixed.

## 2) Profile Matrix

| Profile | Config | Required secrets/env | Primary purpose |
|:--|:--|:--|:--|
| Mock | `config/mock-config.yaml` | none | Deterministic baseline in CI/local |
| Nacos | `config/config.yaml` (or env-specific copy) | `NACOS_PASSWORD` (if auth enabled) | Real registry auth + discovery |
| Eureka | env-specific config | Eureka credentials when enabled | Real registry auth + discovery |
| Kubernetes | env-specific config (`type: kubernetes`) | kubeconfig/token as configured | API health + endpoint discovery |

**Out of scope today:** HashiCorp **Consul** is not a registry type in this codebase; it is tracked as a future extension ([`developer-roadmap-1-2y.md`](./developer-roadmap-1-2y.md) §4.1, [`implementation-status.md`](./implementation-status.md)).

## 3) Command-Level Acceptance

For each profile, run:

```bash
cargo run -- check-config --config <CONFIG_PATH> --json --strict
cargo run -- doctor --config <CONFIG_PATH> --json
cargo run -- doctor --config <CONFIG_PATH> --probe-upstream --json
```

Minimum pass criteria:

- `check-config`: exit `0`, JSON `status == "ok"`, `strict_passed == true`.
- `doctor --json`: exit `0`, JSON `status == "pass"`.
- `doctor --probe-upstream --json`: exit `0`, JSON `status == "pass"`.

If a command fails, keep the JSON artifact and classify by:

- Config/schema issue (strict findings or parse/init failure)
- Registry auth/TLS issue
- Network reachability issue (`TCP_UNREACHABLE`)
- Route target resolution issue (`service_id` unresolved)

## 4) Route Smoke Gate

Pick one stable request per profile and assert route match:

```bash
cargo run -- route-explain <PATH> <METHOD> --config <CONFIG_PATH> --json
```

Pass when exit `0` and JSON `matched == true`.

Recommended stable samples:

- Mock: `/api/orders/123 GET` (matches `orders-api`)
- Nacos/Eureka/K8s: choose one service route guaranteed to exist in target env

## 5) Profile-Specific Notes

- Mock
  - Must be fully green in every pipeline.
  - Use as release blocker baseline even when external registries are flaky.
  - Default `config/mock-config.yaml` points both mock services at `127.0.0.1:9001` so `doctor --probe-upstream` only needs one reachable TCP port (see file comment to split ports again for two-upstream compose).

- Nacos
  - Validate both auth and discovery paths.
  - Confirm `server_addr` format is one of:
    - `http://host:port`
    - `http://host:port/nacos`

- Eureka
  - Confirm `health_path` is correct for deployment (default `/apps`).
  - Ensure one known service can resolve at least one healthy instance.

- Kubernetes
  - Confirm `doctor` health is green against API server.
  - Confirm one `service_id` resolves via Endpoints API in configured namespace.
  - If using kubeconfig, validate `kubeconfig_path` + optional `kubeconfig_context`.

## 6) Release Decision

Release can proceed when:

- Global gates pass.
- At least one deterministic profile (Mock) passes fully.
- Target environment profile(s) pass all command-level checks and route smoke.
- Any degraded/non-blocking item is documented with owner and mitigation.

## 7) Artifact Retention (recommended)

Store these per profile for each release candidate:

- `check-config.json`
- `doctor.json`
- `doctor-probe.json`
- `route-explain-smoke.json`
- `config-snapshot.json` (redacted routing/registry summary; written by `release-acceptance.sh` / `.ps1` since this repo revision)

Retention recommendation: keep at least last 10 release candidates.

## 8) Helper Script Inputs

`docs/release-acceptance.sh` supports:

- Produces **`config-snapshot.json`** (redacted) alongside the other §7 files when the script completes successfully.
- `SERVICE_ROUTER_CONFIG` (default `config/mock-config.yaml`)
- `SERVICE_ROUTER_SMOKE_PATH` (default `/api/orders/123`)
- `SERVICE_ROUTER_SMOKE_METHOD` (default `GET`)
- `SERVICE_ROUTER_ACCEPTANCE_OUT` (default `artifacts/release-acceptance`)
- `SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL` (`1` by default; set `0` to skip `cargo check/test`)
- `SERVICE_ROUTER_ACCEPTANCE_ALLOW_PROBE_FAIL` (`0` by default; set `1` to keep collecting artifacts when `doctor --probe-upstream` fails)

`docs/release-acceptance.ps1` reads the same environment variables.

## 9) Regression archive summary (for M2 / audit trail)

Use this short template after running §1–§4 for **each profile** you care about (`Mock` minimum; add `Nacos`, `Eureka`, `Kubernetes` when validating real environments). Paste into your ticket, wiki, or a **private** archive (do not commit secrets); store §7 JSON artifacts next to the summary. Copy-paste stub: [`docs/regression-archive/section-9-summary-template.md`](./regression-archive/section-9-summary-template.md). Workflow index: [`docs/regression-archive/README.md`](./regression-archive/README.md).

| Field | Example |
|:------|:--------|
| **Date / TZ** | `2026-05-10 Europe/Oslo` |
| **Git** | commit SHA / tag building the binary under test |
| **Router binary** | `service-router 0.1.0` or build ID |
| **Profile** | `Mock` / `Nacos` / `Eureka` / `Kubernetes` |
| **Config** | path or name only (no secrets); note env placeholders resolved |
| **Global gates** | §1: `cargo check` / `cargo test` — pass / fail |
| **CLI gates** | §3: `check-config --strict`, `doctor`, `doctor --probe-upstream` — pass / fail |
| **Route smoke** | §4: `route-explain` path/method — matched yes/no |
| **Config snapshot** | `config-snapshot.json` from same run — yes/no (redacted) |
| **Artifacts dir** | e.g. `artifacts/release-acceptance/` or CI artifact name |
| **Deviations** | e.g. `ALLOW_PROBE_FAIL=1`, flaky registry, known issue link |
| **Sign-off** | name or team |

**Minimum for M2 “四类回归”闭链:** at least one archived row for **Mock** (automation-friendly) and, when available, one row per additional registry type you deploy against. Linking the **release-acceptance** workflow artifact or the local `release-acceptance.sh` output directory satisfies the “JSON 产物” part of the release gate.
