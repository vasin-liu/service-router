# CI Template (Developer Baseline)

## What to run (`check-config` + `doctor` + smoke)

Use the **same three gates** everywhere: strict config validation, environment/diagnostic sanity, then a deterministic **route match** probe.

| Step | Command | Pass when |
|:-----|:--------|:----------|
| Build & unit tests | `cargo check`, `cargo test -- --nocapture` | Exit `0` |
| Strict config | `cargo run -- check-config config/mock-config.yaml --json --strict` | Exit `0`, JSON `strict_passed == true` if you parse it |
| Doctor | `cargo run -- doctor --config config/mock-config.yaml --json` | Exit `0`, JSON top-level `status` is typically `"pass"` for healthy mock runs |
| Doctor + network | `cargo run -- doctor --config config/mock-config.yaml --probe-upstream --json` | Exit `0` if no TCP/registry/upstream failures; JSON includes `registry_endpoint_probe` (remote registries) and `upstream_probe` |
| Smoke (route explain) | `cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json` | Exit `0`, JSON `matched == true` against mock profile; sample `orders-api` includes `response_headers` so JSON may show a non-null `response_headers` object |
| Redacted snapshot | `cargo run -- config-snapshot --config config/mock-config.yaml -o -` | Exit `0`; stdout is pretty-printed JSON (`diagnostic_version` **1.0**); same command used in `.github/workflows/ci.yml` |

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

`.github/workflows/ci.yml` mirrors the table above and adds a **compose-backed upstream probe**:

1. `cargo check`
2. `cargo test -- --nocapture`
3. `check-config … --json --strict`
4. `doctor --config … --json`
5. `route-explain … --json` (smoke; non-zero exit on no match)
6. `config-snapshot … -o -` (redacted JSON to stdout)
7. `docker compose -f .github/compose/doctor-probe.compose.yml up -d`
8. Wait until TCP `127.0.0.1:9000` and `:9001` accept connections (bash `/dev/tcp` loop)
9. `doctor --config … --probe-upstream --json` (validates `upstream_probe` / `failure_code` paths against mock TCP echoes)
10. `docker compose … down -v` (`always()`)

Requires Docker on the runner (default `ubuntu-latest` provides it). The manual **`doctor-upstream-probe`** workflow uses the same compose file for ad-hoc runs.

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

- name: Config snapshot (redacted)
  run: cargo run -- config-snapshot --config config/mock-config.yaml -o -
```

## GitLab CI

The included **`.gitlab-ci.yml`** mirrors **GitHub `ci.yml`**: same Rust gates, then **`docker:24-dind`**, `docker compose -f .github/compose/doctor-probe.compose.yml`, wait on ports **9000/9001**, and **`doctor --probe-upstream --json`**. The job installs Docker CLI + Compose plugin via `apt` and sets `DOCKER_HOST=tcp://docker:2375` with empty `DOCKER_TLS_CERTDIR` (matches GitLab’s common non-TLS DinD recipe). **Requires a runner that allows the DinD service** (privileged).

Copy that file into a consumer repo or paste the `script:` block:

```yaml
script:
  - cargo check
  - cargo test -- --nocapture
  - cargo run -- check-config config/mock-config.yaml --json --strict
  - cargo run -- doctor --config config/mock-config.yaml --json
  - cargo run -- route-explain /api/orders/123 GET --config config/mock-config.yaml --json
  - cargo run -- config-snapshot --config config/mock-config.yaml -o -
  # plus compose + doctor --probe-upstream — see `.gitlab-ci.yml` in this repo
```

Optional manual job **`release-acceptance-manual`** (stage `release`) runs **`docs/release-acceptance.sh`** and uploads JSON from `artifacts/release-acceptance/`. It sets `SERVICE_ROUTER_ACCEPTANCE_ALLOW_PROBE_FAIL=1` so `doctor --probe-upstream` does not fail the job when no mock TCP upstream is listening; for compose-backed probes (like GitHub’s `release-acceptance` workflow), use a runner with [Docker-in-Docker](https://docs.gitlab.com/ci/docker/using_docker_build/) and mirror `.github/workflows/release-acceptance.yml`.

Add **`cache:` for `target/`** on long-running pipelines if runners allow it.

## GitHub manual workflows (compose-backed)

### `release-acceptance` (full JSON artifact set)

Workflow `.github/workflows/release-acceptance.yml` runs **only on manual trigger** (`workflow_dispatch`): it starts the compose-backed mock upstreams unless **`skip_compose`** is `yes`, waits on `9000`/`9001`, runs `bash docs/release-acceptance.sh`, and uploads **`release-acceptance-json`** (files under `artifacts/release-acceptance/`). With **`skip_compose: yes`**, compose is skipped and `SERVICE_ROUTER_ACCEPTANCE_ALLOW_PROBE_FAIL=1` is set so `doctor --probe-upstream` does not fail the job when no TCP upstream is listening. JSON artifacts upload on **`always()`** (partial outputs when a gate fails).

### `doctor-upstream-probe` (doctor only)

Workflow `.github/workflows/doctor-probe.yml` runs **only on manual trigger** (`workflow_dispatch`). It boots the same Compose file (default `.github/compose/doctor-probe.compose.yml`) before running `doctor --probe-upstream --json`.

```bash
docker compose -f .github/compose/doctor-probe.compose.yml up -d
cargo run -- doctor --config config/mock-config.yaml --probe-upstream --json
docker compose -f .github/compose/doctor-probe.compose.yml down -v
```

The compose stack binds `127.0.0.1:9000` and `127.0.0.1:9001`. Mock profile defaults both mock `service_id` targets to `127.0.0.1:9001`, so **`doctor --probe-upstream`** is satisfied once port `9001` is reachable (the workflow still waits on both compose ports).

Both workflows expose `workflow_dispatch` inputs such as `config_path` and `compose_file` (defaults as in each YAML). **`release-acceptance`** also exposes **`skip_compose`** (`no` / `yes`).

When `doctor-probe` fails, the workflow uploads `doctor-probe-docker-diagnostics` containing:

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
- `config-snapshot` proves the redacted export path stays buildable on every PR.

## Optional extension

For real registry integration jobs, add a separate workflow/job that runs against `config/config.yaml` and injects required secrets (for example `NACOS_PASSWORD`) from CI secret storage.
