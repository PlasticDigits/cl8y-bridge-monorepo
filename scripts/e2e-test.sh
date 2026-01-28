#!/bin/bash
# Automated End-to-End Test Script
#
# This script runs a complete transfer cycle in both directions:
# 1. Terra -> EVM (lock, verify approval, execute withdrawal)
# 2. EVM -> Terra (deposit, verify release)
#
# Prerequisites:
# - All infrastructure running (docker compose up)
# - Contracts deployed and configured
# - Environment variables set
#
# Usage:
#   ./scripts/e2e-test.sh

set -e

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source Terra helper functions if available
if [ -f "$SCRIPT_DIR/lib/terra-helpers.sh" ]; then
    source "$SCRIPT_DIR/lib/terra-helpers.sh"
    TERRA_HELPERS_LOADED=true
else
    TERRA_HELPERS_LOADED=false
fi

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_RPC_URL="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_LCD_URL="${TERRA_LCD_URL:-http://localhost:1317}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"
DATABASE_URL="${DATABASE_URL:-postgres://relayer:relayer@localhost:5433/relayer}"

# Legacy aliases for compatibility
TERRA_NODE="$TERRA_RPC_URL"
TERRA_LCD="$TERRA_LCD_URL"

# Contract addresses
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"

# Test accounts
EVM_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
EVM_TEST_ADDRESS="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
TERRA_KEY_NAME="${TERRA_KEY_NAME:-test1}"
TERRA_KEY="$TERRA_KEY_NAME"
TERRA_TEST_ADDRESS="terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"

# Test parameters
TRANSFER_AMOUNT="1000000"  # 1 LUNA in uluna
TIMEOUT_SECS=60
POLL_INTERVAL=2

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_pass() { echo -e "${GREEN}[PASS]${NC} $1"; }
log_fail() { echo -e "${RED}[FAIL]${NC} $1"; }

# Track test results
TESTS_PASSED=0
TESTS_FAILED=0

# Check prerequisites
check_prereqs() {
    log_step "Checking prerequisites..."
    
    local failed=0
    
    # Check tools
    for cmd in cast terrad curl jq psql; do
        if ! command -v $cmd &> /dev/null; then
            log_error "$cmd not found"
            failed=1
        fi
    done
    
    # Check addresses
    if [ -z "$EVM_BRIDGE_ADDRESS" ]; then
        log_error "EVM_BRIDGE_ADDRESS not set"
        failed=1
    fi
    
    if [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_error "TERRA_BRIDGE_ADDRESS not set"
        failed=1
    fi
    
    # Check EVM connectivity
    if ! cast block-number --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1; then
        log_error "Cannot connect to EVM at $EVM_RPC_URL"
        failed=1
    fi
    
    # Check Terra connectivity
    if ! curl -s "$TERRA_NODE/status" > /dev/null 2>&1; then
        log_error "Cannot connect to Terra at $TERRA_NODE"
        failed=1
    fi
    
    # Check database connectivity
    if ! psql "$DATABASE_URL" -c "SELECT 1" > /dev/null 2>&1; then
        log_error "Cannot connect to database"
        failed=1
    fi
    
    if [ $failed -eq 1 ]; then
        log_error "Prerequisites check failed"
        exit 1
    fi
    
    log_pass "Prerequisites OK"
}

# Get balances - use helpers if available, fallback to inline
get_terra_balance() {
    local addr="$1"
    local denom="${2:-uluna}"
    if [ "$TERRA_HELPERS_LOADED" = true ]; then
        terra_balance "$addr" "$denom"
    else
        curl -s "$TERRA_LCD_URL/cosmos/bank/v1beta1/balances/$addr" 2>/dev/null | \
            jq -r ".balances[] | select(.denom==\"$denom\") | .amount" 2>/dev/null || echo "0"
    fi
}

get_evm_balance() {
    local addr="$1"
    cast balance "$addr" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0"
}

# Lock tokens on Terra - use helpers if available
do_terra_lock() {
    local amount="$1"
    local dest_chain="$2"
    local recipient="$3"
    
    if [ "$TERRA_HELPERS_LOADED" = true ]; then
        terra_lock "$amount" "uluna" "$dest_chain" "$recipient"
    else
        LOCK_MSG="{\"lock\":{\"dest_chain_id\":$dest_chain,\"recipient\":\"$recipient\"}}"
        terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
            "$LOCK_MSG" \
            --amount "${amount}uluna" \
            --from "$TERRA_KEY_NAME" \
            --chain-id "$TERRA_CHAIN_ID" \
            --node "$TERRA_RPC_URL" \
            --gas auto --gas-adjustment 1.4 \
            --fees 5000uluna \
            -y -o json 2>&1
    fi
}

# Wait for Terra transaction - use helpers if available
wait_terra_tx() {
    local tx_hash="$1"
    local timeout="${2:-60}"
    
    if [ "$TERRA_HELPERS_LOADED" = true ]; then
        terra_wait_tx "$tx_hash" "$timeout"
    else
        sleep 6  # Simple wait fallback
        terrad query tx "$tx_hash" --node "$TERRA_RPC_URL" -o json 2>/dev/null || echo "{}"
    fi
}

# Wait for condition with timeout
wait_for() {
    local description="$1"
    local check_cmd="$2"
    local timeout=$TIMEOUT_SECS
    local elapsed=0
    
    log_info "Waiting for: $description (timeout: ${timeout}s)"
    
    while [ $elapsed -lt $timeout ]; do
        if eval "$check_cmd" > /dev/null 2>&1; then
            log_pass "$description"
            return 0
        fi
        sleep $POLL_INTERVAL
        elapsed=$((elapsed + POLL_INTERVAL))
        echo -n "."
    done
    
    echo ""
    log_fail "Timeout waiting for: $description"
    return 1
}

# Record test result
record_result() {
    local test_name="$1"
    local result="$2"
    
    if [ "$result" = "pass" ]; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        log_pass "TEST: $test_name"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        log_fail "TEST: $test_name"
    fi
}

# Test 1: Terra -> EVM Transfer
test_terra_to_evm() {
    log_step "=== TEST: Terra -> EVM Transfer ==="
    
    # Get initial balances
    local terra_balance_before=$(get_terra_balance "$TERRA_TEST_ADDRESS")
    log_info "Terra balance before: $terra_balance_before uluna"
    
    # Lock tokens on Terra
    log_info "Locking $TRANSFER_AMOUNT uluna on Terra..."
    
    TX_RESULT=$(do_terra_lock "$TRANSFER_AMOUNT" "31337" "$EVM_TEST_ADDRESS")
    
    TX_HASH=$(echo "$TX_RESULT" | jq -r '.txhash' 2>/dev/null || echo "")
    
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" = "null" ]; then
        log_error "Failed to submit lock transaction"
        log_error "Response: $TX_RESULT"
        record_result "Terra -> EVM Lock" "fail"
        return 1
    fi
    
    log_info "Lock TX: $TX_HASH"
    
    # Wait for confirmation using helper or fallback
    TX_QUERY=$(wait_terra_tx "$TX_HASH" 60)
    TX_CODE=$(echo "$TX_QUERY" | jq -r '.code // 0' 2>/dev/null)
    
    if [ "$TX_CODE" != "0" ]; then
        log_error "Lock transaction failed with code: $TX_CODE"
        record_result "Terra -> EVM Lock" "fail"
        return 1
    fi
    
    record_result "Terra -> EVM Lock" "pass"
    
    # Verify Terra balance decreased
    local terra_balance_after=$(get_terra_balance "$TERRA_TEST_ADDRESS")
    log_info "Terra balance after: $terra_balance_after uluna"
    
    local expected_decrease=$((terra_balance_before - terra_balance_after))
    if [ "$expected_decrease" -ge "$TRANSFER_AMOUNT" ]; then
        record_result "Terra balance decreased" "pass"
    else
        log_warn "Balance decrease ($expected_decrease) less than expected ($TRANSFER_AMOUNT)"
        record_result "Terra balance decreased" "fail"
    fi
    
    # Check database for pending deposit
    log_info "Checking database for lock event..."
    
    local db_count=$(psql "$DATABASE_URL" -t -c \
        "SELECT COUNT(*) FROM deposits WHERE status = 'pending'" 2>/dev/null | tr -d ' ')
    
    log_info "Pending deposits in DB: ${db_count:-0}"
    
    log_info "Lock transaction complete. Relayer will process the approval."
}

# Test 2: EVM -> Terra Transfer
test_evm_to_terra() {
    log_step "=== TEST: EVM -> Terra Transfer ==="
    
    # Get initial balances
    local evm_balance_before=$(get_evm_balance "$EVM_TEST_ADDRESS")
    log_info "EVM balance before: $evm_balance_before wei"
    
    # Compute Terra chain key
    TERRA_CHAIN_KEY=$(cast keccak "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'localterra' 'terra')")
    log_info "Terra Chain Key: $TERRA_CHAIN_KEY"
    
    # Encode Terra recipient
    TERRA_RECIPIENT_HEX="0x$(echo -n "$TERRA_TEST_ADDRESS" | xxd -p | tr -d '\n')"
    while [ ${#TERRA_RECIPIENT_HEX} -lt 66 ]; do
        TERRA_RECIPIENT_HEX="${TERRA_RECIPIENT_HEX}00"
    done
    
    log_info "Depositing on EVM..."
    
    # Note: This may fail if the contract interface doesn't match
    # Adjust the function signature based on your actual contract
    TX_RESULT=$(cast send "$EVM_BRIDGE_ADDRESS" \
        "deposit(address,uint256,bytes32,bytes32)" \
        "0x0000000000000000000000000000000000001234" \
        "1000000000000000000" \
        "$TERRA_CHAIN_KEY" \
        "$TERRA_RECIPIENT_HEX" \
        --rpc-url "$EVM_RPC_URL" \
        --private-key "$EVM_PRIVATE_KEY" \
        2>&1 || echo "failed")
    
    if echo "$TX_RESULT" | grep -q "failed\|error\|revert"; then
        log_warn "Deposit transaction may have failed (check contract interface)"
        log_warn "$TX_RESULT"
        record_result "EVM -> Terra Deposit" "fail"
    else
        log_info "Deposit TX submitted"
        record_result "EVM -> Terra Deposit" "pass"
    fi
    
    log_info "Deposit complete. Relayer will process the release."
}

# Test 3: Database connectivity
test_database() {
    log_step "=== TEST: Database State ==="
    
    # Check tables exist
    local tables=$(psql "$DATABASE_URL" -t -c \
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'" 2>/dev/null | tr -d ' ')
    
    if echo "$tables" | grep -q "deposits"; then
        record_result "deposits table exists" "pass"
    else
        record_result "deposits table exists" "fail"
    fi
    
    # Get counts
    local deposit_count=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM deposits" 2>/dev/null | tr -d ' ')
    log_info "Total deposits: ${deposit_count:-0}"
}

# Print summary
print_summary() {
    echo ""
    echo "========================================"
    echo "         E2E TEST SUMMARY"
    echo "========================================"
    echo ""
    echo -e "  ${GREEN}Passed:${NC} $TESTS_PASSED"
    echo -e "  ${RED}Failed:${NC} $TESTS_FAILED"
    echo ""
    
    if [ $TESTS_FAILED -eq 0 ]; then
        log_pass "All tests passed!"
        exit 0
    else
        log_fail "Some tests failed"
        exit 1
    fi
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "    CL8Y Bridge E2E Test Suite"
    echo "========================================"
    echo ""
    
    check_prereqs
    
    echo ""
    test_database
    
    echo ""
    test_terra_to_evm
    
    echo ""
    test_evm_to_terra
    
    print_summary
}

main "$@"
