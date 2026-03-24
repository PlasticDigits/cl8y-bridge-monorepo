#!/usr/bin/env bash
# Deploy EVM contracts to local Anvil and record bridge addresses in .deploy/local.env
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib-local-deploy-env.sh
source "$SCRIPT_DIR/lib-local-deploy-env.sh"

LOG=$(mktemp)
trap 'rm -f "$LOG"' EXIT

cd "$REPO_ROOT/packages/contracts-evm"

if ! forge script script/DeployLocal.s.sol:DeployLocal \
    --broadcast \
    --rpc-url http://localhost:8545 \
    --sender 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
    --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
    2>&1 | tee "$LOG"; then
    qa_hint_forge_evm_deploy_failed "$LOG"
    exit 1
fi

# Forge may print the address on the same line as the label or on the following line(s).
extract_log_addr() {
    local label=$1
    grep "$label" "$LOG" | head -1 | grep -oE '0x[a-fA-F0-9]{40}' | head -1 \
        || grep -A6 "$label" "$LOG" | grep -oE '0x[a-fA-F0-9]{40}' | head -1
}

EVM_BRIDGE=$(extract_log_addr DEPLOYED_BRIDGE)
EVM_CR=$(extract_log_addr DEPLOYED_CHAIN_REGISTRY)

if [ -z "$EVM_BRIDGE" ]; then
    echo "[ERROR] Could not parse DEPLOYED_BRIDGE from forge output." >&2
    qa_hint_evm_log_parse_failed "$LOG"
    exit 1
fi

write_deploy_env_evm "$EVM_BRIDGE" "${EVM_CR:-}"
echo ""
echo "[INFO] Recorded EVM addresses in $DEPLOY_ENV_FILE"
echo "  EVM_BRIDGE_ADDRESS=$EVM_BRIDGE"
echo "  EVM_CHAIN_REGISTRY=${EVM_CR:-}"
