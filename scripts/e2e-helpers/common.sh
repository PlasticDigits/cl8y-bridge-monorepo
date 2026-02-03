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
    
    # Get balance and strip any formatting (e.g., "1000000000000 [1e12]" -> "1000000000000")
    local raw_balance
    raw_balance=$(cast call "$token" "balanceOf(address)(uint256)" "$account" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0")
    echo "$raw_balance" | awk '{print $1}'
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

# ============================================================================
# Balance Verification Functions
# ============================================================================

# Verify ERC20 balance increased
verify_erc20_balance_increased() {
    local token="$1"
    local account="$2"
    local before="$3"
    local expected_increase="${4:-0}"
    
    local after=$(get_erc20_balance "$token" "$account")
    
    if [ "$after" -gt "$before" ]; then
        local diff=$((after - before))
        log_pass "ERC20 balance increased: $before → $after (+$diff)"
        if [ "$expected_increase" -gt 0 ] && [ "$diff" -ge "$expected_increase" ]; then
            log_pass "Balance increase matches expected: $diff >= $expected_increase"
        fi
        return 0
    else
        log_fail "ERC20 balance did NOT increase: $before → $after"
        return 1
    fi
}

# Verify ERC20 balance decreased
verify_erc20_balance_decreased() {
    local token="$1"
    local account="$2"
    local before="$3"
    local expected_decrease="${4:-0}"
    
    local after=$(get_erc20_balance "$token" "$account")
    
    if [ "$after" -lt "$before" ]; then
        local diff=$((before - after))
        log_pass "ERC20 balance decreased: $before → $after (-$diff)"
        if [ "$expected_decrease" -gt 0 ] && [ "$diff" -ge "$expected_decrease" ]; then
            log_pass "Balance decrease matches expected: $diff >= $expected_decrease"
        fi
        return 0
    else
        log_fail "ERC20 balance did NOT decrease: $before → $after"
        return 1
    fi
}

# Verify Terra balance increased
verify_terra_balance_increased() {
    local address="$1"
    local denom="$2"
    local before="$3"
    local expected_increase="${4:-0}"
    
    local after=$(get_terra_balance "$address" "$denom")
    
    if [ "$after" -gt "$before" ]; then
        local diff=$((after - before))
        log_pass "Terra balance increased: $before → $after (+$diff $denom)"
        if [ "$expected_increase" -gt 0 ] && [ "$diff" -ge "$expected_increase" ]; then
            log_pass "Balance increase matches expected: $diff >= $expected_increase"
        fi
        return 0
    else
        log_fail "Terra balance did NOT increase: $before → $after"
        return 1
    fi
}

# Verify Terra balance decreased
verify_terra_balance_decreased() {
    local address="$1"
    local denom="$2"
    local before="$3"
    local expected_decrease="${4:-0}"
    
    local after=$(get_terra_balance "$address" "$denom")
    
    if [ "$after" -lt "$before" ]; then
        local diff=$((before - after))
        log_pass "Terra balance decreased: $before → $after (-$diff $denom)"
        if [ "$expected_decrease" -gt 0 ] && [ "$diff" -ge "$expected_decrease" ]; then
            log_pass "Balance decrease matches expected: $diff >= $expected_decrease"
        fi
        return 0
    else
        log_fail "Terra balance did NOT decrease: $before → $after"
        return 1
    fi
}

# Get CW20 token balance
get_cw20_balance() {
    local token="$1"
    local address="$2"
    
    local query="{\"balance\":{\"address\":\"$address\"}}"
    local result=$(query_terra_contract "$token" "$query")
    echo "$result" | jq -r '.data.balance // "0"' 2>/dev/null || echo "0"
}

# Verify CW20 balance increased
verify_cw20_balance_increased() {
    local token="$1"
    local address="$2"
    local before="$3"
    local expected_increase="${4:-0}"
    
    local after=$(get_cw20_balance "$token" "$address")
    
    if [ "$after" -gt "$before" ]; then
        local diff=$((after - before))
        log_pass "CW20 balance increased: $before → $after (+$diff)"
        return 0
    else
        log_fail "CW20 balance did NOT increase: $before → $after"
        return 1
    fi
}

# ============================================================================
# Operator Status Functions
# ============================================================================

# Check if operator is processing
check_operator_processing() {
    local log_file="${PROJECT_ROOT:-.}/.operator.log"
    
    if [ -f "$log_file" ]; then
        # Check for recent block processing
        if tail -20 "$log_file" | grep -qE "Processing.*block|Detected deposit|Creating approval"; then
            return 0
        fi
    fi
    return 1
}

# Wait for operator to process a specific deposit
wait_for_operator_deposit_processing() {
    local nonce="$1"
    local timeout="${2:-60}"
    local log_file="${PROJECT_ROOT:-.}/.operator.log"
    
    log_info "Waiting for operator to process deposit nonce $nonce..."
    
    local elapsed=0
    local interval=2
    
    while [ $elapsed -lt $timeout ]; do
        if [ -f "$log_file" ] && grep -q "nonce.*$nonce\|deposit_nonce.*$nonce" "$log_file"; then
            log_pass "Operator detected deposit with nonce $nonce"
            return 0
        fi
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_warn "Operator may not have processed deposit $nonce yet"
    return 1
}

# ============================================================================
# Chain Key Computation
# ============================================================================

# Compute Terra chain key for LocalTerra
get_terra_chain_key() {
    cast keccak256 "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'localterra' 'terra')" 2>/dev/null
}

# Compute EVM chain key for Anvil (chain ID 31337)
get_anvil_chain_key() {
    cast keccak256 "$(cast abi-encode 'f(string,uint256)' 'EVM' '31337')" 2>/dev/null
}

# Encode Terra address as bytes32 (right-padded)
encode_terra_address() {
    local address="$1"
    local hex=$(printf '%s' "$address" | xxd -p | tr -d '\n')
    echo "0x$(printf '%-64s' "$hex" | tr ' ' '0')"
}

# Encode EVM address as bytes32 (left-padded)
encode_evm_address() {
    local address="$1"
    # Remove 0x prefix if present
    address="${address#0x}"
    echo "0x$(printf '%064s' "$address" | tr ' ' '0')"
}
