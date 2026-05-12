# Route Explain JSON Format (v1.0)

`route-explain --json` outputs a stable diagnostic envelope for CI/script parsing.

## Common fields

- `diagnostic_version`: string, currently `"1.0"`
- `matched`: boolean
- `config_path`: string
- `request_file`: string or null — set when `--request-file` was used (path to the sample file)
- `path`: string
- `method`: string

## Request sample file (`--request-file`)

Load path/method/headers from a **YAML** (default) or **JSON** file (if extension is `.json`).

YAML shape:

```yaml
path: "/api/orders/123"
method: "GET"
headers:
  x-env: "dev"
```

JSON shape:

```json
{
  "path": "/api/orders/123",
  "method": "POST",
  "headers": { "X-Trace": "1" }
}
```

- `path` (required): request URI path.
- `method` (optional): defaults to `GET`.
- `headers` (optional): string map. Repeated `--header key:value` on the CLI **overrides** the same key from the file.

Example: `config/route-explain-request-sample.yaml`.

## Matched envelope

When **`matched`** is **`true`**:

- `response_headers`: object or **`null`** — outbound response headers configured on the matched rule (`null` when unset); keys and values mirror the compiled rule (HTTP proxy only when traffic is forwarded; **`route-explain` does not run the proxy**).

## Matched output example

```json
{
  "diagnostic_version": "1.0",
  "matched": true,
  "config_path": "config/mock-config.yaml",
  "request_file": null,
  "path": "/api/orders/123",
  "method": "GET",
  "rule_id": "orders-api",
  "priority": 10,
  "target": "order-service",
  "rewritten_path": "/api/orders/123",
  "response_headers": null
}
```

## Not matched output example

```json
{
  "diagnostic_version": "1.0",
  "matched": false,
  "config_path": "config/mock-config.yaml",
  "request_file": null,
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
