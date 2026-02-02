#!/bin/bash
# Common helper functions for E2E tests

# Colors
export GREEN='\033[0;32m'
export YELLOW='\033[1;33m'
export RED='\033[0;31m'
export BLUE='\033[0;34m'
export NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_pass() { echo -e "${GREEN}[PASS]${NC} $1"; }
log_fail() { echo -e "${RED}[FAIL]${NC} $1"; }

# Wait for a condition with timeout
wait_for() {
    local description="$1"
    local check_cmd="$2"
    local timeout="${3:-120}"
    local interval="${4:-2}"
    local elapsed=0
    
    log_info "Waiting for $description (timeout: ${timeout}s)..."
    
    while [ $elapsed -lt $timeout ]; do
        if eval "$check_cmd" &>/dev/null; then
            log_info "$description ready"
            return 0
        fi
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_error "$description did not become ready within ${timeout}s"
    return 1
}

# Skip time on Anvil using evm_increaseTime
skip_anvil_time() {
    local seconds="$1"
    log_info "Skipping $seconds seconds on Anvil..."
    
    cast rpc evm_increaseTime "$seconds" --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1
    cast rpc evm_mine --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1
    
    log_info "Time skipped successfully"
}

# Get EVM block timestamp
get_evm_timestamp() {
    cast block --rpc-url "$EVM_RPC_URL" -f timestamp 2>/dev/null || echo "0"
}

# Get EVM block number
get_evm_block() {
    cast block-number --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0"
}

# Get Terra block height
get_terra_height() {
    curl -s "$TERRA_RPC_URL/status" 2>/dev/null | jq -r '.result.sync_info.latest_block_height' 2>/dev/null || echo "0"
}

# Query Terra contract
query_terra_contract() {
    local contract="$1"
    local query="$2"
    
    local query_b64=$(echo -n "$query" | base64 -w0)
    curl -s "${TERRA_LCD_URL}/cosmwasm/wasm/v1/contract/${contract}/smart/${query_b64}" 2>/dev/null
}

# Execute Terra contract (via terrad CLI in docker)
execute_terra_contract() {
    local contract="$1"
    local msg="$2"
    local funds="${3:-}"
    
    local funds_arg=""
    if [ -n "$funds" ]; then
        funds_arg="--amount $funds"
    fi
    
    docker compose exec -T terrad-cli terrad tx wasm execute "$contract" "$msg" \
        --from test1 \
        --chain-id localterra \
        --node http://localterra:26657 \
        --gas auto \
        --gas-adjustment 1.5 \
        --broadcast-mode sync \
        $funds_arg \
        -y 2>&1
}

# Get ERC20 balance
get_erc20_balance() {
    local token="$1"
    local account="$2"
    
    cast call "$token" "balanceOf(address)(uint256)" "$account" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0"
}

# Get native balance on EVM
get_evm_balance() {
    local account="$1"
    cast balance "$account" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0"
}

# Get Terra native balance
get_terra_balance() {
    local address="$1"
    local denom="${2:-uluna}"
    
    curl -s "${TERRA_LCD_URL}/cosmos/bank/v1beta1/balances/${address}" 2>/dev/null | \
        jq -r ".balances[] | select(.denom == \"$denom\") | .amount" 2>/dev/null || echo "0"
}

# Parse transaction result
parse_tx_result() {
    local result="$1"
    
    echo "$result" | jq -r '.txhash // .tx_response.txhash // "unknown"' 2>/dev/null || echo "unknown"
}

# Check if transaction succeeded
tx_succeeded() {
    local result="$1"
    
    local code=$(echo "$result" | jq -r '.code // .tx_response.code // 0' 2>/dev/null)
    [ "$code" = "0" ] || [ -z "$code" ]
}
