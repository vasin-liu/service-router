# M2 release readiness (engineering vs organisation)

This document maps **`docs/implementation-status.md` §「完成定义（M2）」** to **what the repository already provides** and **what still requires a target environment**.

## Engineering closure (M2)

Treat **M2 as closed for engineering purposes** when:

- Criteria **2** and **3** below are satisfied by documentation + tooling review (JSON schemas, runbook, matrix, scripts).
- **Mock** profile evidence exists via `.github/workflows/ci.yml` (and optionally local **`scripts/verify-m2-baseline.*`**).
- Any remaining gap is **only** criterion **1** for **Nacos / Eureka / Kubernetes** live environments — tracked with **`release-acceptance-matrix.md`** §**9**, owned by the operating team (credentials and clusters are outside this repo).

## M2 completion criteria (verbatim)

1. 在真实环境下完成 Nacos/Eureka/K8s/Mock 四类配置的回归检查并沉淀报告  
2. 关键诊断命令具备稳定 JSON 契约与 failure code 文档  
3. 发布前后具备统一巡检步骤与可执行回滚预案  

## Criterion 1 — Four-profile regression

| Profile | Automated in repo | Human / environment |
|:--------|:------------------|:---------------------|
| **Mock** | Yes: `.github/workflows/ci.yml` runs build, tests, `check-config --strict`, `doctor`, `route-explain`, compose + `doctor --probe-upstream`. Same commands locally: **`bash scripts/verify-m2-baseline.sh`**; optional **`M2_WITH_DOCKER_PROBE=1 bash scripts/verify-m2-baseline.sh`**. | Optional: archive §9 summary from a manual run. |
| **Nacos** | No live registry in CI. | Run **`docs/release-acceptance-matrix.md`** §3–§4 with your `server_addr` / secrets; store §7 acceptance artifacts (JSON + **`section-9-summary.generated.md`** when using `release-acceptance`); fill §**9** summary. |
| **Eureka** | No live registry in CI. | Same as Nacos with Eureka credentials and `health_path` as needed. |
| **Kubernetes** | No cluster in default CI. | Same matrix gates against kubeconfig/API access; see matrix §5; optional **`RUST_LOG=service_router::registry::k8s=debug`** (or **`trace`**) for resolver tracing (`operations-runbook.md` §6). |

**Closing M2 on criterion 1** requires organisation-owned evidence for **all four** profiles (Mock evidence can be CI green + optional §9 row).

## Criterion 2 — Stable JSON contracts & failure codes

| Area | Documentation |
|:-----|:--------------|
| Metrics / proxy failures | [`diagnostic-codes.md`](./diagnostic-codes.md), [`metrics-json.md`](./metrics-json.md) |
| `doctor --json` | [`doctor-json-schema.md`](./doctor-json-schema.md) |
| `route-explain --json` | [`route-explain-json-schema.md`](./route-explain-json-schema.md) |
| `check-config --strict` | [`check-config-strict-schema.md`](./check-config-strict-schema.md) |
| `/ready` | [`diagnostic-codes.md`](./diagnostic-codes.md), [`doctor-json-schema.md`](./doctor-json-schema.md) |

## Criterion 3 — Rollout inspection & rollback

| Topic | Documentation |
|:------|:--------------|
| Post-deploy checklist, Prometheus hooks | [`operations-runbook.md`](./operations-runbook.md) §7–§8 |
| Config hot-reload / binary upgrade | [`operations-runbook.md`](./operations-runbook.md) §3 |
| Full CLI artifact gate | [`release-acceptance-matrix.md`](./release-acceptance-matrix.md); runner **`docs/release-acceptance.sh`** (or **`.ps1`**); archive template §**9** + [`regression-archive/README.md`](./regression-archive/README.md) |
| Running process smoke | [`scripts/post-deploy-smoke.sh`](../scripts/post-deploy-smoke.sh) / **`.ps1`** (`SERVICE_ROUTER_BASE_URL`) |

## One-shot local verification (Mock)

From repo root:

**Linux / macOS / Git Bash (CI‑parity bash script):**

```bash
bash scripts/verify-m2-baseline.sh
```

With Docker (matches CI probe step):

```bash
M2_WITH_DOCKER_PROBE=1 bash scripts/verify-m2-baseline.sh
```

**Windows PowerShell:**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-m2-baseline.ps1
```

Optional Docker probe:

```powershell
$env:M2_WITH_DOCKER_PROBE = '1'
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-m2-baseline.ps1
```

Change config via **`SERVICE_ROUTER_CONFIG`** if needed (both scripts).

<a id="m2-json-bundle-s9"></a>

### Optional: JSON bundle for §9 archive (Mock)

`verify-m2-baseline` mirrors PR CI gates but **does not** write JSON under `artifacts/release-acceptance/`. To collect the same **five** JSON files as **`release-acceptance-matrix.md`** §7 — `check-config.json`, `doctor.json`, `doctor-probe.json`, `route-explain-smoke.json`, `config-snapshot.json` — plus **`section-9-summary.generated.md`**, for a ticket or **`regression-archive`** §9 row, run **`docs/release-acceptance.sh`** (or **`.ps1`**) after the baseline.

Skip redundant `cargo check` / `cargo test` if you already ran the baseline:

```bash
SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL=0 bash docs/release-acceptance.sh
```

PowerShell:

```powershell
$env:SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL = '0'
powershell -NoProfile -ExecutionPolicy Bypass -File docs/release-acceptance.ps1
```

Then attach `artifacts/release-acceptance/` (including **`section-9-summary.generated.md`** when the runner completed that step), or the GitHub **`release-acceptance-json`** artifact from a manual **`release-acceptance`** workflow run; use **`regression-archive/section-9-summary-template.md`** only if you need a blank stub. Index: **`regression-archive/README.md`**.

## Sign-off

Treat **M2 as engineering-complete** when:

- CI on `dev`/`main` is green (includes Mock + compose probe as configured in `ci.yml`), **and**
- This checklist’s criterion **2** and **3** are satisfied by documentation review, **and**
- Your team has filed criterion **1** evidence per **`release-acceptance-matrix.md`** §**9** for each required profile.

See also **`implementation-status.md`** — section **「M2 仓库侧就绪」**.
