#!/bin/bash
# Configure cross-chain bridge connections
#
# This script registers each chain with the other bridge contract
# and adds supported tokens.
#
# Prerequisites:
# - Both bridges deployed (EVM and Terra)
# - LocalTerra container running
# - Environment variables set (or pass as args)
#
# Usage:
#   ./scripts/setup-bridge.sh
# Or set EVM_BRIDGE_ADDRESS / TERRA_BRIDGE_ADDRESS explicitly (overrides .deploy/local.env).
#
# After `make deploy-evm` and `make deploy-terra`, addresses are stored in .deploy/local.env
# and loaded automatically when unset.
#
# Solana: set SOLANA_PROGRAM_ID, or rely on packages/contracts-solana/target/deploy/cl8y_bridge-keypair.json
# after `make deploy-solana` (script derives the program id automatically).

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ ! -f "$REPO_ROOT/scripts/lib-local-deploy-env.sh" ]; then
    echo "[ERROR] Missing $REPO_ROOT/scripts/lib-local-deploy-env.sh" >&2
    exit 1
fi
# shellcheck source=lib-local-deploy-env.sh
source "$REPO_ROOT/scripts/lib-local-deploy-env.sh"
load_local_deploy_env

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_NODE="http://localhost:26657"
TERRA_LCD="${TERRA_LCD_URL:-http://localhost:1317}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"
CONTAINER_NAME="${LOCALTERRA_CONTAINER:-cl8y-bridge-monorepo-localterra-1}"

# Contract addresses (must be set)
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
EVM_CHAIN_REGISTRY="${EVM_CHAIN_REGISTRY:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"
SOLANA_PROGRAM_ID="${SOLANA_PROGRAM_ID:-}"
SOLANA_RPC_URL="${SOLANA_RPC_URL:-http://localhost:8899}"
# Used by Anchor/ts-mocha in setup_solana_side (same default as Solana CLI)
SOLANA_KEYPAIR="${SOLANA_KEYPAIR:-${HOME}/.config/solana/id.json}"

# Resolve program id: explicit SOLANA_PROGRAM_ID, else Anchor deploy keypair (after `make deploy-solana`)
SOLANA_DEPLOY_KEYPAIR="${REPO_ROOT}/packages/contracts-solana/target/deploy/cl8y_bridge-keypair.json"
if [ -z "${SOLANA_PROGRAM_ID}" ] && [ -f "$SOLANA_DEPLOY_KEYPAIR" ] && command -v solana-keygen >/dev/null 2>&1; then
    SOLANA_PROGRAM_ID=$(solana-keygen pubkey "$SOLANA_DEPLOY_KEYPAIR" 2>/dev/null || true)
fi

# Keys
EVM_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
TERRA_KEY="${TERRA_KEY_NAME:-test1}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Run terrad command via docker exec
terrad_exec() {
    docker exec "$CONTAINER_NAME" terrad "$@"
}

# Validate addresses
check_addresses() {
    if [ -z "$EVM_BRIDGE_ADDRESS" ]; then
        log_error "EVM_BRIDGE_ADDRESS not set (and not loaded from .deploy/local.env)"
        qa_hint_evm_bridge_missing
        exit 1
    fi

    if [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_error "TERRA_BRIDGE_ADDRESS not set (and not loaded from .deploy/local.env)"
        qa_hint_terra_bridge_missing
        exit 1
    fi

    # Check LocalTerra container is running
    if ! docker ps --format '{{.Names}}' | grep -q "$CONTAINER_NAME"; then
        log_error "LocalTerra container not running: $CONTAINER_NAME"
        qa_hint_localterra_not_running "$CONTAINER_NAME"
        exit 1
    fi
    
    log_info "EVM Bridge: $EVM_BRIDGE_ADDRESS"
    log_info "Terra Bridge: $TERRA_BRIDGE_ADDRESS"
}

# Register Terra chain on EVM bridge
setup_evm_side() {
    log_info "=== Configuring EVM Side ==="
    
    # Compute Terra chain key: keccak256(abi.encode("COSMOS", "localterra", "terra"))
    TERRA_CHAIN_KEY=$(cast keccak "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'localterra' 'terra')")
    log_info "Terra Chain Key: $TERRA_CHAIN_KEY"
    
    # Check if ChainRegistry is set (optional - might be combined with bridge)
    if [ -n "$EVM_CHAIN_REGISTRY" ]; then
        log_info "Registering Terra chain in ChainRegistry..."
        cast send "$EVM_CHAIN_REGISTRY" \
            "registerChain(bytes32,uint8,string)" \
            "$TERRA_CHAIN_KEY" \
            2 \
            "Terra Classic Local" \
            --rpc-url "$EVM_RPC_URL" \
            --private-key "$EVM_PRIVATE_KEY" \
            || log_warn "Chain registration failed (may already exist)"
    else
        log_info "Skipping ChainRegistry (not deployed separately)"
    fi
    
    log_info "EVM side configured"
}

# Register EVM chain on Terra bridge
setup_terra_side() {
    log_info "=== Configuring Terra Side ==="
    
    # Add Anvil (chain ID 31337) as supported chain
    log_info "Adding EVM chain to Terra bridge..."
    
    ADD_CHAIN_MSG="{\"add_chain\":{\"chain_id\":31337,\"name\":\"Anvil Local\",\"bridge_address\":\"$EVM_BRIDGE_ADDRESS\"}}"
    
    TX=$(terrad_exec tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$ADD_CHAIN_MSG" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1) || log_warn "Chain registration failed (may already exist or unsupported)"
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -n "$TX_HASH" ] && [ "$TX_HASH" != "null" ]; then
        log_info "Add chain TX: $TX_HASH"
        sleep 6
    fi
    
    # Add uluna as supported token
    log_info "Adding LUNC token..."
    ADD_TOKEN_MSG="{\"add_token\":{\"token\":\"uluna\",\"is_native\":true,\"evm_token_address\":\"0x0000000000000000000000000000000000001234\",\"terra_decimals\":6,\"evm_decimals\":18}}"
    
    TX=$(terrad_exec tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$ADD_TOKEN_MSG" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1) || log_warn "Token registration failed (may already exist or unsupported)"
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -n "$TX_HASH" ] && [ "$TX_HASH" != "null" ]; then
        log_info "Add LUNC TX: $TX_HASH"
        sleep 6
    fi
    
    # Add uusd (USTC) as supported token
    log_info "Adding USTC token..."
    ADD_USD_MSG="{\"add_token\":{\"token\":\"uusd\",\"is_native\":true,\"evm_token_address\":\"0x0000000000000000000000000000000000005678\",\"terra_decimals\":6,\"evm_decimals\":18}}"
    
    TX=$(terrad_exec tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$ADD_USD_MSG" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1) || log_warn "Token registration failed (may already exist or unsupported)"
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -n "$TX_HASH" ] && [ "$TX_HASH" != "null" ]; then
        log_info "Add USTC TX: $TX_HASH"
        sleep 6
    fi
    
    log_info "Terra side configured"
}

# Add operator permissions
setup_operator() {
    log_info "=== Configuring Operator ==="
    
    # The test1 key is already the operator from instantiation
    OPERATOR_TERRA="terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"
    OPERATOR_EVM="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"  # Anvil account 0
    
    log_info "Operator Terra address: $OPERATOR_TERRA"
    log_info "Operator EVM address: $OPERATOR_EVM"
    
    # Try to add operator if there's an add_operator message
    ADD_OP_MSG="{\"add_operator\":{\"operator\":\"$OPERATOR_TERRA\"}}"
    
    TX=$(terrad_exec tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$ADD_OP_MSG" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1) || log_warn "Operator add failed (may already exist or not needed)"
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -n "$TX_HASH" ] && [ "$TX_HASH" != "null" ]; then
        log_info "Add operator TX: $TX_HASH"
        sleep 6
    fi
    
    log_info "Operator configured"
}

# Fund the admin/operator wallet with SOL via the validator's built-in airdrop.
# The cl8y_faucet program is for test SPL tokens only — SOL comes from here.
fund_solana_wallets() {
    if ! command -v solana >/dev/null 2>&1; then
        log_warn "solana CLI not found — skipping SOL funding"
        return 0
    fi

    local wallet_pubkey
    wallet_pubkey=$(solana-keygen pubkey "${SOLANA_KEYPAIR}" 2>/dev/null || true)
    if [ -z "$wallet_pubkey" ]; then
        log_warn "Could not derive pubkey from ${SOLANA_KEYPAIR} — skipping SOL funding"
        return 0
    fi

    log_info "Funding admin wallet ${wallet_pubkey} with SOL..."
    solana airdrop 100 "$wallet_pubkey" --url "$SOLANA_RPC_URL" 2>/dev/null \
        || log_warn "SOL airdrop failed (wallet may already be funded)"

    local balance
    balance=$(solana balance "$wallet_pubkey" --url "$SOLANA_RPC_URL" 2>/dev/null || echo "unknown")
    log_info "Admin wallet balance: ${balance}"
}

# Setup Solana side (initialize bridge, register chains, fund wallets)
setup_solana_side() {
    if [ -z "$SOLANA_PROGRAM_ID" ]; then
        log_warn "SOLANA_PROGRAM_ID not set — skipping Solana bridge configuration"
        return 0
    fi

    log_info "=== Configuring Solana Side ==="
    log_info "Solana Program ID: $SOLANA_PROGRAM_ID"

    # Check Solana validator is reachable
    if ! curl -sf -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","id":1,"method":"getVersion"}' \
        "$SOLANA_RPC_URL" &>/dev/null; then
        log_warn "Solana validator not reachable at $SOLANA_RPC_URL — skipping"
        return 0
    fi

    # Step 1: Fund admin wallet with SOL (needed for init + registration txs)
    fund_solana_wallets

    # Step 2: Initialize bridge (idempotent — skips if PDA already exists)
    log_info "Initializing Solana bridge..."
    if [ -x "$REPO_ROOT/scripts/solana/initialize-bridge.sh" ]; then
        SOLANA_PROGRAM_ID="$SOLANA_PROGRAM_ID" \
        SOLANA_RPC_URL="$SOLANA_RPC_URL" \
        SOLANA_KEYPAIR="$SOLANA_KEYPAIR" \
        SOLANA_OPERATOR_KEYPAIR="${SOLANA_KEYPAIR}" \
        OPERATOR_PUBKEY="$(solana-keygen pubkey "${SOLANA_KEYPAIR}" 2>/dev/null || echo "")" \
            "$REPO_ROOT/scripts/solana/initialize-bridge.sh" \
            || log_warn "Solana bridge initialization failed (may already be initialized)"
    elif [ -x "$(command -v npx)" ] && [ -d "$REPO_ROOT/packages/contracts-solana" ]; then
        cd "$REPO_ROOT/packages/contracts-solana"
        ANCHOR_PROVIDER_URL="${SOLANA_RPC_URL}" \
        ANCHOR_WALLET="${SOLANA_KEYPAIR}" \
        SOLANA_OPERATOR_KEYPAIR="${SOLANA_KEYPAIR}" \
            npx ts-mocha -p ./tsconfig.json -t 60000 tests/bridge.test.ts --grep "initialize" 2>/dev/null \
            || log_warn "Solana bridge initialization via test runner failed"
        cd "$REPO_ROOT"
    fi

    # Step 3: Register EVM chain on Solana bridge (chain ID 0x00000001)
    log_info "Registering EVM chain on Solana bridge..."
    if [ -x "$(command -v npx)" ] && [ -d "$REPO_ROOT/packages/contracts-solana" ]; then
        cd "$REPO_ROOT/packages/contracts-solana"
        ANCHOR_PROVIDER_URL="${SOLANA_RPC_URL}" \
        ANCHOR_WALLET="${SOLANA_KEYPAIR}" \
        SOLANA_OPERATOR_KEYPAIR="${SOLANA_KEYPAIR}" \
            npx ts-mocha -p ./tsconfig.json -t 30000 tests/bridge.test.ts --grep "registers a chain" 2>/dev/null \
            || log_warn "Solana chain registration via test runner failed (may need manual setup)"
        cd "$REPO_ROOT"
    else
        log_warn "npx or contracts-solana not available — run Solana registration manually"
    fi

    # Step 4: Register Solana chain on EVM side
    if [ -n "$EVM_CHAIN_REGISTRY" ]; then
        log_info "Registering Solana chain on EVM ChainRegistry..."
        SOLANA_CHAIN_ID="0x00000005"
        cast send "$EVM_CHAIN_REGISTRY" \
            "registerChain(string,bytes4)" \
            "solana_localnet" \
            "$SOLANA_CHAIN_ID" \
            --rpc-url "$EVM_RPC_URL" \
            --private-key "$EVM_PRIVATE_KEY" \
            2>/dev/null || log_warn "Solana chain registration on EVM failed (may already exist)"
    fi

    log_info "Solana side configured"
}

# Verify configuration
verify_config() {
    log_info "=== Verifying Configuration ==="
    
    # Query Terra bridge config
    CONFIG_QUERY='{"config":{}}'
    CONFIG_B64=$(echo -n "$CONFIG_QUERY" | base64 -w0)
    CONFIG=$(curl -sf "${TERRA_LCD}/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE_ADDRESS}/smart/${CONFIG_B64}" 2>/dev/null | jq '.data' 2>/dev/null)
    
    if [ -n "$CONFIG" ] && [ "$CONFIG" != "null" ]; then
        log_info "Terra bridge config: $CONFIG"
    else
        log_warn "Could not query Terra bridge config"
    fi
    
    # Query EVM bridge withdraw delay
    DELAY=$(cast call "$EVM_BRIDGE_ADDRESS" "withdrawDelay()" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "N/A")
    log_info "EVM withdraw delay: $DELAY seconds"
}

# Main
main() {
    log_info "=== CL8Y Bridge Configuration ==="
    
    check_addresses
    setup_evm_side
    setup_terra_side
    setup_solana_side
    setup_operator
    verify_config
    
    echo ""
    log_info "=== Bridge Configuration Complete ==="
    echo ""
    echo "Configuration:"
    echo "  EVM Bridge:    $EVM_BRIDGE_ADDRESS"
    echo "  Terra Bridge:  $TERRA_BRIDGE_ADDRESS"
    echo "  Solana Program: ${SOLANA_PROGRAM_ID:-(not set)}"
    echo ""
    log_info "Next steps:"
    echo "  1. Update packages/operator/.env with bridge addresses"
    echo "  2. Run: make operator"
    echo "  3. Run: make test-transfer"
}

main "$@"
