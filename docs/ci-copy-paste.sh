#!/usr/bin/env bash
# Standalone snippet: run locally or inline in CI "script:" after checkout + Rust toolchain.
set -eu
CONFIG="${SERVICE_ROUTER_CONFIG:-config/mock-config.yaml}"
PROBE_PATH="${SERVICE_ROUTER_SMOKE_PATH:-/api/orders/123}"
PROBE_METHOD="${SERVICE_ROUTER_SMOKE_METHOD:-GET}"

cargo check
cargo test -- --nocapture
cargo run -- check-config "${CONFIG}" --json --strict
cargo run -- doctor --config "${CONFIG}" --json
cargo run -- route-explain "${PROBE_PATH}" "${PROBE_METHOD}" --config "${CONFIG}" --json
cargo run -- config-snapshot --config "${CONFIG}" -o -
