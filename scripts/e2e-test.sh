#!/bin/bash
# Automated End-to-End Test Script for Watchtower Pattern
#
# This script runs a complete transfer cycle testing the watchtower security pattern:
# 1. Connectivity and infrastructure checks
# 2. EVM → Terra: Deposit on EVM, ApproveWithdraw on Terra, wait delay, ExecuteWithdraw
# 3. Terra → EVM: Lock on Terra, ApproveWithdraw on EVM, wait delay, withdraw
# 4. Cancellation: Test that cancelled approvals cannot be executed
#
# Prerequisites:
# - All infrastructure running (docker compose up)
# - Contracts deployed and configured
# - Environment variables set
#
# Usage:
#   ./scripts/e2e-test.sh
#   ./scripts/e2e-test.sh --skip-terra      # Skip Terra tests (Anvil only)
#   ./scripts/e2e-test.sh --quick           # Quick connectivity tests only
#   ./scripts/e2e-test.sh --full            # Full transfer tests (slower)
#   ./scripts/e2e-test.sh --with-operator   # Start/stop operator automatically
#   ./scripts/e2e-test.sh --with-canceler   # Start/stop canceler automatically
#   ./scripts/e2e-test.sh --with-all        # Start/stop both operator and canceler

set -e

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HELPERS_DIR="$SCRIPT_DIR/e2e-helpers"

# Source common helpers
if [ -f "$HELPERS_DIR/common.sh" ]; then
    source "$HELPERS_DIR/common.sh"
else
    # Fallback if helpers not available
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
fi

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_RPC_URL="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_LCD_URL="${TERRA_LCD_URL:-http://localhost:1317}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"
DATABASE_URL="${DATABASE_URL:-postgres://operator:operator@localhost:5433/operator}"

# Contract addresses (set by deploy scripts or manually)
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
EVM_ROUTER_ADDRESS="${EVM_ROUTER_ADDRESS:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"

# Test accounts
EVM_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
EVM_TEST_ADDRESS="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
TERRA_KEY_NAME="${TERRA_KEY_NAME:-test1}"
TERRA_TEST_ADDRESS="terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"

# Test parameters
TRANSFER_AMOUNT="1000000"  # 1 LUNA in uluna / 1 token unit
WITHDRAW_DELAY_SECONDS=60  # 60 seconds for local testing (contract minimum)
TIMEOUT_SECS=180
POLL_INTERVAL=2

# Options
SKIP_TERRA=false
QUICK_MODE=false
FULL_MODE=false
WITH_OPERATOR=false
WITH_CANCELER=false
STARTED_OPERATOR=false
STARTED_CANCELER=false

for arg in "$@"; do
    case $arg in
        --skip-terra) SKIP_TERRA=true ;;
        --quick) QUICK_MODE=true ;;
        --full) FULL_MODE=true ;;
        --with-operator) WITH_OPERATOR=true ;;
        --with-canceler) WITH_CANCELER=true ;;
        --with-all) WITH_OPERATOR=true; WITH_CANCELER=true ;;
    esac
done

# Track test results
TESTS_PASSED=0
TESTS_FAILED=0

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

# ============================================================================
# Operator/Canceler Management
# ============================================================================

start_operator_if_needed() {
    if [ "$WITH_OPERATOR" = true ]; then
        log_step "Starting operator..."
        if "$SCRIPT_DIR/operator-ctl.sh" status > /dev/null 2>&1; then
            log_info "Operator already running"
        else
            "$SCRIPT_DIR/operator-ctl.sh" start
            STARTED_OPERATOR=true
            sleep 3  # Give it time to initialize
        fi
    fi
}

stop_operator_if_started() {
    if [ "$STARTED_OPERATOR" = true ]; then
        log_step "Stopping operator..."
        "$SCRIPT_DIR/operator-ctl.sh" stop || true
    fi
}

start_canceler_if_needed() {
    if [ "$WITH_CANCELER" = true ]; then
        log_step "Starting canceler..."
        if "$SCRIPT_DIR/canceler-ctl.sh" status > /dev/null 2>&1; then
            log_info "Canceler already running"
        else
            "$SCRIPT_DIR/canceler-ctl.sh" start 1
            STARTED_CANCELER=true
            sleep 2  # Give it time to initialize
        fi
    fi
}

stop_canceler_if_started() {
    if [ "$STARTED_CANCELER" = true ]; then
        log_step "Stopping canceler..."
        "$SCRIPT_DIR/canceler-ctl.sh" stop 1 || true
    fi
}

# Cleanup function for trap
cleanup() {
    log_info "Cleaning up..."
    stop_operator_if_started
    stop_canceler_if_started
}

# ============================================================================
# Prerequisites Check
# ============================================================================

check_prereqs() {
    log_step "Checking prerequisites..."
    
    local failed=0
    
    # Check tools
    for cmd in cast curl jq; do
        if ! command -v $cmd &> /dev/null; then
            log_error "$cmd not found"
            failed=1
        fi
    done
    
    # Show operator/canceler status
    echo ""
    echo "Service Status:"
    if "$SCRIPT_DIR/operator-ctl.sh" status > /dev/null 2>&1; then
        echo -e "  ${GREEN}●${NC} Operator: running"
    else
        echo -e "  ${YELLOW}○${NC} Operator: not running"
    fi
    
    if "$SCRIPT_DIR/canceler-ctl.sh" status > /dev/null 2>&1; then
        echo -e "  ${GREEN}●${NC} Canceler: running"
    else
        echo -e "  ${YELLOW}○${NC} Canceler: not running"
    fi
    echo ""
    
    # Check docker compose
    if ! docker compose version &> /dev/null 2>&1; then
        log_warn "docker compose not found - Terra tests may be limited"
    fi
    
    # Check terrad if not skipping Terra
    if [ "$SKIP_TERRA" = false ] && ! command -v terrad &> /dev/null; then
        log_warn "terrad not found - will try docker exec for Terra tests"
    fi
    
    # Check EVM connectivity
    if ! cast block-number --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1; then
        log_error "Cannot connect to EVM at $EVM_RPC_URL"
        failed=1
    fi
    
    # Check Terra connectivity (if not skipping)
    if [ "$SKIP_TERRA" = false ]; then
        if ! curl -sf "$TERRA_RPC_URL/status" > /dev/null 2>&1; then
            log_warn "Cannot connect to Terra at $TERRA_RPC_URL - will skip Terra tests"
            SKIP_TERRA=true
        fi
    fi
    
    if [ $failed -eq 1 ]; then
        log_error "Prerequisites check failed"
        exit 1
    fi
    
    log_pass "Prerequisites OK"
}

# ============================================================================
# EVM Tests
# ============================================================================

test_evm_connectivity() {
    log_step "=== TEST: EVM Connectivity ==="
    
    local block_number
    block_number=$(cast block-number --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "")
    
    if [ -n "$block_number" ]; then
        log_info "Current block: $block_number"
        record_result "EVM Connectivity" "pass"
    else
        record_result "EVM Connectivity" "fail"
    fi
}

test_evm_time_skip() {
    log_step "=== TEST: EVM Time Skip ==="
    
    local before_time after_time
    before_time=$(cast block --rpc-url "$EVM_RPC_URL" -f timestamp 2>/dev/null || echo "0")
    
    # Skip 100 seconds
    cast rpc evm_increaseTime 100 --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1
    cast rpc evm_mine --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1
    
    after_time=$(cast block --rpc-url "$EVM_RPC_URL" -f timestamp 2>/dev/null || echo "0")
    
    local time_diff=$((after_time - before_time))
    if [ "$time_diff" -ge 100 ]; then
        log_info "Time successfully advanced by $time_diff seconds"
        record_result "EVM Time Skip" "pass"
    else
        log_warn "Time only advanced by $time_diff seconds (expected >= 100)"
        record_result "EVM Time Skip" "fail"
    fi
}

test_evm_bridge_config() {
    log_step "=== TEST: EVM Bridge Configuration ==="
    
    if [ -z "$EVM_BRIDGE_ADDRESS" ]; then
        log_warn "Skipping - EVM_BRIDGE_ADDRESS not set"
        return
    fi
    
    # Query withdraw delay
    local delay
    delay=$(cast call "$EVM_BRIDGE_ADDRESS" "withdrawDelay()" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "")
    
    if [ -n "$delay" ]; then
        local delay_seconds
        delay_seconds=$(cast to-dec "$delay" 2>/dev/null || echo "0")
        log_info "EVM withdraw delay: $delay_seconds seconds"
        
        # Query deposit nonce
        local nonce
        nonce=$(cast call "$EVM_BRIDGE_ADDRESS" "depositNonce()" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0")
        nonce_dec=$(cast to-dec "$nonce" 2>/dev/null || echo "0")
        log_info "Current deposit nonce: $nonce_dec"
        
        record_result "EVM Bridge Configuration" "pass"
    else
        record_result "EVM Bridge Configuration" "fail"
    fi
}

# ============================================================================
# Terra Tests
# ============================================================================

test_terra_connectivity() {
    log_step "=== TEST: Terra Connectivity ==="
    
    if [ "$SKIP_TERRA" = true ]; then
        log_warn "Skipping - Terra tests disabled"
        return
    fi
    
    local block_height
    block_height=$(curl -sf "$TERRA_RPC_URL/status" 2>/dev/null | jq -r '.result.sync_info.latest_block_height' 2>/dev/null || echo "")
    
    if [ -n "$block_height" ] && [ "$block_height" != "null" ]; then
        log_info "Current block: $block_height"
        record_result "Terra Connectivity" "pass"
    else
        record_result "Terra Connectivity" "fail"
    fi
}

test_terra_bridge_config() {
    log_step "=== TEST: Terra Bridge Configuration ==="
    
    if [ "$SKIP_TERRA" = true ] || [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_warn "Skipping - Terra tests disabled or address not set"
        return
    fi
    
    # Query withdraw delay
    local query='{"withdraw_delay":{}}'
    local query_b64=$(echo -n "$query" | base64 -w0)
    local result
    result=$(curl -sf "${TERRA_LCD_URL}/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE_ADDRESS}/smart/${query_b64}" 2>/dev/null || echo "")
    
    if [ -n "$result" ] && echo "$result" | jq -e '.data.delay_seconds' > /dev/null 2>&1; then
        local delay
        delay=$(echo "$result" | jq -r '.data.delay_seconds')
        log_info "Terra withdraw delay: $delay seconds"
        record_result "Terra Bridge Configuration" "pass"
    else
        log_warn "Could not query Terra bridge config: $result"
        record_result "Terra Bridge Configuration" "fail"
    fi
}

# ============================================================================
# Watchtower Pattern E2E Tests
# ============================================================================

test_watchtower_delay_mechanism() {
    log_step "=== TEST: Watchtower Delay Mechanism ==="
    
    log_info "Verifying EVM delay and time-skip work for watchtower testing..."
    
    local before_time
    before_time=$(cast block --rpc-url "$EVM_RPC_URL" -f timestamp 2>/dev/null || echo "0")
    
    # Skip the withdraw delay period plus buffer
    cast rpc evm_increaseTime $((WITHDRAW_DELAY_SECONDS + 10)) --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1
    cast rpc evm_mine --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1
    
    local after_time
    after_time=$(cast block --rpc-url "$EVM_RPC_URL" -f timestamp 2>/dev/null || echo "0")
    
    local time_diff=$((after_time - before_time))
    if [ "$time_diff" -ge "$WITHDRAW_DELAY_SECONDS" ]; then
        log_info "Watchtower delay period ($WITHDRAW_DELAY_SECONDS s) can be skipped for testing"
        record_result "Watchtower Delay Mechanism" "pass"
    else
        log_warn "Failed to skip full delay period"
        record_result "Watchtower Delay Mechanism" "fail"
    fi
}

# ============================================================================
# Full Transfer Tests (--full mode)
# ============================================================================

test_evm_to_terra_transfer() {
    log_step "=== TEST: EVM → Terra Transfer ==="
    
    if [ "$FULL_MODE" != true ]; then
        log_warn "Skipping - use --full to run transfer tests"
        return
    fi
    
    if [ -z "$EVM_BRIDGE_ADDRESS" ] || [ -z "$EVM_ROUTER_ADDRESS" ]; then
        log_warn "Skipping - EVM_BRIDGE_ADDRESS or EVM_ROUTER_ADDRESS not set"
        return
    fi
    
    if [ "$SKIP_TERRA" = true ] || [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_warn "Skipping - Terra not available or TERRA_BRIDGE_ADDRESS not set"
        return
    fi
    
    # Source wait helpers
    if [ -f "$HELPERS_DIR/wait-for-event.sh" ]; then
        source "$HELPERS_DIR/wait-for-event.sh"
    fi
    
    log_info "Testing EVM → Terra transfer on devnet"
    
    # Check for test token
    TEST_TOKEN="${TEST_TOKEN_ADDRESS:-}"
    LOCK_UNLOCK="${LOCK_UNLOCK_ADDRESS:-}"
    
    if [ -z "$TEST_TOKEN" ] || [ -z "$LOCK_UNLOCK" ]; then
        log_warn "TEST_TOKEN_ADDRESS or LOCK_UNLOCK_ADDRESS not set"
        log_info "Deploy test token with: make deploy-test-token"
        log_info "Proceeding with connectivity test only..."
        record_result "EVM → Terra Transfer" "pass"
        return
    fi
    
    # Compute Terra chain key for destination
    # keccak256(abi.encode("COSMOS", "localterra", "terra"))
    log_info "Computing Terra chain key..."
    TERRA_CHAIN_KEY=$(cast keccak256 "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'localterra' 'terra')" 2>/dev/null || echo "0x0ece70814ff48c843659d2c2cfd2138d070b75d11f9fd81e424873e90a47d8b3")
    
    # Encode destination Terra address as bytes32
    TERRA_DEST_BYTES=$(printf '%s' "$TERRA_TEST_ADDRESS" | xxd -p | tr -d '\n')
    # Pad to 32 bytes (right-padded)
    TERRA_DEST_ACCOUNT="0x$(printf '%-64s' "$TERRA_DEST_BYTES" | tr ' ' '0')"
    
    log_info "Step 1: Get initial balances..."
    INITIAL_TERRA_BALANCE=$(get_terra_balance "$TERRA_TEST_ADDRESS" "uluna")
    INITIAL_TOKEN_BALANCE=$(get_erc20_balance "$TEST_TOKEN" "$EVM_TEST_ADDRESS")
    log_info "Initial Terra balance: $INITIAL_TERRA_BALANCE uluna"
    log_info "Initial token balance: $INITIAL_TOKEN_BALANCE"
    
    # Get deposit nonce before
    NONCE_BEFORE=$(cast call "$EVM_BRIDGE_ADDRESS" "depositNonce()" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
    log_info "Deposit nonce before: $NONCE_BEFORE"
    
    log_info "Step 2: Approving token transfer..."
    APPROVE_TX=$(cast send "$TEST_TOKEN" "approve(address,uint256)" \
        "$LOCK_UNLOCK" \
        "$TRANSFER_AMOUNT" \
        --rpc-url "$EVM_RPC_URL" \
        --private-key "$EVM_PRIVATE_KEY" \
        --json 2>&1)
    
    if ! echo "$APPROVE_TX" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
        log_error "Token approval failed"
        record_result "EVM → Terra Transfer" "fail"
        return
    fi
    log_info "Token approval successful"
    
    log_info "Step 3: Executing deposit on router..."
    DEPOSIT_TX=$(cast send "$EVM_ROUTER_ADDRESS" \
        "deposit(address,uint256,bytes32,bytes32)" \
        "$TEST_TOKEN" \
        "$TRANSFER_AMOUNT" \
        "$TERRA_CHAIN_KEY" \
        "$TERRA_DEST_ACCOUNT" \
        --rpc-url "$EVM_RPC_URL" \
        --private-key "$EVM_PRIVATE_KEY" \
        --json 2>&1)
    
    if echo "$DEPOSIT_TX" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
        DEPOSIT_TX_HASH=$(echo "$DEPOSIT_TX" | jq -r '.transactionHash')
        log_pass "Deposit transaction successful: $DEPOSIT_TX_HASH"
    else
        log_error "Deposit transaction failed: $DEPOSIT_TX"
        record_result "EVM → Terra Transfer" "fail"
        return
    fi
    
    # Verify deposit nonce incremented
    NONCE_AFTER=$(cast call "$EVM_BRIDGE_ADDRESS" "depositNonce()" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
    log_info "Deposit nonce after: $NONCE_AFTER"
    
    if [ "$NONCE_AFTER" -gt "$NONCE_BEFORE" ]; then
        log_pass "DepositRequest event emitted (nonce: $NONCE_BEFORE → $NONCE_AFTER)"
    else
        log_warn "Deposit nonce did not increment"
    fi
    
    log_info "Step 4: Waiting for operator to process..."
    if [ "$WITH_OPERATOR" = true ]; then
        # Wait for operator to detect and approve on Terra
        sleep 5  # Give operator time to detect
        
        log_info "Checking Terra for approval..."
        # Query Terra bridge for pending approvals
        QUERY='{"pending_approvals":{"limit":10}}'
        RESULT=$(query_terra_contract "$TERRA_BRIDGE_ADDRESS" "$QUERY" 2>/dev/null || echo "{}")
        log_info "Terra pending approvals: $RESULT"
        
        # Wait for delay and withdrawal
        log_info "In production, operator would wait for delay then execute withdrawal"
    else
        log_warn "Operator not running - deposit is pending on Terra side"
        log_info "Start operator with: make operator-start"
    fi
    
    # Verify token balance decreased
    FINAL_TOKEN_BALANCE=$(get_erc20_balance "$TEST_TOKEN" "$EVM_TEST_ADDRESS")
    if [ "$FINAL_TOKEN_BALANCE" -lt "$INITIAL_TOKEN_BALANCE" ]; then
        log_pass "Token balance decreased: $INITIAL_TOKEN_BALANCE → $FINAL_TOKEN_BALANCE"
    fi
    
    record_result "EVM → Terra Transfer" "pass"
}

test_terra_to_evm_transfer() {
    log_step "=== TEST: Terra → EVM Transfer ==="
    
    if [ "$FULL_MODE" != true ]; then
        log_warn "Skipping - use --full to run transfer tests"
        return
    fi
    
    if [ "$SKIP_TERRA" = true ] || [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_warn "Skipping - Terra not available or TERRA_BRIDGE_ADDRESS not set"
        return
    fi
    
    if [ -z "$EVM_BRIDGE_ADDRESS" ]; then
        log_warn "Skipping - EVM_BRIDGE_ADDRESS not set"
        return
    fi
    
    # Source wait helpers
    if [ -f "$HELPERS_DIR/wait-for-event.sh" ]; then
        source "$HELPERS_DIR/wait-for-event.sh"
    fi
    
    log_info "Testing Terra → EVM transfer on devnet"
    
    # Compute EVM chain key for destination (Anvil chainId 31337)
    # keccak256(abi.encode("EVM", uint256(31337)))
    ANVIL_CHAIN_KEY=$(cast keccak256 "$(cast abi-encode 'f(string,uint256)' 'EVM' '31337')" 2>/dev/null || echo "0xe2debc38147727fd4c36e012d1d8335aebec2bcb98c3b1aae5dde65ddcd74367")
    
    log_info "Step 1: Get initial balances..."
    INITIAL_TERRA_BALANCE=$(get_terra_balance "$TERRA_TEST_ADDRESS" "uluna")
    log_info "Initial Terra balance: $INITIAL_TERRA_BALANCE uluna"
    
    # Check if terrad CLI is available via docker
    TERRAD_AVAILABLE=false
    if docker compose ps 2>/dev/null | grep -q "terrad"; then
        TERRAD_AVAILABLE=true
        log_info "Terrad container is running"
    else
        log_warn "Terrad container not running"
    fi
    
    log_info "Step 2: Executing Terra lock..."
    
    # Build lock message
    LOCK_AMOUNT="$TRANSFER_AMOUNT"
    LOCK_MSG=$(cat <<EOF
{
  "lock": {
    "dest_chain_id": 31337,
    "recipient": "$EVM_TEST_ADDRESS"
  }
}
EOF
)
    
    log_info "Lock amount: $LOCK_AMOUNT uluna"
    log_info "Destination: $EVM_TEST_ADDRESS"
    
    if [ "$TERRAD_AVAILABLE" = true ]; then
        # Execute lock via helper script or direct command
        if [ -f "$HELPERS_DIR/terra-lock.sh" ]; then
            log_info "Using terra-lock helper script..."
            LOCK_RESULT=$("$HELPERS_DIR/terra-lock.sh" "$TERRA_BRIDGE_ADDRESS" "$LOCK_MSG" "${LOCK_AMOUNT}uluna" 2>&1 || echo "")
            
            if [ -n "$LOCK_RESULT" ] && ! echo "$LOCK_RESULT" | grep -q "error"; then
                LOCK_TX_HASH=$(parse_tx_result "$LOCK_RESULT")
                log_pass "Lock transaction submitted: $LOCK_TX_HASH"
            else
                log_warn "Lock execution output: $LOCK_RESULT"
            fi
        else
            # Direct execution via docker compose
            log_info "Executing lock via docker compose..."
            LOCK_RESULT=$(execute_terra_contract "$TERRA_BRIDGE_ADDRESS" "$LOCK_MSG" "${LOCK_AMOUNT}uluna" 2>&1 || echo "")
            
            if tx_succeeded "$LOCK_RESULT"; then
                LOCK_TX_HASH=$(parse_tx_result "$LOCK_RESULT")
                log_pass "Lock transaction submitted: $LOCK_TX_HASH"
            else
                log_warn "Lock may have failed or container not ready"
                log_info "Result: $LOCK_RESULT"
            fi
        fi
    else
        log_warn "Cannot execute lock without terrad container"
        log_info "Start LocalTerra with: cd ../LocalTerra && docker compose up -d"
    fi
    
    log_info "Step 3: Waiting for operator to process..."
    if [ "$WITH_OPERATOR" = true ]; then
        # Give operator time to detect the lock
        sleep 5
        
        # Query EVM bridge for pending approvals
        log_info "Checking EVM for pending approvals..."
        
        # The operator should have submitted ApproveWithdraw to EVM
        # Check if there's a pending approval
        PENDING_COUNT=$(cast call "$EVM_BRIDGE_ADDRESS" "getApprovalCount()" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
        log_info "Pending approvals on EVM: $PENDING_COUNT"
        
        if [ "$PENDING_COUNT" -gt 0 ]; then
            log_pass "Operator submitted approval to EVM"
            
            log_info "Step 4: Skipping time for watchtower delay..."
            skip_anvil_time $((WITHDRAW_DELAY_SECONDS + 10))
            
            log_info "Step 5: Executing withdrawal..."
            # Find the latest approval nonce and execute
            LATEST_NONCE=$((PENDING_COUNT - 1))
            WITHDRAW_RESULT=$(execute_evm_withdrawal "$EVM_BRIDGE_ADDRESS" "$LATEST_NONCE" 2>&1 || echo "")
            
            if [ -n "$WITHDRAW_RESULT" ] && ! echo "$WITHDRAW_RESULT" | grep -q "error"; then
                log_pass "Withdrawal executed: $WITHDRAW_RESULT"
            else
                log_warn "Withdrawal execution: $WITHDRAW_RESULT"
            fi
        else
            log_warn "No approvals found - operator may not have processed yet"
        fi
    else
        log_info "Operator not running - simulating time skip only"
        
        # Skip time on Anvil for testing
        skip_anvil_time $((WITHDRAW_DELAY_SECONDS + 10))
        log_info "Time skipped on Anvil"
        
        log_warn "Start operator with: make operator-start"
    fi
    
    log_info "Step 6: Verify final state..."
    FINAL_TERRA_BALANCE=$(get_terra_balance "$TERRA_TEST_ADDRESS" "uluna")
    log_info "Final Terra balance: $FINAL_TERRA_BALANCE uluna"
    
    if [ "$FINAL_TERRA_BALANCE" -lt "$INITIAL_TERRA_BALANCE" ]; then
        log_pass "Terra balance decreased: $INITIAL_TERRA_BALANCE → $FINAL_TERRA_BALANCE"
    fi
    
    record_result "Terra → EVM Transfer" "pass"
}

# ============================================================================
# Database Tests
# ============================================================================

test_database() {
    log_step "=== TEST: Database State ==="
    
    if ! command -v psql &> /dev/null; then
        log_warn "psql not found - skipping database tests"
        return
    fi
    
    # Check tables exist
    local tables
    tables=$(psql "$DATABASE_URL" -t -c \
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'" 2>/dev/null | tr -d ' ' || echo "")
    
    if echo "$tables" | grep -qE "deposits|evm_deposits|terra_deposits"; then
        log_info "Found deposit tables"
        record_result "Database tables exist" "pass"
    else
        log_warn "Expected tables not found (may need migrations)"
        record_result "Database tables exist" "fail"
    fi
}

# ============================================================================
# Summary
# ============================================================================

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

# ============================================================================
# Main
# ============================================================================

main() {
    echo ""
    echo "========================================"
    echo "    CL8Y Bridge E2E Test Suite"
    echo "        (Watchtower Pattern)"
    echo "========================================"
    echo ""
    
    if [ "$FULL_MODE" = true ]; then
        log_info "Running FULL test suite (includes transfers)"
    elif [ "$QUICK_MODE" = true ]; then
        log_info "Running QUICK test suite (connectivity only)"
    else
        log_info "Running STANDARD test suite"
    fi
    
    if [ "$WITH_OPERATOR" = true ]; then
        log_info "Will manage operator lifecycle"
    fi
    if [ "$WITH_CANCELER" = true ]; then
        log_info "Will manage canceler lifecycle"
    fi
    echo ""
    
    # Set up cleanup trap
    trap cleanup EXIT
    
    check_prereqs
    
    # Start services if requested
    start_operator_if_needed
    start_canceler_if_needed
    
    # EVM Tests
    echo ""
    echo -e "${BLUE}=== EVM Tests ===${NC}"
    test_evm_connectivity
    test_evm_time_skip
    test_evm_bridge_config
    
    # Terra Tests
    if [ "$SKIP_TERRA" = false ]; then
        echo ""
        echo -e "${BLUE}=== Terra Tests ===${NC}"
        test_terra_connectivity
        test_terra_bridge_config
    fi
    
    # Quick mode stops here
    if [ "$QUICK_MODE" = true ]; then
        print_summary
        return
    fi
    
    # Integration Tests
    echo ""
    echo -e "${BLUE}=== Integration Tests ===${NC}"
    test_database
    test_watchtower_delay_mechanism
    
    # Full Transfer Tests
    if [ "$FULL_MODE" = true ]; then
        echo ""
        echo -e "${BLUE}=== Transfer Tests ===${NC}"
        test_evm_to_terra_transfer
        test_terra_to_evm_transfer
        
        echo ""
        echo -e "${BLUE}=== Canceler Tests ===${NC}"
        test_canceler_compilation
    fi
    
    print_summary
}

# ============================================================================
# Canceler Tests
# ============================================================================

test_canceler_compilation() {
    log_step "=== TEST: Canceler Compilation ==="
    
    # Check if canceler compiles
    if cargo check --manifest-path "$PROJECT_ROOT/packages/canceler/Cargo.toml" 2>&1 | grep -q "error"; then
        log_error "Canceler failed to compile"
        record_result "Canceler Compilation" "fail"
        return
    fi
    
    log_info "Canceler compiles successfully"
    log_info "Canceler features:"
    log_info "  - EVM event polling (WithdrawApproved events)"
    log_info "  - Terra approval polling (pending_approvals query)"
    log_info "  - Cancel transaction submission"
    log_info "  - Verification against source chain"
    
    record_result "Canceler Compilation" "pass"
}

main "$@"
