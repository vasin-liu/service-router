#!/usr/bin/env bash
# Release acceptance runner: executes matrix command gates and stores JSON outputs plus section-9-summary.generated.md.
set -euo pipefail

CONFIG_PATH="${SERVICE_ROUTER_CONFIG:-config/mock-config.yaml}"
SMOKE_PATH="${SERVICE_ROUTER_SMOKE_PATH:-/api/orders/123}"
SMOKE_METHOD="${SERVICE_ROUTER_SMOKE_METHOD:-GET}"
ARTIFACT_DIR="${SERVICE_ROUTER_ACCEPTANCE_OUT:-artifacts/release-acceptance}"
RUN_GLOBAL_GATES="${SERVICE_ROUTER_ACCEPTANCE_RUN_GLOBAL:-1}"
ALLOW_PROBE_FAIL="${SERVICE_ROUTER_ACCEPTANCE_ALLOW_PROBE_FAIL:-0}"

mkdir -p "${ARTIFACT_DIR}"

echo "[release-acceptance] config: ${CONFIG_PATH}"
echo "[release-acceptance] smoke: ${SMOKE_METHOD} ${SMOKE_PATH}"
echo "[release-acceptance] output: ${ARTIFACT_DIR}"

if [[ "${RUN_GLOBAL_GATES}" == "1" ]]; then
  echo "[release-acceptance] global gates: text encoding + cargo check + cargo test"
  python scripts/check-text-encoding.py
  cargo check
  cargo test -- --nocapture
fi

echo "[release-acceptance] check-config --strict"
cargo run -- check-config --config "${CONFIG_PATH}" --json --strict \
  | tee "${ARTIFACT_DIR}/check-config.json"

echo "[release-acceptance] doctor"
cargo run -- doctor --config "${CONFIG_PATH}" --json \
  | tee "${ARTIFACT_DIR}/doctor.json"

echo "[release-acceptance] doctor --probe-upstream"
set +e
cargo run -- doctor --config "${CONFIG_PATH}" --probe-upstream --json \
  | tee "${ARTIFACT_DIR}/doctor-probe.json"
probe_exit=$?
set -e
if [[ "${probe_exit}" -ne 0 && "${ALLOW_PROBE_FAIL}" != "1" ]]; then
  echo "[release-acceptance] doctor --probe-upstream failed (exit ${probe_exit})"
  exit "${probe_exit}"
fi

echo "[release-acceptance] route-explain smoke"
cargo run -- route-explain "${SMOKE_PATH}" "${SMOKE_METHOD}" --config "${CONFIG_PATH}" --json \
  | tee "${ARTIFACT_DIR}/route-explain-smoke.json"

echo "[release-acceptance] config-snapshot (redacted)"
cargo run -- config-snapshot --config "${CONFIG_PATH}" -o "${ARTIFACT_DIR}/config-snapshot.json"

echo "[release-acceptance] section-9 summary (markdown)"
python scripts/summarize-section9-release-acceptance.py \
  --artifacts-dir "${ARTIFACT_DIR}" \
  > "${ARTIFACT_DIR}/section-9-summary.generated.md"

echo "[release-acceptance] done"
