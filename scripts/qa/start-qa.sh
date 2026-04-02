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

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  _QA_HI=$'\033[93;1m'
  _QA_RST=$'\033[0m'
else
  _QA_HI=''
  _QA_RST=''
fi
printf '%b\n' "${_QA_HI}┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓${_QA_RST}"
printf '%b\n' "${_QA_HI}┃  REMINDER: SSH tunnel + laptop steps are printed at the END of this run ┃${_QA_RST}"
printf '%b\n' "${_QA_HI}┃  Reprint anytime: make qa-tunnel-help                                    ┃${_QA_RST}"
printf '%b\n' "${_QA_HI}┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛${_QA_RST}"
echo ""

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

EVM1_RPC_URL="${EVM1_RPC_URL:-http://127.0.0.1:8546}"

echo "==> Starting Docker Compose (anvil + anvil1 + localterra + postgres + solana)..."
_qa_compose_up_failed() {
  echo "" >&2
  echo "[start-qa] ERROR: docker compose did not start all services (LocalTerra often exits on bad volume or port conflict)." >&2
  echo "[start-qa] --- docker compose ps -a --format 'table {{.Name}}\t{{.Status}}\t{{.State}}' ---" >&2
  docker compose ps -a --format 'table {{.Name}}\t{{.Status}}\t{{.State}}' 2>&1 || true
  echo "[start-qa] --- localterra logs (last 100 lines) ---" >&2
  docker compose logs localterra --tail 100 2>&1 || true
  echo "" >&2
  echo "[start-qa] Try: check host ports (E2E_TERRA_RPC_PORT / E2E_TERRA_LCD_PORT vs defaults 26657/1317);" >&2
  echo "  reset chain data:  docker compose down -v   # or: docker volume ls | grep localterra  then docker volume rm <name>" >&2
  echo "  If logs mention 'validator set' / 'empty set' / replay at high height: persisted .terra volume is bad — down -v is required." >&2
  echo "  ARM host + linux/amd64 image: install QEMU/binfmt or use an amd64 runner." >&2
}

if docker compose up --help 2>&1 | grep -q -- '--wait'; then
  if ! docker compose up -d --wait anvil anvil1 localterra postgres solana; then
    _qa_compose_up_failed
    exit 1
  fi
else
  if ! docker compose up -d anvil anvil1 localterra postgres solana; then
    _qa_compose_up_failed
    exit 1
  fi
fi

echo "==> Syncing LocalTerra host ports from docker compose (LCD/RPC URLs for wait + deploy + .env.e2e.local)..."
# shellcheck source=/dev/null
source "$REPO_ROOT/scripts/qa/sync-localterra-compose-ports.sh"
echo "[start-qa] LocalTerra LCD=$TERRA_LCD_URL RPC=$TERRA_RPC_URL"

echo "==> Waiting for chain RPCs: EVM + EVM1 + Terra + Solana (up to ~90s)..."
for _ in $(seq 1 45); do
  if curl -sf -X POST -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
    "$EVM_RPC_URL" >/dev/null 2>&1 \
    && curl -sf -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
      "$EVM1_RPC_URL" >/dev/null 2>&1 \
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

echo "==> Terra WASM artifacts (required for deploy-terra --cw20)..."
make ensure-terra-artifacts

echo "==> Deploy contracts + setup-bridge (TERRA_* synced from compose publish ports above)..."
export TERRA_RPC_URL TERRA_LCD_URL EVM_RPC_URL EVM1_RPC_URL SOLANA_RPC_URL
make deploy

echo "==> Sync .deploy/local.env EVM + EVM1 addresses from forge broadcast (repair stale/parsed drift)..."
chmod +x "$REPO_ROOT/scripts/qa/sync-local-env-from-forge-broadcast.sh" 2>/dev/null || true
"$REPO_ROOT/scripts/qa/sync-local-env-from-forge-broadcast.sh"

echo "==> Optional: fund extra Solana QA wallets (SOLANA_QA_AIRDROP_WALLETS from .env)..."
"$REPO_ROOT/scripts/solana/airdrop-qa-wallets.sh"

echo "==> Optional: fund Anvil / Anvil1 / LocalTerra gas (EVM_QA_FUND_WALLETS / TERRA_QA_FUND_WALLETS from .env)..."
"$REPO_ROOT/scripts/qa/fund-qa-gas-wallets.sh"

FE_DIR="$REPO_ROOT/packages/frontend"
if [ "${START_QA_SKIP_NPM_CI:-}" != "1" ] && command -v npm >/dev/null 2>&1; then
  _need_npm_ci=0
  if [ ! -d "$FE_DIR/node_modules" ]; then
    _need_npm_ci=1
  elif [ -f "$FE_DIR/package-lock.json" ] && [ "$FE_DIR/package-lock.json" -nt "$FE_DIR/node_modules" ]; then
    _need_npm_ci=1
  fi
  if [ "$_need_npm_ci" -eq 1 ]; then
    echo "==> Frontend dependencies: npm ci ($FE_DIR) — first run, missing node_modules, or package-lock newer than node_modules..."
    ( cd "$FE_DIR" && npm ci )
  fi
elif ! command -v npm >/dev/null 2>&1; then
  echo "[start-qa] WARN: npm not on PATH — ensure packages/frontend deps are installed before qa:full-token-setup." >&2
fi

echo "==> Full E2E token matrix + cross-chain registration (e2e-infra) + Solana register_token..."
( cd "$FE_DIR" && npm run qa:full-token-setup )

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
  if [ -n "${EVM1_BRIDGE_ADDRESS:-}" ]; then
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" EVM_CHAINS_COUNT "1"
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" EVM_CHAIN_1_NAME "anvil1"
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" EVM_CHAIN_1_CHAIN_ID "31338"
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" EVM_CHAIN_1_THIS_CHAIN_ID "3"
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" EVM_CHAIN_1_RPC_URL "${EVM1_RPC_URL}"
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" EVM_CHAIN_1_BRIDGE_ADDRESS "${EVM1_BRIDGE_ADDRESS}"
    "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" EVM_CHAIN_1_FINALITY_BLOCKS "1"
  fi
fi

echo "==> Writing .env.e2e.local + packages/frontend/.env.local..."
"$REPO_ROOT/scripts/qa/write-qa-env-e2e.sh"
echo "[start-qa] If Vite is already running for manual QA, restart it so new VITE_* (bridge, LockUnlock, tokens) load."

# Operator requires SOLANA_PRIVATE_KEY (base58) when SOLANA_RPC_URL is set; deploy scripts use SOLANA_KEYPAIR JSON only.
if [ -z "${SOLANA_PRIVATE_KEY:-}" ] && [ -n "${SOLANA_RPC_URL:-}" ] && [ -f "$REPO_ROOT/.env" ]; then
  _sol_kp="${SOLANA_KEYPAIR:-${HOME}/.config/solana/id.json}"
  _sol_node_cd="$REPO_ROOT/packages/contracts-solana"
  if [ ! -f "$_sol_node_cd/node_modules/@solana/web3.js/package.json" ]; then
    _sol_node_cd="$REPO_ROOT/packages/frontend"
  fi
  if [ -f "$_sol_kp" ] && command -v node >/dev/null 2>&1 && [ -f "$_sol_node_cd/node_modules/@solana/web3.js/package.json" ]; then
    SOLANA_PRIVATE_KEY="$(
      cd "$_sol_node_cd" && KP="$_sol_kp" node -e "
        const fs = require('fs');
        const bs58 = require('bs58');
        const { Keypair } = require('@solana/web3.js');
        const raw = JSON.parse(fs.readFileSync(process.env.KP, 'utf8'));
        process.stdout.write(bs58.encode(Keypair.fromSecretKey(Uint8Array.from(raw)).secretKey));
      "
    )" || true
    if [ -n "${SOLANA_PRIVATE_KEY:-}" ]; then
      export SOLANA_PRIVATE_KEY
      chmod +x "$REPO_ROOT/scripts/merge-env-var.sh" 2>/dev/null || true
      "$REPO_ROOT/scripts/merge-env-var.sh" "$REPO_ROOT/.env" SOLANA_PRIVATE_KEY "$SOLANA_PRIVATE_KEY"
      echo "[start-qa] Set SOLANA_PRIVATE_KEY from ${_sol_kp} for operator (merged into .env)."
    fi
  fi
fi

echo "==> Starting operator (API $OPERATOR_API_URL)..."
"$REPO_ROOT/scripts/operator-ctl.sh" start

echo "==> Starting canceler..."
"$REPO_ROOT/scripts/canceler-ctl.sh" start

# Instance 1: HEALTH_PORT = HEALTH_PORT_BASE + id - 1 (see scripts/canceler-ctl.sh)
_default_canceler_health_port="${HEALTH_PORT_BASE:-9099}"
CANCELER_HEALTH_URL="${CANCELER_HEALTH_URL:-http://127.0.0.1:${_default_canceler_health_port}}"

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

echo ""
echo "========================================================================"
echo "  start-qa finished successfully on this host."
echo "========================================================================"
chmod +x "$REPO_ROOT/scripts/qa/print-qa-tunnel-instructions.sh" 2>/dev/null || true
"$REPO_ROOT/scripts/qa/print-qa-tunnel-instructions.sh"
