# CI Template (Developer Baseline)

## GitHub Actions baseline

This repository includes `.github/workflows/ci.yml` with the baseline pipeline:

1. `cargo check`
2. `cargo test -- --nocapture`
3. `check-config` against `config/mock-config.yaml` with `--json --strict`
4. `doctor --json` against `config/mock-config.yaml`

## Optional: upstream TCP probe

Workflow `.github/workflows/doctor-probe.yml` runs **only on manual trigger** (`workflow_dispatch`).

It executes:

```bash
cargo run -- doctor --config config/mock-config.yaml --probe-upstream --json
```

On default GitHub-hosted runners nothing listens on the mock profile ports (`127.0.0.1:9000`, etc.), so this job often **fails** until you run it where upstreams exist (self-hosted runner, VPN to dev cluster, or after starting local mocks). Use it to validate connectivity in an environment that mirrors runtime.

## Why this baseline

- Catches compile/test regressions early.
- Validates strict route/config checks in a deterministic mock environment.
- Ensures developer diagnostics commands stay healthy in CI.

## Optional extension

For real registry integration jobs, add a separate workflow that runs against `config/config.yaml` and injects required secrets (for example `NACOS_PASSWORD`) from CI secret storage.
