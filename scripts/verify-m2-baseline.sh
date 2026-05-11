#!/usr/bin/env bash
# M2 engineering baseline: mirrors GitHub ci.yml mock profile gates (text encoding,
# build, unit tests, check-config --strict, doctor --json, route-explain smoke,
# config-snapshot).
# Optional: set M2_WITH_DOCKER_PROBE=1 to run compose + doctor --probe-upstream (needs Docker).
# For five JSON files under artifacts/release-acceptance/ (§9 archive), run docs/release-acceptance.sh
# afterward (see docs/m2-release-readiness.md).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "[m2-baseline] cargo check"
cargo check

echo "[m2-baseline] cargo test"
cargo test -- --nocapture

CONFIG="${SERVICE_ROUTER_CONFIG:-config/mock-config.yaml}"

echo "[m2-baseline] text encoding check"
python scripts/check-text-encoding.py

echo "[m2-baseline] check-config --strict (${CONFIG})"
cargo run -- check-config "${CONFIG}" --json --strict

echo "[m2-baseline] doctor --json"
cargo run -- doctor --config "${CONFIG}" --json

echo "[m2-baseline] route-explain smoke"
cargo run -- route-explain /api/orders/123 GET --config "${CONFIG}" --json

echo "[m2-baseline] config-snapshot (stdout)"
cargo run -- config-snapshot --config "${CONFIG}" -o -

if [[ "${M2_WITH_DOCKER_PROBE:-0}" == "1" ]]; then
  echo "[m2-baseline] docker compose up (doctor-probe.compose.yml)"
  docker compose -f .github/compose/doctor-probe.compose.yml up -d
  echo "[m2-baseline] wait for TCP 9000 9001"
  for p in 9000 9001; do
    for i in $(seq 1 20); do
      if (echo > /dev/tcp/127.0.0.1/$p) >/dev/null 2>&1; then
        break
      fi
      sleep 1
      if [[ "$i" -eq 20 ]]; then
        echo "port $p did not become reachable"
        exit 1
      fi
    done
  done
  echo "[m2-baseline] doctor --probe-upstream --json"
  cargo run -- doctor --config "${CONFIG}" --probe-upstream --json
  docker compose -f .github/compose/doctor-probe.compose.yml down -v
fi

echo "[m2-baseline] OK"
echo "[m2-baseline] tip: for §7 JSON artifacts (release-acceptance), run: SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL=0 bash docs/release-acceptance.sh (see docs/m2-release-readiness.md#m2-json-bundle-s9)"
