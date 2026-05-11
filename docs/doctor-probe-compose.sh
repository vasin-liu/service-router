#!/usr/bin/env bash
# Local helper for B09: boot compose upstreams, run doctor --probe-upstream, tear down.
# For the full release-acceptance artifact set (five §7 JSON + section-9-summary.generated.md), use
# docs/release-acceptance.sh — see docs/m2-release-readiness.md#m2-json-bundle-s9.
set -euo pipefail

CONFIG_PATH="${SERVICE_ROUTER_CONFIG:-config/mock-config.yaml}"
COMPOSE_FILE="${SERVICE_ROUTER_PROBE_COMPOSE_FILE:-.github/compose/doctor-probe.compose.yml}"

echo "[doctor-probe] compose up: ${COMPOSE_FILE}"
docker compose -f "${COMPOSE_FILE}" up -d

cleanup() {
  echo "[doctor-probe] compose down: ${COMPOSE_FILE}"
  docker compose -f "${COMPOSE_FILE}" down -v
}
trap cleanup EXIT

echo "[doctor-probe] wait ports 9000/9001"
for port in 9000 9001; do
  for i in {1..20}; do
    if (echo > /dev/tcp/127.0.0.1/"${port}") >/dev/null 2>&1; then
      break
    fi
    sleep 1
    if [[ "${i}" -eq 20 ]]; then
      echo "[doctor-probe] port ${port} not reachable after 20s"
      exit 1
    fi
  done
done

echo "[doctor-probe] run doctor --probe-upstream"
cargo run -- doctor --config "${CONFIG_PATH}" --probe-upstream --json
