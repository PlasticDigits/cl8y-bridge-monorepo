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
#   EVM_BRIDGE_ADDRESS=0x... TERRA_BRIDGE_ADDRESS=terra1... ./scripts/setup-bridge.sh

set -e

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
        log_error "EVM_BRIDGE_ADDRESS not set"
        log_info "Deploy EVM contracts first: make deploy-evm"
        exit 1
    fi
    
    if [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_error "TERRA_BRIDGE_ADDRESS not set"
        log_info "Deploy Terra contracts first: ./scripts/deploy-terra-local.sh"
        exit 1
    fi
    
    # Check LocalTerra container is running
    if ! docker ps --format '{{.Names}}' | grep -q "$CONTAINER_NAME"; then
        log_error "LocalTerra container not running: $CONTAINER_NAME"
        log_info "Start with: docker compose up -d localterra"
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
    setup_operator
    verify_config
    
    echo ""
    log_info "=== Bridge Configuration Complete ==="
    echo ""
    echo "Configuration:"
    echo "  EVM Bridge: $EVM_BRIDGE_ADDRESS"
    echo "  Terra Bridge: $TERRA_BRIDGE_ADDRESS"
    echo ""
    log_info "Next steps:"
    echo "  1. Update packages/operator/.env with bridge addresses"
    echo "  2. Run: make operator"
    echo "  3. Run: make test-transfer"
}

main "$@"
