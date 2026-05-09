# Doctor JSON Format (v1.0)

`doctor --json` returns a stable diagnostic envelope for CI and automation scripts.

## Common fields

- `diagnostic_version`: string, currently `"1.0"`
- `status`: `"pass"` or `"fail"`
- `config_path`: string
- `probe_upstream_enabled`: boolean
- `registry_health`: array
- `registry_endpoint_probe`: array (populated when `--probe-upstream` runs; empty otherwise)
- `upstream_probe`: array (populated when `--probe-upstream` runs; empty otherwise)

## `registry_health` item

- `priority`: number
- `kind`: string (for example `Mock`, `Nacos`, `Eureka`, `Kubernetes`)
- `status`: `"healthy" | "degraded" | "unhealthy"`
- `message`: optional string (present for degraded/unhealthy)

## `registry_endpoint_probe` item (`--probe-upstream`)

TCP connect (2s timeout) to the **remote** registry’s configured API address. **Mock** registries are skipped (they have no remote endpoint).

Successful parse and probe:

- `kind`: `"Nacos" | "Eureka" | "Kubernetes"`
- `priority`: number
- `configured`: string (raw value from YAML: URL or `host:port`)
- `host`, `port`: resolved values used for the socket
- `reachable`: boolean

Unreachable or parse failure:

- `reachable`: false
- `failure_code`: `"TCP_UNREACHABLE"` (probe failed) or `"ENDPOINT_PARSE_ERROR"` (could not parse host/port from config)
- `reason`: human-readable explanation

## `upstream_probe` item

- `route_id`: string
- `target_type`: `"upstream_url"` or `"service_id"`
- `reachable`: boolean
- Additional fields by target type:
  - `upstream_url`: `host`, `port` or `error`
  - `service_id`: `service_id`, `resolved_instances` or `error`

### Kubernetes registry and `service_id` probes

When the configured registry for a route is **Kubernetes**, resolving `service_id` uses the same logic as proxy discovery:

1. Load `Service` TCP targets from `spec.ports` (skips `UDP` / `SCTP` for HTTP proxy use).
2. Query Core `Endpoints`, then `EndpointSlice` if no instances are produced.
3. Filter backend port rows to those targets (numeric `targetPort` or named port entry match).

`resolved_instances` is the count of distinct `(host, port)` pairs after that filtering. Each pair is TCP-probed for `reachable` when `--probe-upstream` is set.

## Pass example

```json
{
  "diagnostic_version": "1.0",
  "status": "pass",
  "config_path": "config/mock-config.yaml",
  "probe_upstream_enabled": false,
  "registry_health": [
    {
      "priority": 1,
      "kind": "Mock",
      "status": "healthy"
    }
  ],
  "registry_endpoint_probe": [],
  "upstream_probe": []
}
```

## Fail example (`--probe-upstream`)

```json
{
  "diagnostic_version": "1.0",
  "status": "fail",
  "config_path": "config/mock-config.yaml",
  "probe_upstream_enabled": true,
  "registry_health": [
    {
      "priority": 1,
      "kind": "Mock",
      "status": "healthy"
    }
  ],
  "registry_endpoint_probe": [],
  "upstream_probe": [
    {
      "route_id": "orders-api",
      "target_type": "service_id",
      "service_id": "order-service",
      "resolved_instances": 1,
      "reachable": true
    },
    {
      "route_id": "catch-all",
      "target_type": "service_id",
      "service_id": "api-gateway",
      "resolved_instances": 1,
      "reachable": false
    }
  ]
}
```

## CI policy suggestions

- Block pipeline when `status == "fail"`.
- In triage jobs, enable `--probe-upstream` for network-level diagnosis (registry API endpoints and resolved route upstreams).
- For environment-sensitive checks, run both:
  - baseline: `doctor --json`
  - connectivity: `doctor --probe-upstream --json`
