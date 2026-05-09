# Doctor JSON Format (v1.0)

`doctor --json` returns a stable diagnostic envelope for CI and automation scripts.

## Common fields

- `diagnostic_version`: string, currently `"1.0"`
- `status`: `"pass"` or `"fail"`
- `config_path`: string
- `probe_upstream_enabled`: boolean
- `registry_health`: array
- `upstream_probe`: array (non-empty only when `--probe-upstream` is enabled)

## `registry_health` item

- `priority`: number
- `kind`: string (for example `Mock`, `Nacos`, `Eureka`, `Kubernetes`)
- `status`: `"healthy" | "degraded" | "unhealthy"`
- `message`: optional string (present for degraded/unhealthy)

## `upstream_probe` item

- `route_id`: string
- `target_type`: `"upstream_url"` or `"service_id"`
- `reachable`: boolean
- Additional fields by target type:
  - `upstream_url`: `host`, `port` or `error`
  - `service_id`: `service_id`, `resolved_instances` or `error`

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
- In triage jobs, enable `--probe-upstream` for network-level diagnosis.
- For environment-sensitive checks, run both:
  - baseline: `doctor --json`
  - connectivity: `doctor --probe-upstream --json`
