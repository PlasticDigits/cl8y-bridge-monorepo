#!/bin/bash
# Deploy CL8Y Bridge to Terra Classic Testnet (rebel-2)
#
# Prerequisites:
# - terrad CLI installed
# - Account with testnet LUNA for gas
# - Contract WASM built
#
# Usage:
#   ./scripts/deploy-terra-testnet.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Terra Classic Testnet configuration
NODE="${TERRA_TESTNET_NODE:-https://terra-testnet-rpc.polkachu.com:443}"
CHAIN_ID="${TERRA_TESTNET_CHAIN_ID:-rebel-2}"
KEY_NAME="${TERRA_KEY_NAME:-deployer}"

# Contract paths
CONTRACT_DIR="$PROJECT_ROOT/packages/contracts-terraclassic/bridge"
WASM_FILE="$CONTRACT_DIR/artifacts/bridge.wasm"

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

    if ! command -v terrad &> /dev/null; then
        log_error "terrad CLI not found. Please install it first."
        exit 1
    fi

    if ! terrad keys show "$KEY_NAME" --keyring-backend test &> /dev/null; then
        log_error "Key '$KEY_NAME' not found. Import or create it first:"
        echo "  terrad keys add $KEY_NAME --recover --keyring-backend test"
        exit 1
    fi

    if [ ! -f "$WASM_FILE" ]; then
        log_info "Building contract..."
        cd "$CONTRACT_DIR"
        cargo build --release --target wasm32-unknown-unknown
        mkdir -p artifacts
        cp ../../../target/wasm32-unknown-unknown/release/bridge.wasm artifacts/
    fi

    log_info "Prerequisites OK"
}

# Get deployer address
get_deployer() {
    DEPLOYER=$(terrad keys show "$KEY_NAME" -a --keyring-backend test)
    log_info "Deployer: $DEPLOYER"
}

# Store contract code
store_contract() {
    log_info "Storing contract on testnet..."

    TX=$(terrad tx wasm store "$WASM_FILE" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 500000uluna \
        --keyring-backend test \
        -y -o json 2>&1)

    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" == "null" ]; then
        log_error "Failed to store contract: $TX"
        exit 1
    fi

    log_info "Store TX: $TX_HASH"
    log_info "Waiting for confirmation..."
    sleep 10

    CODE_ID=$(terrad query tx "$TX_HASH" --node "$NODE" -o json 2>/dev/null | \
        jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value' || echo "")

    if [ -z "$CODE_ID" ]; then
        log_error "Failed to get code_id from transaction"
        exit 1
    fi

    log_info "Code ID: $CODE_ID"
}

# Instantiate contract
instantiate_contract() {
    log_info "Instantiating contract..."

    # Testnet uses 60 second delay (minimum allowed)
    INIT_MSG=$(cat << EOF
{
    "admin": "$DEPLOYER",
    "operators": ["$DEPLOYER"],
    "min_signatures": 1,
    "min_bridge_amount": "1000000",
    "max_bridge_amount": "1000000000000000",
    "fee_bps": 30,
    "fee_collector": "$DEPLOYER"
}
EOF
)

    TX=$(terrad tx wasm instantiate "$CODE_ID" "$INIT_MSG" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$NODE" \
        --label "cl8y-bridge-testnet" \
        --admin "$DEPLOYER" \
        --gas auto --gas-adjustment 1.4 \
        --fees 200000uluna \
        --keyring-backend test \
        -y -o json 2>&1)

    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" == "null" ]; then
        log_error "Failed to instantiate contract: $TX"
        exit 1
    fi

    log_info "Instantiate TX: $TX_HASH"
    log_info "Waiting for confirmation..."
    sleep 10

    CONTRACT=$(terrad query wasm list-contract-by-code "$CODE_ID" --node "$NODE" -o json | jq -r '.contracts[-1]')
    log_info "Contract Address: $CONTRACT"
}

# Set testnet withdraw delay (60 seconds)
configure_testnet() {
    log_info "Configuring for testnet..."

    # Set withdraw delay to 60 seconds (minimum allowed)
    SET_DELAY_MSG='{"set_withdraw_delay":{"delay_seconds":60}}'

    TX=$(terrad tx wasm execute "$CONTRACT" "$SET_DELAY_MSG" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 50000uluna \
        --keyring-backend test \
        -y -o json 2>&1)

    log_info "Testnet configuration complete"
}

# Main
main() {
    log_info "=== CL8Y Bridge Terra Testnet Deployment ==="
    log_info "Chain ID: $CHAIN_ID"
    log_info "Node: $NODE"

    check_prereqs
    get_deployer
    store_contract
    instantiate_contract
    configure_testnet

    echo ""
    log_info "=== Deployment Complete ==="
    echo ""
    echo "========================================"
    echo "TERRA_TESTNET_BRIDGE_ADDRESS=$CONTRACT"
    echo "CODE_ID=$CODE_ID"
    echo "========================================"
    echo ""
    log_info "Next steps:"
    echo "  1. Add EVM chain configuration"
    echo "  2. Add supported tokens"
    echo "  3. Register additional operators"
}

main "$@"
