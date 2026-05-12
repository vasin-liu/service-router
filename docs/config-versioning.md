# Configuration Versioning Strategy

## Overview

`service-router` uses a `config_version` field in the top-level YAML to track the configuration schema version. This enables forward-compatible evolution of the config format while preserving backward compatibility for existing users.

## Current Version

The current config schema version is **`1`**.

## Usage

```yaml
config_version: "1"
server:
  host: "0.0.0.0"
  port: 8080
# ...
```

If `config_version` is omitted, the loader treats the file as version `"1"` (backward compatible with all existing configs).

## Versioning Rules

1. **Additive changes** (new optional fields with defaults) do NOT bump the version number
2. **Breaking changes** (field renames, removed fields, semantic changes) bump the version number
3. The loader will validate `config_version` and emit a clear error if the binary does not support the declared version

## Migration Path (Future)

When version `"2"` is introduced:

- The binary will support both `"1"` and `"2"` simultaneously
- `check-config` will warn if running with a deprecated version
- A `config-migrate` CLI command may be added to auto-upgrade config files
- The CHANGELOG will document all breaking changes per version

## Design Decisions

- Version is a string (not integer) to allow semver-like schemes if needed later
- Version is optional with a default, so zero existing configs need modification
- No automatic migration is applied -- upgrades are explicit and user-initiated
