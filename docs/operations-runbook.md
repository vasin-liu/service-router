# Operations runbook (environment-agnostic)

Minimal playbook for running **service-router** behind a load balancer or in Kubernetes without assuming naming conventions in your cluster. Adapt hostnames, paths, and secrets to your deployment.

## 1. Health endpoints

| Endpoint | Role | Success |
|:---------|:-----|:--------|
| `GET /health` | Liveness | Always **200** when the process is up (`{"status":"ok"}`). |
| `GET /ready` | Readiness | **200** when at least one registry is healthy or degraded, or when **no** registry is configured. **503** when **every** configured registry reports unhealthy (`{"status":"not_ready",...}`). |

Wire the orchestrator so **liveness** uses `/health` and **readiness** uses `/ready`. Draining traffic when all registries are down avoids routing through a proxy that cannot resolve services.

## 2. Metrics

- `GET /metrics` — JSON snapshot (`route_hits`, `failure_reasons`). See [`metrics-json.md`](./metrics-json.md).
- `GET /metrics/prometheus` — Prometheus text exposition (`service_router_route_hits_total`, `service_router_failures_total`).

Stable failure reason strings are listed in [`diagnostic-codes.md`](./diagnostic-codes.md). For alert ideas tied to those counters, see [§8](#8-alerting-hooks-prometheus).

If counters climb during an incident:

| `failure_reasons` key | First checks |
|:-----------------------|:--------------|
| `no_matching_route` | Route path/method/headers vs traffic (`cargo run -- route-explain …`). |
| `no_instances` / `registry_all_failed` | `GET /ready`, `doctor --json`; `service_id` and Endpoint/Slice conditions in-cluster. |
| `registry_auth_failed` | Credentials or tokens for Nacos, Eureka, Kubernetes. |
| `upstream_connection` | `doctor --probe-upstream`; firewall and upstream host:port. |
| `registry_http` / `registry_unexpected` | Registry endpoint URL, TLS, and compatibility. |

Rising `registry_all_failed`, `registry_auth_failed`, or `no_instances` usually precede user-visible errors.

## 3. Config change, rollback, and binary upgrade

The server watches the config file’s directory and reloads on write (see `src/config/watcher.rs`). If reload fails, the **previous** config and resolver stay active.

**Rollback config without redeploying the binary:**

1. Restore the known-good YAML on disk (same path as process startup).
2. Ensure the write triggers the watcher (save / atomic replace).
3. Confirm `/ready` returns **200** (or acceptable degraded state) and spot-check a representative route with `route-explain`.

For a bad change already loaded: keep the last revision in version control or artifact storage next to the binary.

**Binary upgrade (rolling deploy):** ship a new `service-router` artifact; keep the **previous binary** and **last-known-good config** tarball adjacent for fast rollback. After processes restart, run the [§7](#7-post-deployment-checklist) checklist. Config hot-reload does **not** replace a binary upgrade — both layers change independently.

## 4. Structured checks before escalation

Run from a bastion or CI using the **same** config file the process uses:

```bash
cargo run -- check-config --config /path/to/config.yaml --json --strict
cargo run -- doctor --config /path/to/config.yaml --json
cargo run -- doctor --config /path/to/config.yaml --probe-upstream --json
```

Interpret JSON using [`doctor-json-schema.md`](./doctor-json-schema.md). Use `--probe-upstream` when the symptom looks like network or upstream reachability.

**Routing snapshot:** `check-config` always compiles `routes:` (including optional **`response_headers`** on each rule). Invalid values fail before `--strict` findings; behavior matches process startup. Reference: [`plugin-extension.md`](./plugin-extension.md).

## 5. Release gate

Use [`release-acceptance-matrix.md`](./release-acceptance-matrix.md) and `docs/release-acceptance.sh` (or `.ps1`) before promoting a build.

## 6. Kubernetes (no namespace conventions required)

Until team standards exist:

- Point `service-router` at one namespace via the Kubernetes registry block (`namespace:` in YAML); validate discovery against **that** namespace only.
- Confirm `doctor --json` reports Kubernetes registry **healthy** and that `route-explain` for a known `service_id` shows a match when the backing Service has endpoints.
- **Discovery tracing**: enable debug logs for the Kubernetes registry resolver, e.g. RUST_LOG=service_router::registry::k8s=debug — logs include service_id, namespace, instance count, and whether backends came from **Core Endpoints** or **EndpointSlice** fallback. Use RUST_LOG=service_router::registry::k8s=trace for per-request GET URLs (Service, Core Endpoints, EndpointSlice list).
- If readiness flaps, inspect `registry_health` from **`GET /ready`** side-by-side with `kubectl get endpoints` / `endpointslices` for the same Service name — mismatches are usually RBAC, wrong cluster context, or wrong Service name in routes.

Further product context: [`product-design.md`](./product-design.md), [`implementation-status.md`](./implementation-status.md).

## 7. Post-deployment checklist

Run within minutes of a **rollout**, **binary upgrade**, or **config hot-reload**:

1. **`GET /health`** → 200.
2. **`GET /ready`** → 200 unless every registry is intentionally offline (then 503 is expected until at least one registry recovers). Quick script (same checks): [`../scripts/post-deploy-smoke.sh`](../scripts/post-deploy-smoke.sh) or **`scripts/post-deploy-smoke.ps1`** with optional **`SERVICE_ROUTER_BASE_URL`**.
3. **CLI gates** (same `--config` as the running process): `check-config … --strict` exit 0; `doctor --json` has `status: "pass"`.
4. **Traffic spot-check**: one `route-explain` for a critical rule id, or a single synthetic request through the load balancer.
5. **Metrics baseline**: glance at `route_hits` and `failure_reasons` in `GET /metrics`; compare rates to pre-change only when investigating regressions (counters reset on process restart).

## 8. Alerting hooks (Prometheus)

`GET /metrics/prometheus` exposes `service_router_failures_total{reason="…"}` with the same **`reason`** strings as JSON `failure_reasons`. Useful starting points:

| Symptom intent | What to watch | First response |
|:---------------|:--------------|:---------------|
| Routing misconfiguration | Rise in `no_matching_route` | Verify deployed route table; `route-explain` with real paths/methods. |
| Empty discovery | Rise in `no_instances` or `registry_all_failed` | Registry / cluster registration; `doctor --probe-upstream --json`. |
| Auth / tokens | `registry_auth_failed` | Refresh Nacos/Eureka/kube credentials in config or secrets store. |
| Cannot reach instance hosts | `upstream_connection` | Network policy, upstream pods, target port; probe paths in [`diagnostic-codes.md`](./diagnostic-codes.md). |

Example expression (tune thresholds for your environment): `sum(rate(service_router_failures_total{reason="no_instances"}[5m])) > 0`.

---

For a single-page map between metrics, `doctor`, and `route-explain` codes, use [`diagnostic-codes.md`](./diagnostic-codes.md).
