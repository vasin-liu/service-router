# Next engineering priorities (after M3 slice)

Ordered roughly by **`docs/implementation-status.md`** “下一阶段建议” and roadmap docs. **No code change is implied** until each item is scheduled.

| Priority | Track | Action |
|:---------|:------|:-------|
| P0 | Environment | Run **`release-acceptance-matrix.md`** §3–§4 per profile (Nacos / Eureka / Kubernetes + Mock); archive §9 summary + artifacts (**`docs/regression-archive/`**). |
| P0 | Rollout | After deploy or hot-reload: **`scripts/post-deploy-smoke.sh`** / **`.ps1`** + runbook §7–§8 checks. |
| P1 | FR-5.3 process | Ticket paste: **`docs/config-snapshot-workflow.md`**；release 五 JSON → §9 表：**`scripts/summarize-section9-release-acceptance.py`**（**`docs/regression-archive/`**）。 |
| P1 | FR-6 | Dynamic plugins: follow **ADR 001** (`docs/adr/001-fr6-dynamic-plugins-deferred.md`) — design review before implementation. |
| P2 | Kubernetes | Scale/observability/multi-cluster: **`implementation-status.md`** §下一阶段-2; **`developer-roadmap-1-2y.md`**. |
| P2 | Resilience | LB weights, circuit breakers, richer WebSocket policy: **`product-design-one-pager.md`**, roadmap §4.2. |
| 远期 | Registry | **Consul**: **`developer-roadmap-1-2y.md` §4.1**. |
