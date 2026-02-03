#!/bin/bash
# Deploy Terra Bridge to LocalTerra
#
# Prerequisites:
# - LocalTerra running (docker compose up localterra)
# - Contract WASM built
#
# Usage:
#   ./scripts/deploy-terra-local.sh
#   ./scripts/deploy-terra-local.sh --cw20  # Also deploy cw20-mintable

set -e

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration
CHAIN_ID="localterra"
NODE="http://localhost:26657"
LCD="http://localhost:1317"
KEY_NAME="${TERRA_KEY_NAME:-test1}"
WASM_PATH="$PROJECT_ROOT/packages/contracts-terraclassic/artifacts/bridge.wasm"
CW20_WASM_PATH="$PROJECT_ROOT/packages/contracts-terraclassic/artifacts/cw20_mintable.wasm"
CONTAINER_NAME="${LOCALTERRA_CONTAINER:-cl8y-bridge-monorepo-localterra-1}"

# LocalTerra default test address (pre-funded in genesis)
TEST_ADDRESS="terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"

# Parse arguments
DEPLOY_CW20=false
for arg in "$@"; do
    case $arg in
        --cw20) DEPLOY_CW20=true ;;
    esac
done

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Run terrad TX command via docker exec (needs keyring-backend)
terrad_tx() {
    docker exec "$CONTAINER_NAME" terrad "$@" --keyring-backend test
}

# Run terrad query command via docker exec (no keyring needed)
terrad_query() {
    docker exec "$CONTAINER_NAME" terrad "$@"
}

# Setup test key in container (imports if not present)
setup_key() {
    log_info "Setting up test key in container..."
    
    # Check if key exists
    if docker exec "$CONTAINER_NAME" terrad keys show "$KEY_NAME" --keyring-backend test > /dev/null 2>&1; then
        log_info "Key '$KEY_NAME' already exists in container"
    else
        log_info "Importing test key into container..."
        echo "$TEST_MNEMONIC" | docker exec -i "$CONTAINER_NAME" terrad keys add "$KEY_NAME" --recover --keyring-backend test
    fi
}

# LocalTerra default test mnemonic
TEST_MNEMONIC="notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius"

# Check prerequisites
check_prereqs() {
    log_info "Checking prerequisites..."
    
    # Check docker is available
    if ! command -v docker &> /dev/null; then
        log_error "docker not found. Install Docker first."
        exit 1
    fi
    
    # Check LocalTerra container is running
    if ! docker ps --format '{{.Names}}' | grep -q "$CONTAINER_NAME"; then
        log_error "LocalTerra container not running: $CONTAINER_NAME"
        log_info "Start with: docker compose up -d localterra"
        exit 1
    fi
    
    # Check LocalTerra is producing blocks (use LCD since RPC may not be exposed)
    if ! curl -s "$LCD/cosmos/base/tendermint/v1beta1/blocks/latest" > /dev/null 2>&1; then
        log_error "LocalTerra not responding at $LCD"
        exit 1
    fi
    
    # Check WASM exists
    if [ ! -f "$WASM_PATH" ]; then
        log_error "WASM not found at $WASM_PATH"
        log_info "Build with: cd packages/contracts-terraclassic && cargo build --release --target wasm32-unknown-unknown --lib"
        exit 1
    fi
    
    log_info "Prerequisites OK"
}

# Copy WASM file into container
copy_wasm_to_container() {
    log_info "Copying WASM to container..."
    
    # Create temp directory in container
    docker exec "$CONTAINER_NAME" mkdir -p /tmp/wasm
    
    # Copy bridge WASM
    docker cp "$WASM_PATH" "$CONTAINER_NAME:/tmp/wasm/bridge.wasm"
    log_info "Copied bridge.wasm to container"
    
    # Copy cw20-mintable if requested
    if [ "$DEPLOY_CW20" = true ] && [ -f "$CW20_WASM_PATH" ]; then
        docker cp "$CW20_WASM_PATH" "$CONTAINER_NAME:/tmp/wasm/cw20_mintable.wasm"
        log_info "Copied cw20_mintable.wasm to container"
    fi
}

# Store bridge contract
store_bridge_contract() {
    log_info "Storing bridge contract..."
    
    # Store contract (test1 key is already in the keyring from genesis)
    TX=$(terrad_tx tx wasm store /tmp/wasm/bridge.wasm \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 200000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1)
    
    # Extract txhash - handle both sync response and error formats
    TX_HASH=$(echo "$TX" | grep -o '"txhash":"[^"]*"' | cut -d'"' -f4 || echo "")
    TX_CODE=$(echo "$TX" | jq -r '.code // 0' 2>/dev/null || echo "0")
    
    if [ -z "$TX_HASH" ]; then
        log_error "Failed to store contract: $TX"
        exit 1
    fi
    
    log_info "Store TX: $TX_HASH"
    
    # Check if tx was rejected immediately (code != 0 means error)
    if [ "$TX_CODE" != "0" ]; then
        RAW_LOG=$(echo "$TX" | jq -r '.raw_log // "Unknown error"' 2>/dev/null)
        log_error "Transaction rejected: $RAW_LOG"
        exit 1
    fi
    
    log_info "Waiting for confirmation..."
    sleep 10
    
    # Get code ID from list-code
    CODE_ID=$(terrad_query query wasm list-code -o json | jq -r '.code_infos[-1].code_id')
    
    if [ -z "$CODE_ID" ] || [ "$CODE_ID" = "null" ]; then
        log_error "Failed to get code ID"
        exit 1
    fi
    
    log_info "Bridge Code ID: $CODE_ID"
    BRIDGE_CODE_ID=$CODE_ID
}

# Instantiate bridge contract
instantiate_bridge_contract() {
    log_info "Instantiating bridge contract..."
    
    # Build init message
    INIT_MSG=$(cat << EOF
{
    "admin": "$TEST_ADDRESS",
    "operators": ["$TEST_ADDRESS"],
    "min_signatures": 1,
    "min_bridge_amount": "1000000",
    "max_bridge_amount": "1000000000000000",
    "fee_bps": 30,
    "fee_collector": "$TEST_ADDRESS"
}
EOF
)
    
    TX=$(terrad_tx tx wasm instantiate "$BRIDGE_CODE_ID" "$INIT_MSG" \
        --label "cl8y-bridge-local" \
        --admin "$TEST_ADDRESS" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 50000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1)
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" = "null" ]; then
        log_error "Failed to instantiate contract: $TX"
        exit 1
    fi
    
    log_info "Instantiate TX: $TX_HASH"
    
    log_info "Waiting for confirmation..."
    sleep 8
    
    # Get contract address
    BRIDGE_CONTRACT=$(terrad_query query wasm list-contract-by-code "$BRIDGE_CODE_ID" -o json | jq -r '.contracts[-1]')
    
    if [ -z "$BRIDGE_CONTRACT" ] || [ "$BRIDGE_CONTRACT" = "null" ]; then
        log_error "Failed to get contract address"
        exit 1
    fi
    
    log_info "Bridge Contract Address: $BRIDGE_CONTRACT"
}

# Store cw20-mintable contract
store_cw20_contract() {
    if [ "$DEPLOY_CW20" != true ]; then
        return
    fi
    
    log_info "Storing cw20-mintable contract..."
    
    TX=$(terrad_tx tx wasm store /tmp/wasm/cw20_mintable.wasm \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 200000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1)
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" = "null" ]; then
        log_error "Failed to store cw20-mintable: $TX"
        exit 1
    fi
    
    log_info "Store TX: $TX_HASH"
    
    log_info "Waiting for confirmation..."
    sleep 8
    
    # Get code ID
    CW20_CODE_ID=$(terrad_query query wasm list-code -o json | jq -r '.code_infos[-1].code_id')
    log_info "CW20-Mintable Code ID: $CW20_CODE_ID"
}

# Instantiate a test CW20 token (optional)
instantiate_test_cw20() {
    if [ "$DEPLOY_CW20" != true ]; then
        return
    fi
    
    log_info "Instantiating test CW20 token..."
    
    # Build init message for cw20-mintable
    CW20_INIT_MSG=$(cat << EOF
{
    "name": "Test Bridge Token",
    "symbol": "TBT",
    "decimals": 6,
    "initial_balances": [
        {
            "address": "$TEST_ADDRESS",
            "amount": "1000000000000"
        }
    ],
    "mint": {
        "minter": "$TEST_ADDRESS",
        "cap": null
    },
    "marketing": null
}
EOF
)
    
    TX=$(terrad_tx tx wasm instantiate "$CW20_CODE_ID" "$CW20_INIT_MSG" \
        --label "test-bridge-token" \
        --admin "$TEST_ADDRESS" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 50000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1)
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" = "null" ]; then
        log_warn "Failed to instantiate CW20: $TX"
        return
    fi
    
    log_info "CW20 Instantiate TX: $TX_HASH"
    
    sleep 8
    
    # Get CW20 contract address
    CW20_CONTRACT=$(terrad_query query wasm list-contract-by-code "$CW20_CODE_ID" -o json | jq -r '.contracts[-1]')
    log_info "CW20 Contract Address: $CW20_CONTRACT"
}

# Configure bridge for local testing
configure_local() {
    log_info "Configuring bridge for local testing..."
    
    # Set withdraw delay to 60 seconds (contract minimum)
    SET_DELAY_MSG='{"set_withdraw_delay":{"delay_seconds":60}}'
    
    TX=$(terrad_tx tx wasm execute "$BRIDGE_CONTRACT" "$SET_DELAY_MSG" \
        --from "$KEY_NAME" \
        --chain-id "$CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y -o json 2>&1)
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    if [ -n "$TX_HASH" ] && [ "$TX_HASH" != "null" ]; then
        log_info "Set withdraw delay TX: $TX_HASH"
        sleep 6
    else
        log_warn "Could not set withdraw delay (may already be default or unsupported)"
    fi
    
    log_info "Local configuration complete"
}

# Verify deployment
verify_deployment() {
    log_info "Verifying deployment..."
    
    # Query config
    CONFIG_QUERY='{"config":{}}'
    CONFIG_B64=$(echo -n "$CONFIG_QUERY" | base64 -w0)
    CONFIG=$(curl -sf "${LCD}/cosmwasm/wasm/v1/contract/${BRIDGE_CONTRACT}/smart/${CONFIG_B64}" 2>/dev/null | jq '.data' 2>/dev/null)
    
    if [ -n "$CONFIG" ] && [ "$CONFIG" != "null" ]; then
        log_info "Bridge config: $CONFIG"
    else
        log_warn "Could not query bridge config"
    fi
}

# Main
main() {
    log_info "=== CL8Y Bridge LocalTerra Deployment ==="
    log_info "Using container: $CONTAINER_NAME"
    
    check_prereqs
    setup_key
    copy_wasm_to_container
    store_bridge_contract
    instantiate_bridge_contract
    store_cw20_contract
    instantiate_test_cw20
    configure_local
    verify_deployment
    
    echo ""
    log_info "=== Deployment Complete ==="
    echo ""
    echo "========================================"
    echo "TERRA_BRIDGE_ADDRESS=$BRIDGE_CONTRACT"
    if [ "$DEPLOY_CW20" = true ] && [ -n "$CW20_CONTRACT" ]; then
        echo "TERRA_CW20_ADDRESS=$CW20_CONTRACT"
        echo "TERRA_CW20_CODE_ID=$CW20_CODE_ID"
    fi
    echo "========================================"
    echo ""
    log_info "Add to packages/operator/.env:"
    echo "  TERRA_BRIDGE_ADDRESS=$BRIDGE_CONTRACT"
    echo ""
    log_info "Next steps:"
    echo "  1. Export: export TERRA_BRIDGE_ADDRESS=$BRIDGE_CONTRACT"
    echo "  2. Run: ./scripts/setup-bridge.sh"
    echo "  3. Run: make operator"
    echo "  4. Run: make test-transfer"
}

main "$@"
