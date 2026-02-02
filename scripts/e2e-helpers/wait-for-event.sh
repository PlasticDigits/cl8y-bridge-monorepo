#!/bin/bash
# Event waiting helpers for E2E tests
#
# Provides functions to wait for specific blockchain events during transfers

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Wait for an EVM DepositRequest event
# Usage: wait_for_deposit_event <bridge_address> [timeout]
wait_for_deposit_event() {
    local bridge="$1"
    local timeout="${2:-120}"
    local initial_nonce="$3"
    
    log_info "Waiting for DepositRequest event on $bridge..."
    
    # Get initial deposit nonce if not provided
    if [ -z "$initial_nonce" ]; then
        initial_nonce=$(cast call "$bridge" "depositNonce()" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
    fi
    
    local elapsed=0
    local interval=2
    
    while [ $elapsed -lt $timeout ]; do
        current_nonce=$(cast call "$bridge" "depositNonce()" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
        
        if [ "$current_nonce" -gt "$initial_nonce" ]; then
            log_pass "DepositRequest detected! Nonce: $initial_nonce -> $current_nonce"
            return 0
        fi
        
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_error "No DepositRequest event within ${timeout}s"
    return 1
}

# Wait for an EVM WithdrawApproved event
# Usage: wait_for_approval_event <bridge_address> <nonce> [timeout]
wait_for_approval_event() {
    local bridge="$1"
    local nonce="$2"
    local timeout="${3:-120}"
    
    log_info "Waiting for WithdrawApproved event (nonce: $nonce)..."
    
    local elapsed=0
    local interval=2
    
    while [ $elapsed -lt $timeout ]; do
        # Query pending approvals or approved status
        local approval=$(cast call "$bridge" "approvals(uint256)" "$nonce" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "")
        
        if [ -n "$approval" ] && [ "$approval" != "0x0000000000000000000000000000000000000000000000000000000000000000" ]; then
            log_pass "WithdrawApproved detected for nonce $nonce"
            return 0
        fi
        
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_error "No WithdrawApproved event within ${timeout}s"
    return 1
}

# Wait for a Terra lock transaction
# Usage: wait_for_terra_lock <bridge_address> [timeout]
wait_for_terra_lock() {
    local bridge="$1"
    local timeout="${2:-120}"
    local initial_nonce="$3"
    
    log_info "Waiting for Lock on Terra bridge $bridge..."
    
    # Query initial lock nonce
    if [ -z "$initial_nonce" ]; then
        local query='{"lock_nonce":{}}'
        local result=$(query_terra_contract "$bridge" "$query")
        initial_nonce=$(echo "$result" | jq -r '.data.nonce // "0"' 2>/dev/null || echo "0")
    fi
    
    local elapsed=0
    local interval=2
    
    while [ $elapsed -lt $timeout ]; do
        local query='{"lock_nonce":{}}'
        local result=$(query_terra_contract "$bridge" "$query")
        current_nonce=$(echo "$result" | jq -r '.data.nonce // "0"' 2>/dev/null || echo "0")
        
        if [ "$current_nonce" -gt "$initial_nonce" ]; then
            log_pass "Lock detected! Nonce: $initial_nonce -> $current_nonce"
            return 0
        fi
        
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_error "No Lock event within ${timeout}s"
    return 1
}

# Wait for balance change on EVM
# Usage: wait_for_evm_balance_change <token> <account> <initial_balance> [timeout]
wait_for_evm_balance_change() {
    local token="$1"
    local account="$2"
    local initial_balance="$3"
    local timeout="${4:-120}"
    
    log_info "Waiting for balance change on $account..."
    
    local elapsed=0
    local interval=2
    
    while [ $elapsed -lt $timeout ]; do
        current_balance=$(get_erc20_balance "$token" "$account")
        
        if [ "$current_balance" != "$initial_balance" ]; then
            log_pass "Balance changed: $initial_balance -> $current_balance"
            echo "$current_balance"
            return 0
        fi
        
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_error "No balance change within ${timeout}s"
    return 1
}

# Wait for balance change on Terra
# Usage: wait_for_terra_balance_change <address> <initial_balance> [denom] [timeout]
wait_for_terra_balance_change() {
    local address="$1"
    local initial_balance="$2"
    local denom="${3:-uluna}"
    local timeout="${4:-120}"
    
    log_info "Waiting for Terra balance change on $address..."
    
    local elapsed=0
    local interval=2
    
    while [ $elapsed -lt $timeout ]; do
        current_balance=$(get_terra_balance "$address" "$denom")
        
        if [ "$current_balance" != "$initial_balance" ]; then
            log_pass "Balance changed: $initial_balance -> $current_balance"
            echo "$current_balance"
            return 0
        fi
        
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_error "No balance change within ${timeout}s"
    return 1
}

# Wait for withdrawal to be executable (delay passed)
# Usage: wait_for_withdrawal_ready <bridge_address> <nonce> [timeout]
wait_for_withdrawal_ready() {
    local bridge="$1"
    local nonce="$2"
    local timeout="${3:-300}"
    
    log_info "Waiting for withdrawal to be executable (nonce: $nonce)..."
    
    # Get the withdraw delay
    local delay=$(cast call "$bridge" "withdrawDelay()" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "60")
    
    log_info "Withdraw delay is $delay seconds"
    
    # Skip time on Anvil
    skip_anvil_time $((delay + 10))
    
    log_pass "Time skipped, withdrawal should be executable"
    return 0
}

# Execute withdrawal on EVM
# Usage: execute_evm_withdrawal <bridge_address> <nonce>
execute_evm_withdrawal() {
    local bridge="$1"
    local nonce="$2"
    
    log_info "Executing withdrawal (nonce: $nonce)..."
    
    local result=$(cast send "$bridge" "withdraw(uint256)" "$nonce" \
        --rpc-url "$EVM_RPC_URL" \
        --private-key "$EVM_PRIVATE_KEY" \
        --json 2>&1)
    
    if echo "$result" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
        local tx_hash=$(echo "$result" | jq -r '.transactionHash')
        log_pass "Withdrawal executed successfully!"
        log_info "Transaction hash: $tx_hash"
        echo "$tx_hash"
        return 0
    else
        log_error "Withdrawal failed: $result"
        return 1
    fi
}

# Export functions for sourcing
export -f wait_for_deposit_event
export -f wait_for_approval_event
export -f wait_for_terra_lock
export -f wait_for_evm_balance_change
export -f wait_for_terra_balance_change
export -f wait_for_withdrawal_ready
export -f execute_evm_withdrawal
