# §9 regression summary (copy below the line into ticket / wiki)

Tip: **`docs/release-acceptance.sh`** / **`.ps1`** now writes **`section-9-summary.generated.md`** in the same directory after the five JSON files. To regenerate or customize flags, run **`python scripts/summarize-section9-release-acceptance.py --help`** (CLI gates and route smoke are inferred from the artifacts; global gates must be passed in or set via `SERVICE_ROUTER_ACCEPTANCE_GLOBAL_GATES`).

---

| Field | Value |
|:------|:------|
| **Date / TZ** | |
| **Git** | `git rev-parse HEAD` |
| **Router binary** | e.g. `service-router 0.1.0` |
| **Profile** | Mock / Nacos / Eureka / Kubernetes |
| **Config** | path or name only (no secrets) |
| **Global gates** | §1 `cargo check` / `cargo test` — pass / fail |
| **CLI gates** | §3 `check-config --strict`, `doctor`, `doctor --probe-upstream` — pass / fail |
| **Route smoke** | §4 `route-explain` — matched yes/no (path/method: ___ ) |
| **Config snapshot** | `config-snapshot.json` present — yes/no (redacted export) |
| **Artifacts dir** | e.g. `artifacts/release-acceptance/` or CI artifact URL |
| **Deviations** | e.g. `ALLOW_PROBE_FAIL=1`, flaky registry, issue link |
| **Sign-off** | |

## Expected artifacts (JSON + Markdown) (`release-acceptance.sh` / `.ps1` or CI `release-acceptance-json`)

Check off each file you attach or link (same names under `artifacts/release-acceptance/`). The five JSON files are required for machine-readable evidence; **`section-9-summary.generated.md`** is the human paste companion from the same run.

- [ ] `check-config.json`
- [ ] `doctor.json`
- [ ] `doctor-probe.json`
- [ ] `route-explain-smoke.json`
- [ ] `config-snapshot.json` (redacted)
- [ ] `section-9-summary.generated.md` (optional but recommended; from the same `release-acceptance` run)
