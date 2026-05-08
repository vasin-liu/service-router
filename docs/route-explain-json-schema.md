# Route Explain JSON Format (v1.0)

`route-explain --json` outputs a stable diagnostic envelope for CI/script parsing.

## Common fields

- `diagnostic_version`: string, currently `"1.0"`
- `matched`: boolean
- `config_path`: string
- `path`: string
- `method`: string

## Matched output example

```json
{
  "diagnostic_version": "1.0",
  "matched": true,
  "config_path": "config/mock-config.yaml",
  "path": "/api/orders/123",
  "method": "GET",
  "rule_id": "orders-api",
  "priority": 10,
  "target": "order-service",
  "rewritten_path": "/api/orders/123"
}
```

## Not matched output example

```json
{
  "diagnostic_version": "1.0",
  "matched": false,
  "config_path": "config/mock-config.yaml",
  "path": "/api/unknown",
  "method": "POST",
  "inspected_rules": 5,
  "diagnostics": [
    {
      "rule_id": "orders-api",
      "path": false,
      "method": true,
      "headers": true,
      "reasons": [
        "path '/api/unknown' does not match rule pattern"
      ],
      "suggestions": [
        {
          "code": "PATH_MISMATCH",
          "message": "check path matcher type/value for this rule",
          "command": "cargo run -- route-explain /api/unknown POST --config config/mock-config.yaml --verbose"
        }
      ]
    }
  ]
}
```

## CI usage notes

- Fail pipeline when:
  - `matched == false` for required probe routes
  - OR any suggestion `code` belongs to a disallowed set (policy-based)
- Prefer `--verbose` in CI triage jobs for full rule inspection.
