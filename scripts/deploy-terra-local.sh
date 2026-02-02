#!/bin/bash
# Deploy Terra Bridge to LocalTerra
#
# Prerequisites:
# - LocalTerra running (docker compose up localterra)
# - terrad CLI installed
# - Contract WASM built
#
# Usage:
#   ./scripts/deploy-terra-local.sh

set -e

# Configuration
CHAIN_ID="localterra"
NODE="http://localhost:26657"
LCD="http://localhost:1317"
KEY_NAME="${TERRA_KEY_NAME:-test1}"
WASM_PATH="packages/contracts-terraclassic/artifacts/bridge.wasm"

# LocalTerra default test mnemonic
TEST_MNEMONIC="notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check prerequisites
check_prereqs() {
    log_info "Checking prerequisites..."
    
    # Check terrad
    if ! command -v terrad &> /dev/null; then
        log_error "terrad not found. Install Terra Classic CLI first."
        exit 1
    fi
    
    # Check LocalTerra is running
    if ! curl -s "$NODE/status" > /dev/null 2>&1; then
        log_error "LocalTerra not running at $NODE"
        log_info "Start with: docker compose up -d localterra"
        exit 1
    fi
    
    # Check WASM exists
    if [ ! -f "$WASM_PATH" ]; then
        log_error "WASM not found at $WASM_PATH"
        log_info "Build with: cd packages/contracts-terraclassic && cargo build --release --target wasm32-unknown-unknown"
        exit 1
    fi
    
    log_info "Prerequisites OK"
}

# Setup test key
setup_key() {
    log_info "Setting up test key..."
    
    # Check if key exists
    if terrad keys show "$KEY_NAME" > /dev/null 2>&1; then
        log_info "Key '$KEY_NAME' already exists"
    else
        log_info "Importing test key..."
        echo "$TEST_MNEMONIC" | terrad keys add "$KEY_NAME" --recover --index 0
    fi
    
    ADMIN=$(terrad keys show "$KEY_NAME" -a)
    log_info "Using address: $ADMIN"
}

# Store contract
store_contract() {
    log_info "Storing contract..."
    
    TX=$(terrad tx wasm store "$WASM_PATH" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 50000uluna \
        -y -o json)
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash')
    log_info "Store TX: $TX_HASH"
    
    log_info "Waiting for confirmation..."
    sleep 6
    
    # Get code ID
    CODE_ID=$(terrad query wasm list-code --node "$NODE" -o json | jq -r '.code_infos[-1].code_id')
    log_info "Code ID: $CODE_ID"
}

# Instantiate contract
instantiate_contract() {
    log_info "Instantiating contract..."
    
    INIT_MSG=$(cat << EOF
{
    "admin": "$ADMIN",
    "relayers": ["$ADMIN"],
    "min_signatures": 1,
    "min_bridge_amount": "1000000",
    "max_bridge_amount": "1000000000000000",
    "fee_bps": 30,
    "fee_collector": "$ADMIN"
}
EOF
)
    
    TX=$(terrad tx wasm instantiate "$CODE_ID" "$INIT_MSG" \
        --label "cl8y-bridge-local" \
        --admin "$ADMIN" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 50000uluna \
        -y -o json)
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash')
    log_info "Instantiate TX: $TX_HASH"
    
    log_info "Waiting for confirmation..."
    sleep 6
    
    # Get contract address
    CONTRACT=$(terrad query wasm list-contract-by-code "$CODE_ID" --node "$NODE" -o json | jq -r '.contracts[-1]')
    log_info "Contract Address: $CONTRACT"
}

# Main
main() {
    log_info "=== CL8Y Bridge LocalTerra Deployment ==="
    
    check_prereqs
    setup_key
    store_contract
    instantiate_contract
    
    echo ""
    log_info "=== Deployment Complete ==="
    echo ""
    echo "========================================"
    echo "TERRA_BRIDGE_ADDRESS=$CONTRACT"
    echo "========================================"
    echo ""
    log_info "Add this to packages/operator/.env:"
    echo "  TERRA_BRIDGE_ADDRESS=$CONTRACT"
    echo ""
    log_info "Next steps:"
    echo "  1. Run: ./scripts/setup-bridge.sh"
    echo "  2. Run: make relayer"
    echo "  3. Run: make test-transfer"
}

main "$@"
