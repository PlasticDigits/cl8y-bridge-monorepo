#!/usr/bin/env bash
# Deploy EVM contracts to second Anvil (anvil1, chain id 31338, V2 id 3) and merge EVM1_* into .deploy/local.env
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib-local-deploy-env.sh
source "$SCRIPT_DIR/lib-local-deploy-env.sh"

EVM1_RPC_URL="${EVM1_RPC_URL:-http://127.0.0.1:8546}"

LOG=$(mktemp)
trap 'rm -f "$LOG"' EXIT

cd "$REPO_ROOT/packages/contracts-evm"

if ! THIS_V2_CHAIN_ID=3 THIS_CHAIN_LABEL=evm_31338 \
    forge script script/DeployLocal.s.sol:DeployLocal \
    --broadcast \
    --rpc-url "$EVM1_RPC_URL" \
    --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
    --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
    2>&1 | tee "$LOG"; then
    qa_hint_forge_evm1_deploy_failed "$LOG"
    exit 1
fi

extract_log_addr() {
    local label=$1
    grep "$label" "$LOG" | head -1 | grep -oE '0x[a-fA-F0-9]{40}' | head -1 \
        || grep -A6 "$label" "$LOG" | grep -oE '0x[a-fA-F0-9]{40}' | head -1
}

EVM1_BRIDGE=$(extract_log_addr DEPLOYED_BRIDGE)
EVM1_CR=$(extract_log_addr DEPLOYED_CHAIN_REGISTRY)
EVM1_TR=$(extract_log_addr DEPLOYED_TOKEN_REGISTRY)
EVM1_LU=$(extract_log_addr DEPLOYED_LOCK_UNLOCK)

if [ -z "$EVM1_BRIDGE" ]; then
    echo "[ERROR] Could not parse DEPLOYED_BRIDGE from forge output (anvil1)." >&2
    qa_hint_evm_log_parse_failed "$LOG"
    exit 1
fi

write_deploy_env_evm1 "$EVM1_BRIDGE" "${EVM1_CR:-}" "${EVM1_TR:-}" "${EVM1_LU:-}"
echo ""
echo "[INFO] Recorded EVM1 (anvil1) addresses in $DEPLOY_ENV_FILE"
echo "  EVM1_BRIDGE_ADDRESS=$EVM1_BRIDGE"
echo "  EVM1_CHAIN_REGISTRY=${EVM1_CR:-}"
echo "  EVM1_TOKEN_REGISTRY_ADDRESS=${EVM1_TR:-}"
echo "  EVM1_LOCK_UNLOCK_ADDRESS=${EVM1_LU:-}"

if [ -f "$SCRIPT_DIR/merge-env-var.sh" ]; then
  chmod +x "$SCRIPT_DIR/merge-env-var.sh" 2>/dev/null || true
  for envf in "$REPO_ROOT/.env" "$REPO_ROOT/packages/operator/.env"; do
    "$SCRIPT_DIR/merge-env-var.sh" "$envf" EVM1_BRIDGE_ADDRESS "$EVM1_BRIDGE" 2>/dev/null || true
  done
  echo "[INFO] Merged EVM1 bridge address into existing .env files (if present)."
fi
