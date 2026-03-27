#!/usr/bin/env bash
# One-shot QA server bootstrap: Docker chains + deploy + operator + canceler + env files.
# Prerequisites: repo-root .env (copy from packages/operator/.env.example) with DATABASE_URL and keys.
#
# Laptop devs still need SSH -L port forwards to reach 127.0.0.1 services on this host.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

export QA_SHARED_HOST=1

if [ ! -f "$REPO_ROOT/.env" ]; then
  echo "[start-qa] Missing $REPO_ROOT/.env"
  echo "  Copy: cp packages/operator/.env.example .env"
  echo "  Set DATABASE_URL (e.g. postgres://operator:operator@127.0.0.1:5433/operator), EVM_PRIVATE_KEY, TERRA_MNEMONIC, SOLANA_PRIVATE_KEY, etc."
  exit 1
fi

# Tear down any previous bridge QA processes / containers so we start clean
echo "==> Tearing down existing bridge stack (canceler, operator, Docker) if present..."
"$REPO_ROOT/scripts/canceler-ctl.sh" stop-all 2>/dev/null || true
"$REPO_ROOT/scripts/operator-ctl.sh" stop 2>/dev/null || true
docker compose down 2>/dev/null || true

set -a
# shellcheck source=/dev/null
source "$REPO_ROOT/.env"
# shellcheck source=/dev/null
source "$REPO_ROOT/scripts/qa/qa-host.env"
set +a

echo "==> Starting Docker Compose (bridge infrastructure)..."
if docker compose up --help 2>&1 | grep -q -- '--wait'; then
  docker compose up -d --wait
else
  docker compose up -d
fi

echo "==> Waiting for chain RPCs (up to ~90s)..."
for _ in $(seq 1 45); do
  if curl -sf -X POST -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
    "$EVM_RPC_URL" >/dev/null 2>&1 \
    && curl -sf "$TERRA_LCD_URL/cosmos/base/tendermint/v1beta1/blocks/latest" >/dev/null 2>&1 \
    && curl -sf -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
      "$SOLANA_RPC_URL" >/dev/null 2>&1; then
    echo "==> RPCs responding."
    break
  fi
  sleep 2
done

docker compose ps

echo "==> Database migrations (operator)..."
make operator-migrate

echo "==> Deploy contracts + setup-bridge (uses TERRA_RPC_URL / TERRA_LCD_URL from qa-host.env)..."
# Force remapped LocalTerra URLs into deploy / setup-bridge (override stale .env from operator template).
export TERRA_RPC_URL="http://127.0.0.1:${E2E_TERRA_RPC_PORT:-26658}"
export TERRA_LCD_URL="http://127.0.0.1:${E2E_TERRA_LCD_PORT:-1318}"
export TERRA_RPC_URL TERRA_LCD_URL EVM_RPC_URL SOLANA_RPC_URL
make deploy

echo "==> Merging deploy outputs into repo-root .env for operator..."
if [ -f "$REPO_ROOT/.deploy/local.env" ]; then
  chmod +x "$REPO_ROOT/scripts/merge-env-var.sh" 2>/dev/null || true
  set -a
  # shellcheck source=/dev/null
  source "$REPO_ROOT/.deploy/local.env"
  set +a
  if [ -n "${SOLANA_PROGRAM_ID:-}" ]; then
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" SOLANA_PROGRAM_ID "$SOLANA_PROGRAM_ID"
  fi
  "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" TERRA_RPC_URL "${TERRA_RPC_URL}"
  "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" TERRA_LCD_URL "${TERRA_LCD_URL}"
fi

echo "==> Writing .env.e2e.local + packages/frontend/.env.local..."
"$REPO_ROOT/scripts/qa/write-qa-env-e2e.sh"

echo "==> Starting operator (API $OPERATOR_API_URL)..."
"$REPO_ROOT/scripts/operator-ctl.sh" start

echo "==> Starting canceler..."
"$REPO_ROOT/scripts/canceler-ctl.sh" start

# Defaults match scripts/status.sh and canceler-ctl (instance 1)
CANCELER_HEALTH_URL="${CANCELER_HEALTH_URL:-http://127.0.0.1:9099}"

echo ""
echo "==> Verifying operator and canceler health..."
sleep 2
if ! curl -sf "${OPERATOR_API_URL}/health" >/dev/null; then
  echo "[start-qa] ERROR: Operator health check failed (${OPERATOR_API_URL}/health)" >&2
  echo "  See: $REPO_ROOT/.operator.log" >&2
  exit 1
fi
if ! curl -sf "${CANCELER_HEALTH_URL}/health" >/dev/null; then
  echo "[start-qa] ERROR: Canceler health check failed (${CANCELER_HEALTH_URL}/health)" >&2
  exit 1
fi
echo "==> Operator and canceler responded OK."

# Ports for SSH -L local:remote — remote is always 127.0.0.1 on this machine
TERRA_RPC_PORT="${E2E_TERRA_RPC_PORT}"
TERRA_LCD_PORT="${E2E_TERRA_LCD_PORT}"
OP_PORT="${OPERATOR_API_PORT}"
# shellcheck disable=SC2001
SOL_PORT=$(echo "${SOLANA_RPC_URL}" | sed -n 's/.*:\([0-9][0-9]*\).*/\1/p')
WS_PORT=$(echo "${SOLANA_WS_URL}" | sed -n 's/.*:\([0-9][0-9]*\).*/\1/p')
# shellcheck disable=SC2001
CAN_PORT=$(echo "${CANCELER_HEALTH_URL}" | sed -n 's/.*:\([0-9][0-9]*\).*/\1/p')
SOL_PORT="${SOL_PORT:-8899}"
WS_PORT="${WS_PORT:-8900}"
CAN_PORT="${CAN_PORT:-9099}"

# Printed ssh/scp use whoami@(QA_SSH_HOST or this machine's hostname) so the account matches whoever runs make start-qa.
if [ -n "${QA_SSH_HOST:-}" ]; then
  SSH_DEST="$(whoami)@${QA_SSH_HOST}"
else
  SSH_DEST="$(whoami)@$(hostname -f 2>/dev/null || hostname)"
fi
# Non-default SSH port: OpenSSH uses ssh -p / scp -P
QA_SSH_PORT="${QA_SSH_PORT:-22}"
SSH_P_ARGS=""
SCP_P_ARGS=""
if [ "${QA_SSH_PORT}" != "22" ]; then
  SSH_P_ARGS="-p ${QA_SSH_PORT} "
  SCP_P_ARGS="-P ${QA_SSH_PORT} "
fi

echo ""
echo "========================================================================"
echo "  start-qa finished successfully on this host."
echo "========================================================================"
echo ""
echo "  --- Laptop workflow (do these on your laptop, in order) ---"
echo "  Full doc: scripts/qa/README.md  (section: On your laptop)"
echo ""
echo "  Optional next time you run make start-qa here — bake SSH host/port into the lines below:"
echo "    QA_SSH_HOST   hostname or IP as seen from the laptop (user is $(whoami) from this shell)"
echo "    QA_SSH_PORT   if SSH is not on port 22 (adds -p / -P to ssh and scp)"
echo ""
echo "  Step 1 — SSH port forwards (run on laptop; keep this terminal open)."
echo "           Use 127.0.0.1 on both sides to avoid IPv6 [::1] bind issues on some desktops."
echo ""
echo "ssh -4 -N ${SSH_P_ARGS}\\"
echo "  -L 127.0.0.1:${SOL_PORT}:127.0.0.1:${SOL_PORT} \\"
echo "  -L 127.0.0.1:${WS_PORT}:127.0.0.1:${WS_PORT} \\"
echo "  -L 127.0.0.1:9900:127.0.0.1:9900 \\"
echo "  -L 127.0.0.1:8545:127.0.0.1:8545 \\"
echo "  -L 127.0.0.1:8546:127.0.0.1:8546 \\"
echo "  -L 127.0.0.1:${TERRA_RPC_PORT}:127.0.0.1:${TERRA_RPC_PORT} \\"
echo "  -L 127.0.0.1:${TERRA_LCD_PORT}:127.0.0.1:${TERRA_LCD_PORT} \\"
echo "  -L 127.0.0.1:${OP_PORT}:127.0.0.1:${OP_PORT} \\"
echo "  -L 127.0.0.1:${CAN_PORT}:127.0.0.1:${CAN_PORT} \\"
echo "  ${SSH_DEST}"
echo ""
echo "  Step 2 — Copy .deploy/local.env from this host into your laptop repo clone:"
echo "    scp ${SCP_P_ARGS}${SSH_DEST}:${REPO_ROOT}/.deploy/local.env .deploy/local.env"
echo ""
echo "  Step 3 — Generate packages/frontend/.env.local (URLs + bridge addresses):"
echo "    ./scripts/qa/write-frontend-env-local.sh"
echo ""
echo "  Step 4 — Install deps and run Vite:"
echo "    cd packages/frontend && npm ci && npm run dev"
echo ""
echo "  Step 5 — Open the URL Vite prints (e.g. http://localhost:3000)."
echo ""
echo "  On this server: make status  (expect operator + canceler running)"
echo ""
