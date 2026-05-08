# CI Template (Developer Baseline)

## GitHub Actions baseline

This repository includes `.github/workflows/ci.yml` with the baseline pipeline:

1. `cargo check`
2. `cargo test -- --nocapture`
3. `check-config` against `config/mock-config.yaml` with `--json --strict`
4. `doctor` against `config/mock-config.yaml`

## Why this baseline

- Catches compile/test regressions early.
- Validates strict route/config checks in a deterministic mock environment.
- Ensures developer diagnostics commands stay healthy in CI.

## Optional extension

For real registry integration jobs, add a separate workflow that runs against `config/config.yaml` and injects required secrets (for example `NACOS_PASSWORD`) from CI secret storage.
