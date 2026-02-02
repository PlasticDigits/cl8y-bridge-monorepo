#!/bin/bash
# Configure cross-chain bridge connections
#
# This script registers each chain with the other bridge contract
# and adds supported tokens.
#
# Prerequisites:
# - Both bridges deployed (EVM and Terra)
# - Environment variables set (or pass as args)
#
# Usage:
#   EVM_BRIDGE_ADDRESS=0x... TERRA_BRIDGE_ADDRESS=terra1... ./scripts/setup-bridge.sh

set -e

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_NODE="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"

# Contract addresses (must be set)
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
EVM_CHAIN_REGISTRY="${EVM_CHAIN_REGISTRY:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"

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

# Validate addresses
check_addresses() {
    if [ -z "$EVM_BRIDGE_ADDRESS" ]; then
        log_error "EVM_BRIDGE_ADDRESS not set"
        log_info "Deploy EVM contracts first: make deploy-evm"
        exit 1
    fi
    
    if [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_error "TERRA_BRIDGE_ADDRESS not set"
        log_info "Deploy Terra contracts first: ./scripts/deploy-terra-local.sh"
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
    terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
        "{\"add_chain\":{\"chain_id\":31337,\"name\":\"Anvil Local\",\"bridge_address\":\"$EVM_BRIDGE_ADDRESS\"}}" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --node "$TERRA_NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 5000uluna \
        -y || log_warn "Chain registration failed (may already exist)"
    
    sleep 3
    
    # Add uluna as supported token
    log_info "Adding LUNC token..."
    terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
        "{\"add_token\":{\"token\":\"uluna\",\"is_native\":true,\"evm_token_address\":\"0x0000000000000000000000000000000000001234\",\"terra_decimals\":6,\"evm_decimals\":18}}" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --node "$TERRA_NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 5000uluna \
        -y || log_warn "Token registration failed (may already exist)"
    
    sleep 3
    
    # Add uusd (USTC) as supported token
    log_info "Adding USTC token..."
    terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
        "{\"add_token\":{\"token\":\"uusd\",\"is_native\":true,\"evm_token_address\":\"0x0000000000000000000000000000000000005678\",\"terra_decimals\":6,\"evm_decimals\":18}}" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --node "$TERRA_NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 5000uluna \
        -y || log_warn "Token registration failed (may already exist)"
    
    log_info "Terra side configured"
}

# Add relayer permissions
setup_relayer() {
    log_info "=== Configuring Relayer ==="
    
    # Get relayer Terra address from key
    RELAYER_TERRA=$(terrad keys show "$TERRA_KEY" -a 2>/dev/null || echo "")
    RELAYER_EVM="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"  # Anvil account 0
    
    if [ -n "$RELAYER_TERRA" ]; then
        log_info "Adding relayer to Terra bridge: $RELAYER_TERRA"
        terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
            "{\"add_relayer\":{\"relayer\":\"$RELAYER_TERRA\"}}" \
            --from "$TERRA_KEY" \
            --chain-id "$TERRA_CHAIN_ID" \
            --node "$TERRA_NODE" \
            --gas auto --gas-adjustment 1.4 \
            --fees 5000uluna \
            -y || log_warn "Relayer add failed (may already exist)"
    fi
    
    log_info "Relayer EVM address: $RELAYER_EVM"
    log_info "Relayer configured"
}

# Main
main() {
    log_info "=== CL8Y Bridge Configuration ==="
    
    check_addresses
    setup_evm_side
    setup_terra_side
    setup_relayer
    
    echo ""
    log_info "=== Bridge Configuration Complete ==="
    echo ""
    echo "Configuration:"
    echo "  EVM Bridge: $EVM_BRIDGE_ADDRESS"
    echo "  Terra Bridge: $TERRA_BRIDGE_ADDRESS"
    echo ""
    log_info "Next steps:"
    echo "  1. Update packages/operator/.env with bridge addresses"
    echo "  2. Run: make relayer"
    echo "  3. Run: make test-transfer"
}

main "$@"
