#!/usr/bin/env bash
# Standalone snippet: run locally or inline in CI "script:" after checkout + Rust toolchain.
set -eu
CONFIG="${SERVICE_ROUTER_CONFIG:-config/mock-config.yaml}"
PROBE_PATH="${SERVICE_ROUTER_SMOKE_PATH:-/api/orders/123}"
PROBE_METHOD="${SERVICE_ROUTER_SMOKE_METHOD:-GET}"

cargo check
cargo test -- --nocapture
python scripts/check-text-encoding.py
cargo run -- check-config "${CONFIG}" --json --strict
cargo run -- doctor --config "${CONFIG}" --json
cargo run -- route-explain "${PROBE_PATH}" "${PROBE_METHOD}" --config "${CONFIG}" --json
cargo run -- config-snapshot --config "${CONFIG}" -o -

# Full release-acceptance bundle (five §7 *.json + section-9-summary.generated.md under
# artifacts/release-acceptance/): SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL=0 bash docs/release-acceptance.sh
# See docs/release-acceptance-matrix.md §7–§9.
