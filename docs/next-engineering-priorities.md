# Next engineering priorities (post v1.0.0)

v1.0.0 released 2026-05-12. Ordered by impact. **No code change is implied** until each item is scheduled.

| Priority | Track | Status | Action |
|:---------|:------|:-------|:-------|
| ~~P0~~ ✔ | Release | done | v1.0.0 tag + release workflow + Mock profile acceptance archived. |
| ~~P0~~ ✔ | Docs | done | Getting Started guide, plugin dev guide, config JSON Schema, Dockerfile. |
| P0 | Environment | pending | Nacos / Eureka / Kubernetes acceptance on real clusters; archive to `docs/regression-archive/`. |
| P1 | Distribution | pending | Docker image publish to container registry. |
| P1 | FR-6.3 | pending | `dlopen` external plugin loading (ADR 002 design ready, implement on demand). |
| P2 | Observability | pending | OpenTelemetry integration (replace self-built /metrics). |
| P2 | Testing | pending | E2E integration tests (real HTTP proxy round-trips). |
| P2 | Kubernetes | pending | Scale/observability/multi-cluster enhancements. |
| P3 | DX | pending | `run --dev` mode, IDE support enhancements. |
| 远期 | Registry | pending | **Consul**: **`developer-roadmap-1-2y.md` §4.1**. |
