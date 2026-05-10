# Stable diagnostic codes

Single reference for **machine-stable** strings emitted by the proxy (`GET /metrics`), `doctor --json`, and `route-explain --json`.  
Human-only text (for example `check-config --strict` finding lines) is intentionally not listed here.

## Cross-reference (triage)

Use this when correlating production metrics with CLI checks:

| `failure_reasons` key (`GET /metrics`) | Typical cause | Next checks |
|:---------------------------------------|:--------------|:------------|
| `no_matching_route` | Path/method/header did not match any rule | `route-explain <path> <method> --config … --json` (`PATH_MISMATCH`, `METHOD_MISMATCH`, …) |
| `no_instances` | Registry returned no backends for `service_id` | `doctor --probe-upstream --json` → `upstream_probe` / registry health; verify registration |
| `registry_http`, `registry_unexpected`, `registry_not_found`, `registry_auth_failed`, `registry_all_failed` | Registry API/TLS/auth/query failure | `doctor --probe-upstream` (`registry_endpoint_probe`, `upstream_probe` with same code family); compare [`doctor-json-schema.md`](./doctor-json-schema.md) |
| `upstream_connection` | TCP/connect failure to a resolved upstream | Same routes with `--probe-upstream`; expect `TCP_UNREACHABLE`-style probe failures where applicable |
| `ws_upgrade`, `body_read` | Proxy forwarding edge cases | Logs; reproduce with same route/upstream |

Doctor **`TCP_UNREACHABLE`** and **`ENDPOINT_PARSE_ERROR`** describe TCP reachability and host/port parse failures during probes (registry endpoint or upstream); they do not appear in `failure_reasons` today but align with operational checks for `upstream_connection` / bad URLs.

## Proxy failure reasons (`failure_reasons` in `GET /metrics`)

These match `failure_code_for_proxy` / `failure_code_for_registry` in `src/server/metrics.rs`.

| Code | Meaning |
|:-----|:--------|
| `no_matching_route` | No route matched the request path/method/headers. |
| `no_instances` | Registry returned zero instances for `service_id`. |
| `registry_http` | HTTP-level error talking to a registry. |
| `registry_unexpected` | Unexpected registry response or parse failure. |
| `registry_not_found` | Service not found in registry (when applicable). |
| `registry_auth_failed` | Registry rejected credentials (401/403-class). |
| `registry_all_failed` | All registries failed or returned empty (depends on query mode). |
| `upstream_connection` | Could not connect to resolved upstream. |
| `ws_upgrade` | WebSocket upgrade handling failed. |
| `body_read` | Failed reading request body for proxying. |

Prometheus label `reason` on `service_router_failures_total` uses the same strings.

## Doctor (`doctor --json`, `--probe-upstream`)

Documented in detail in [`doctor-json-schema.md`](./doctor-json-schema.md).

### Registry endpoint probe (`registry_endpoint_probe`)

| Code | Meaning |
|:-----|:--------|
| `TCP_UNREACHABLE` | Socket connect to configured registry API host:port failed within timeout. |
| `ENDPOINT_PARSE_ERROR` | Could not derive host/port from configured URL. |

### Upstream probe (`upstream_probe`)

Each row may include **`failure_code`** when `reachable` is false or when the target could not be probed:

| Code | When |
|:-----|:-----|
| `TCP_UNREACHABLE` | Direct `upstream_url` host:port not reachable, or `service_id` resolved to instances but none answered TCP within the probe. |
| `ENDPOINT_PARSE_ERROR` | `upstream_url` could not be parsed to host/port. |
| `no_instances` | Resolution returned zero instances (`failure_reasons` uses the same string). |
| `registry_http`, `registry_unexpected`, `registry_not_found`, `registry_auth_failed`, `registry_all_failed` | Registry error while resolving `service_id` — same strings as [`failure_reasons`](#proxy-failure-reasons-get-metrics). |

Top-level `status` is `"pass"` or `"fail"` (any unhealthy registry row sets failure; probe failures add to failure).

## Route explain (`route-explain --json`)

Suggestion entries under `suggestions[]` / `remediation_outline[]` use:

| Code | Typical cause |
|:-----|:----------------|
| `PATH_MISMATCH` | Request path does not satisfy rule matcher. |
| `METHOD_MISMATCH` | HTTP method does not match rule. |
| `HEADER_VALUE_MISMATCH` | Header present but value mismatch. |
| `HEADER_MISSING` | Required header absent. |
| `RULE_HEADER_NAME_INVALID` | Rule declares an invalid header name. |

## Readiness (`GET /ready`)

HTTP status is **503** only when **every** configured registry reports `status: unhealthy` in the embedded `registry_health` array (same row shape as doctor). Otherwise **200** with `status: ready`. No registry configured → **200** (direct `upstream_url` routing still allowed).

## `check-config --strict` (`strict_findings`)

Structured rows are documented in [`check-config-strict-schema.md`](./check-config-strict-schema.md). Legacy integrations that assumed `strict_findings` was an array of strings must be updated.

## Related

- [`metrics-json.md`](./metrics-json.md) — JSON shape for `/metrics`
- [`doctor-json-schema.md`](./doctor-json-schema.md) — full doctor envelope
- [`route-explain-json-schema.md`](./route-explain-json-schema.md) — route-explain JSON fields
- [`operations-runbook.md`](./operations-runbook.md) — rollout and triage
