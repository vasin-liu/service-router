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
  "remediation_outline": [
    {
      "code": "PATH_MISMATCH",
      "message": "ensure the path starts with '/api/orders' or adjust the rule prefix / try a more specific rule first",
      "command": "cargo run -- route-explain /api/unknown POST --config config/mock-config.yaml --verbose"
    }
  ],
  "diagnostics": [
    {
      "rule_id": "orders-api",
      "path": false,
      "method": true,
      "headers": true,
      "reasons": [
        "path '/api/unknown' does not match rule (prefix '/api/orders' (path must start with this))"
      ],
      "suggestions": [
        {
          "code": "PATH_MISMATCH",
          "message": "ensure the path starts with '/api/orders' or adjust the rule prefix / try a more specific rule first",
          "command": "cargo run -- route-explain /api/unknown POST --config config/mock-config.yaml --verbose"
        }
      ]
    }
  ]
}
```

- `remediation_outline`: first suggestion per distinct `code` across inspected rules (stable de-duplication). Same objects also appear under each `diagnostics[].suggestions` where applicable.
- Suggestion codes include: `PATH_MISMATCH`, `METHOD_MISMATCH`, `HEADER_MISSING`, `HEADER_VALUE_MISMATCH`, `RULE_HEADER_NAME_INVALID`.

## CI usage notes

- Fail pipeline when:
  - `matched == false` for required probe routes
  - OR any suggestion `code` belongs to a disallowed set (policy-based)
- Prefer `--verbose` in CI triage jobs for full rule inspection.
- Prefer `remediation_outline` for compact “next action” signals; use `diagnostics` for per-rule detail.
