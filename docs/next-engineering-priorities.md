# Next engineering priorities (after M3 slice)

Ordered roughly by **`docs/implementation-status.md`** “下一阶段建议” and roadmap docs. **No code change is implied** until each item is scheduled.

| Priority | Track | Action |
|:---------|:------|:-------|
| P0 | Environment | Run **`release-acceptance-matrix.md`** §3–§4 per profile (Nacos / Eureka / Kubernetes + Mock); archive §9 summary + artifacts (**`docs/regression-archive/`**). |
| P0 | Rollout | After deploy or hot-reload: **`scripts/post-deploy-smoke.sh`** / **`.ps1`** + runbook §7–§8 checks. |
| P1 | FR-5.3 process | Ticket paste: **`docs/config-snapshot-workflow.md`**；release 五份 §7 JSON + **`section-9-summary.generated.md`** / §9 表：**`scripts/summarize-section9-release-acceptance.py`**（**`docs/regression-archive/`**）。 |
| P1 | FR-6 | ~~Dynamic plugins design review~~ ✔ ADR 002; ~~Plugin SDK (Phase C)~~ ✔ trait + chain + config + 3 built-in plugins; **next**: FR-6.3 `dlopen` external plugin loading. |
| P1 | Resilience | ~~LB weights~~ ✔; ~~WS bidirectional relay~~ ✔; ~~circuit breakers~~ ✔; ~~retry policy~~ ✔ — **全部完成**; **next**: active health checks (periodic upstream liveness probes). |
| P2 | Kubernetes | Scale/observability/multi-cluster: **`implementation-status.md`** §下一阶段-2; **`developer-roadmap-1-2y.md`**. |
| P2 | Phase C done | ~~FR-6 plugin SDK design review~~ ✔; ~~multi-env profile + config drift detection~~ ✔ `config-drift` CLI. |
| 远期 | Registry | **Consul**: **`developer-roadmap-1-2y.md` §4.1**. |
