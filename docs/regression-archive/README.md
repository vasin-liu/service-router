# Regression evidence (M2 §9)

This folder holds **templates and workflow notes** for archiving release acceptance results. It does **not** store real JSON by default (avoid committing secrets or environment-specific paths).

## Suggested flow

1. Run **`docs/release-acceptance.sh`** (or **`docs/release-acceptance.ps1`**) with the same `SERVICE_ROUTER_CONFIG` as the environment under test.
2. Collect the output directory (default **`artifacts/release-acceptance/`**), which should contain these five §7 JSON files after a successful run (plus **`section-9-summary.generated.md`**):
   - `check-config.json`
   - `doctor.json`
   - `doctor-probe.json`
   - `route-explain-smoke.json`
   - `config-snapshot.json` (redacted)
3. Copy **`section-9-summary-template.md`** to your ticket/wiki or internal storage; fill one row per **profile** (Mock / Nacos / Eureka / Kubernetes); use the template’s checklist for the JSON files. Optionally generate a paste-ready Markdown table from the five §7 JSON files: **`python scripts/summarize-section9-release-acceptance.py`** (see `--help`; supports the same env vars as `release-acceptance` for profile, artifact path, and sign-off fields). The runner scripts now emit this automatically as **`section-9-summary.generated.md`** in the artifact directory.
4. Attach or link the artifact bundle: on GitHub Actions, the workflow uploads **`release-acceptance-bundle`** (five §7 JSON + **`section-9-summary.generated.md`**); GitLab **`release-acceptance-manual`** publishes `artifacts/release-acceptance/`; or use object storage / ticket attachment per org policy (do not commit secrets into this repo).

## References

- Full matrix: [`../release-acceptance-matrix.md`](../release-acceptance-matrix.md) (especially **§7** artifacts and **§9** summary).
- CI naming and copy-paste jobs: [`../ci-template.md`](../ci-template.md) (`release-acceptance`, GitLab manual job).
- M2 readiness mapping: [`../m2-release-readiness.md`](../m2-release-readiness.md) — optional release-acceptance bundle after baseline: [`#m2-release-acceptance-bundle`](../m2-release-readiness.md#m2-release-acceptance-bundle).
- Post-rollout HTTP smoke (running process): [`../../scripts/post-deploy-smoke.sh`](../../scripts/post-deploy-smoke.sh) or **`.ps1`**.
