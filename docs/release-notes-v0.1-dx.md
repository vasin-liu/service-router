# Service Router v0.1 Release Notes (Developer Experience)

## Highlights

- New developer-first CLI flow for config validation, diagnostics, and health checks.
- Mock registry support for local development and CI without external Nacos/Eureka.
- Better strict validation to catch common routing mistakes earlier.

## What is new

### 1) CLI diagnostics and validation

- `check-config` now supports:
  - `--json` for machine-readable output
  - `--strict` for stronger static checks
- `route-explain` now supports:
  - JSON output
  - verbose diagnostics with `--verbose`
  - explicit mismatch reasons for path/method/header
- `doctor` now supports:
  - `--config <path>` option
  - per-registry health status output

### 2) Strict check improvements

- Duplicate route IDs are reported.
- Identical matcher conflicts are reported.
- Catch-all shadowing risk is reported when `prefix "/"` can override later rules.

### 3) Mock registry support

- New registry source: `type: mock`
- Service instances can be declared directly in config.
- Example profile: `config/mock-config.yaml`
- Enables local/CI workflows without external registry infrastructure.

## Validation status

- `cargo check` passed.
- Full test suite passed (`23` tests).
- Mock-mode smoke commands passed:
  - `check-config --json --strict`
  - `route-explain --json`
  - `doctor --config ...`

## Recommended adoption steps

1. Start local development with `config/mock-config.yaml`.
2. Add `check-config --json --strict` to CI for early route/config validation.
3. Use `route-explain --verbose` during routing issue triage.
4. Keep production registry config in `config/config.yaml` with required env vars.
