#!/bin/bash
# Deploy CL8Y Bridge to Terra Classic Mainnet (columbus-5)
#
# ⚠️  WARNING: This script deploys to MAINNET. Use with caution!
#
# Prerequisites:
# - terrad CLI installed
# - Account with mainnet LUNA for gas
# - Contract WASM built and audited
# - Multi-sig setup for admin
#
# Usage:
#   ./scripts/deploy-terra-mainnet.sh
#
# Environment variables:
#   TERRA_ADMIN - Admin address (should be multi-sig)
#   TERRA_OPERATORS - Comma-separated operator addresses
#   TERRA_FEE_COLLECTOR - Fee collector address

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Terra Classic Mainnet configuration
NODE="${TERRA_MAINNET_NODE:-https://terra-classic-rpc.publicnode.com:443}"
CHAIN_ID="${TERRA_MAINNET_CHAIN_ID:-columbus-5}"
KEY_NAME="${TERRA_KEY_NAME:-deployer}"

# Contract paths
CONTRACT_DIR="$PROJECT_ROOT/packages/contracts-terraclassic/bridge"
WASM_FILE="$CONTRACT_DIR/artifacts/bridge.wasm"

# Production configuration
WITHDRAW_DELAY=300  # 5 minutes (production default)
MIN_SIGNATURES=1
MIN_BRIDGE_AMOUNT="1000000"        # 1 LUNA
MAX_BRIDGE_AMOUNT="1000000000000"  # 1M LUNA
FEE_BPS=30                         # 0.3%

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Safety check
confirm_mainnet() {
    echo ""
    echo -e "${RED}╔══════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ⚠️  MAINNET DEPLOYMENT - REAL FUNDS AT RISK    ║${NC}"
    echo -e "${RED}╚══════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Chain: columbus-5 (Terra Classic Mainnet)"
    echo "Node:  $NODE"
    echo ""

    read -p "Type 'DEPLOY_MAINNET' to confirm: " CONFIRM
    if [ "$CONFIRM" != "DEPLOY_MAINNET" ]; then
        log_error "Deployment cancelled"
        exit 1
    fi
}

# Check prerequisites
check_prereqs() {
    log_info "Checking prerequisites..."

    if ! command -v terrad &> /dev/null; then
        log_error "terrad CLI not found. Please install it first."
        exit 1
    fi

    # Check required environment variables
    if [ -z "$TERRA_ADMIN" ]; then
        log_error "TERRA_ADMIN environment variable required (multi-sig recommended)"
        exit 1
    fi

    if [ -z "$TERRA_OPERATORS" ]; then
        log_error "TERRA_OPERATORS environment variable required"
        exit 1
    fi

    if [ -z "$TERRA_FEE_COLLECTOR" ]; then
        log_error "TERRA_FEE_COLLECTOR environment variable required"
        exit 1
    fi

    if ! terrad keys show "$KEY_NAME" --keyring-backend os &> /dev/null; then
        log_error "Key '$KEY_NAME' not found in OS keyring"
        exit 1
    fi

    if [ ! -f "$WASM_FILE" ]; then
        log_error "Contract WASM not found at: $WASM_FILE"
        log_error "Build with: cd $CONTRACT_DIR && cargo build --release --target wasm32-unknown-unknown"
        exit 1
    fi

    log_info "Prerequisites OK"
}

# Get deployer address
get_deployer() {
    DEPLOYER=$(terrad keys show "$KEY_NAME" -a --keyring-backend os)
    log_info "Deployer: $DEPLOYER"
    log_info "Admin: $TERRA_ADMIN"
}

# Store contract code
store_contract() {
    log_info "Storing contract on mainnet..."

    # Higher gas for mainnet
    TX=$(terrad tx wasm store "$WASM_FILE" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$NODE" \
        --gas 5000000 \
        --fees 2000000uluna \
        --keyring-backend os \
        -y -o json 2>&1)

    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" == "null" ]; then
        log_error "Failed to store contract: $TX"
        exit 1
    fi

    log_info "Store TX: $TX_HASH"
    log_info "Waiting for confirmation (this may take 30+ seconds)..."
    sleep 30

    CODE_ID=$(terrad query tx "$TX_HASH" --node "$NODE" -o json 2>/dev/null | \
        jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value' || echo "")

    if [ -z "$CODE_ID" ]; then
        log_error "Failed to get code_id from transaction"
        log_info "Check transaction manually: terrad query tx $TX_HASH --node $NODE"
        exit 1
    fi

    log_info "Code ID: $CODE_ID"
}

# Instantiate contract
instantiate_contract() {
    log_info "Instantiating contract..."

    # Parse operators list
    IFS=',' read -ra OPERATORS_ARRAY <<< "$TERRA_OPERATORS"
    OPERATORS_JSON=$(printf '%s\n' "${OPERATORS_ARRAY[@]}" | jq -R . | jq -s .)

    INIT_MSG=$(cat << EOF
{
    "admin": "$TERRA_ADMIN",
    "operators": $OPERATORS_JSON,
    "min_signatures": $MIN_SIGNATURES,
    "min_bridge_amount": "$MIN_BRIDGE_AMOUNT",
    "max_bridge_amount": "$MAX_BRIDGE_AMOUNT",
    "fee_bps": $FEE_BPS,
    "fee_collector": "$TERRA_FEE_COLLECTOR"
}
EOF
)

    log_info "Instantiate message:"
    echo "$INIT_MSG" | jq .

    TX=$(terrad tx wasm instantiate "$CODE_ID" "$INIT_MSG" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --node "$NODE" \
        --label "cl8y-bridge-mainnet-v1" \
        --admin "$TERRA_ADMIN" \
        --gas 1000000 \
        --fees 500000uluna \
        --keyring-backend os \
        -y -o json 2>&1)

    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" == "null" ]; then
        log_error "Failed to instantiate contract: $TX"
        exit 1
    fi

    log_info "Instantiate TX: $TX_HASH"
    log_info "Waiting for confirmation..."
    sleep 30

    CONTRACT=$(terrad query wasm list-contract-by-code "$CODE_ID" --node "$NODE" -o json | jq -r '.contracts[-1]')
    log_info "Contract Address: $CONTRACT"
}

# Main
main() {
    confirm_mainnet

    log_info "=== CL8Y Bridge Terra Mainnet Deployment ==="
    log_info "Chain ID: $CHAIN_ID"
    log_info "Node: $NODE"

    check_prereqs
    get_deployer
    store_contract
    instantiate_contract

    echo ""
    log_info "=== Deployment Complete ==="
    echo ""
    echo "╔════════════════════════════════════════════════════╗"
    echo "║  MAINNET CONTRACT DEPLOYED                         ║"
    echo "╚════════════════════════════════════════════════════╝"
    echo ""
    echo "TERRA_MAINNET_BRIDGE_ADDRESS=$CONTRACT"
    echo "CODE_ID=$CODE_ID"
    echo ""
    log_info "Post-deployment checklist:"
    echo "  1. ✓ Contract instantiated"
    echo "  2. □ Add EVM chain configuration (addChain)"
    echo "  3. □ Add supported tokens (addToken)"
    echo "  4. □ Set rate limits (setRateLimit)"
    echo "  5. □ Register canceler addresses (addCanceler)"
    echo "  6. □ Verify contract on explorer"
    echo "  7. □ Update environment variables"
    echo ""
    log_warn "Withdraw delay is set to $WITHDRAW_DELAY seconds (5 minutes)"
}

main "$@"
