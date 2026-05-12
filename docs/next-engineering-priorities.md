# Next engineering priorities (post v1.1.0)

v1.1.0 in development. Ordered by impact.

| Priority | Track | Status | Action |
|:---------|:------|:-------|:-------|
| ~~P0~~ ✔ | Release | done | v1.0.0 tag + release workflow + Mock profile acceptance archived. |
| ~~P0~~ ✔ | Docs | done | Getting Started guide, plugin dev guide, config JSON Schema, Dockerfile. |
| ~~P2~~ ✔ | Observability | done | OpenTelemetry tracing integration (OTLP exporter, env-driven). |
| ~~P2~~ ✔ | Testing | done | E2E integration tests (`tests/e2e_proxy.rs`, 3 tests). |
| ~~P3~~ ✔ | DX | done | `run --dev` mode (verbose log + local-override auto-discover). |
| ~~P2~~ ✔ | Security | done | HTTPS/TLS termination (`server.tls` config, rustls). |
| ~~P2~~ ✔ | Resilience | done | Graceful shutdown enhancement (drain logging). |
| P0 | Environment | pending | Nacos / Eureka / Kubernetes acceptance on real clusters; archive to `docs/regression-archive/`. |
| P1 | Distribution | pending | Docker image publish to container registry. |
| P1 | FR-6.3 | pending | `dlopen` external plugin loading (ADR 002 design ready, implement on demand). |
| P2 | Kubernetes | pending | Scale/observability/multi-cluster enhancements. |
| 远期 | Registry | pending | **Consul**: **`developer-roadmap-1-2y.md` §4.1**. |
