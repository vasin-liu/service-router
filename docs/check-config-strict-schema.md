# `check-config --strict` JSON findings

When you run `cargo run -- check-config <path> --json --strict`, the summary includes **`strict_findings`**: an array of objects (not plain strings).

Each item:

| Field | Type | Description |
|:------|:-----|:------------|
| `code` | string | Stable upper-snake identifier (see below). |
| `message` | string | Human-readable explanation (same text as non-JSON mode prints). |
| `details` | object or omitted | Machine-oriented context; schema depends on `code`. |

Constants match `src/config/strict_check.rs`.

## Codes and `details`

| `code` | When | `details` keys |
|:-------|:-----|:-----------------|
| `ROUTES_EMPTY` | `routes:` is empty | _(none)_ |
| `DUPLICATE_ROUTE_ID` | Same `id` on more than one rule | `route_id`, `count` |
| `IDENTICAL_MATCHERS` | Two rules share path/method/header match keys | `rule_ids` (two strings) |
| `RULE_SHADOWED` | Broader rule is evaluated before a narrower one it fully covers | `covering_rule_id`, `shadowed_rule_id` |
| `UPSTREAM_AND_SERVICE_ID` | Rule sets both `upstream_url` and `service_id` | `rule_id` |
| `STRIP_PREFIX_UNREACHABLE` | Prefix matcher cannot yield paths starting with `strip_prefix` | `rule_id`, `strip_prefix`, `prefix` |

## Automation

Gate on `strict_passed == true`, or branch on `strict_findings[].code` for targeted fixes.

Non-JSON CLI output still prints **`message`** only (one line per finding).

## Regenerating `strict_check.rs` (maintainers)

After editing `tools/emit_strict_check.mjs`, run `node tools/emit_strict_check.mjs` and commit the generated `src/config/strict_check.rs`.
