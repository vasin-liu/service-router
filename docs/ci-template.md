# CI Template (Developer Baseline)

## What to run (`check-config` + `doctor` + smoke)

Use the **same three gates** everywhere: strict config validation, environment/diagnostic sanity, then a deterministic **route match** probe.

| Step | Command | Pass when |
|:-----|:--------|:----------|
| Build & unit tests | `cargo check`, `cargo test -- --nocapture` | Exit `0` |
| Strict config | `cargo run -- check-config config/mock-config.yaml --json --strict` | Exit `0`, JSON `strict_passed == true` if you parse it |
| Doctor | `cargo run -- doctor --config config/mock-config.yaml --json` | Exit `0`, JSON top-level `status` is typically `"pass"` for healthy mock runs |
| Doctor + network | `cargo run -- doctor --config config/mock-config.yaml --probe-upstream --json` | Exit `0` if no TCP/registry/upstream failures; JSON includes `registry_endpoint_probe` (remote registries) and `upstream_probe` |
| Smoke (route explain) | `cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json` | Exit `0`, JSON `matched == true` against mock profile |

The smoke route matches `orders-api` in `config/mock-config.yaml`; change path/method if your golden config differs (`SERVICE_ROUTER_*` vars in the shell snippet below).

## Standalone shell snippet

Repository file: **`docs/ci-copy-paste.sh`** — run after Rust toolchain install and checkout:

```bash
bash docs/ci-copy-paste.sh
```

Or paste the commands from that file into any job. Override defaults:

```bash
export SERVICE_ROUTER_CONFIG=config/mock-config.yaml
export SERVICE_ROUTER_SMOKE_PATH=/api/orders/123
export SERVICE_ROUTER_SMOKE_METHOD=GET
bash docs/ci-copy-paste.sh
```

## GitHub Actions (this repo)

`.github/workflows/ci.yml` mirrors the table above:

1. `cargo check`
2. `cargo test -- --nocapture`
3. `check-config … --json --strict`
4. `doctor --config … --json`
5. `route-explain … --json` (smoke; non-zero exit on no match)

### Drop-in excerpt for another repository

Keep **paths** aligned with where you copied `mock-config.yaml` (or your profile).

```yaml
- name: Install Rust
  uses: dtolnay/rust-toolchain@stable

- name: Build and test
  run: |
    cargo check
    cargo test -- --nocapture

- name: check-config strict (mock profile example)
  run: cargo run -- check-config config/mock-config.yaml --json --strict

- name: doctor (JSON)
  run: cargo run -- doctor --config config/mock-config.yaml --json

- name: Smoke route-explain
  run: cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json
```

## GitLab CI

This repository ships **`.gitlab-ci.yml`** with the same stages in a single `rust` image job (`rust:1-bookworm`). Copy that file into a consumer repo or paste the `script:` block:

```yaml
script:
  - cargo check
  - cargo test -- --nocapture
  - cargo run -- check-config config/mock-config.yaml --json --strict
  - cargo run -- doctor --config config/mock-config.yaml --json
  - cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json
```

Add **`cache:` for `target/`** on long-running pipelines if runners allow it.

## Upstream TCP probe (dockerized)

Workflow `.github/workflows/doctor-probe.yml` runs **only on manual trigger** (`workflow_dispatch`).

It now boots local mock upstreams via Docker Compose (`.github/compose/doctor-probe.compose.yml`) before probing:

```bash
docker compose -f .github/compose/doctor-probe.compose.yml up -d
cargo run -- doctor --config config/mock-config.yaml --probe-upstream --json
docker compose -f .github/compose/doctor-probe.compose.yml down -v
```

The compose stack binds `127.0.0.1:9000` and `127.0.0.1:9001`. Mock profile defaults both mock `service_id` targets to `127.0.0.1:9001`, so **`doctor --probe-upstream`** is satisfied once port `9001` is reachable (the workflow still waits on both compose ports).

`workflow_dispatch` supports optional inputs:

- `config_path` (default `config/mock-config.yaml`)
- `compose_file` (default `.github/compose/doctor-probe.compose.yml`)

When probe fails, workflow uploads `doctor-probe-docker-diagnostics` artifact containing:

- `compose-ps.txt`
- `compose-logs.txt`

Local equivalent script: `docs/doctor-probe-compose.sh`

```bash
bash docs/doctor-probe-compose.sh
```

Optional env vars for local run:

- `SERVICE_ROUTER_CONFIG` (default `config/mock-config.yaml`)
- `SERVICE_ROUTER_PROBE_COMPOSE_FILE` (default `.github/compose/doctor-probe.compose.yml`)

## Why this baseline

- Catches compile/test regressions early.
- Validates strict route/config checks in a deterministic mock environment.
- Ensures developer diagnostics commands stay healthy in CI.
- Smoke `route-explain` proves routing compiles **and** a known-good request still hits an expected rule.

## Optional extension

For real registry integration jobs, add a separate workflow/job that runs against `config/config.yaml` and injects required secrets (for example `NACOS_PASSWORD`) from CI secret storage.
