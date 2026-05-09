# Proxy metrics JSON (`GET /metrics`)

In-process counters updated on each proxied request. Poll from automation or scrape into your log/TSDB pipeline.

## Response shape

```json
{
  "route_hits": {
    "orders-api": 42
  },
  "failure_reasons": {
    "no_matching_route": 3,
    "no_instances": 1,
    "registry_all_failed": 0
  }
}
```

- **`route_hits`**: key = routing `rule_id`, value = times that rule matched (path/method/headers) before upstream handling.
- **`failure_reasons`**: key = stable reason code (see below), value = count of terminal failures.

## Failure reason codes

| Code | When |
|:-----|:-----|
| `no_matching_route` | No routing rule matched the request (same 503 response body behavior as before). |
| `no_instances` | Matched rule but zero upstream instances (empty registry result or route without target). |
| `registry_http` | Registry HTTP client error. |
| `registry_unexpected` | Registry returned an unexpected payload. |
| `registry_not_found` | Service not found in registry. |
| `registry_auth_failed` | Registry login/auth failed. |
| `registry_all_failed` | All configured registries failed to resolve. |
| `upstream_connection` | Error talking to upstream after route resolution. |
| `ws_upgrade` | WebSocket upgrade error. |
| `body_read` | Failed to read client request body. |

## Notes

- Counters are **in-memory**; they reset on process restart.
- **`GET /metrics`** is registered on the Axum router **before** the catch-all proxy; it is not forwarded upstream.
