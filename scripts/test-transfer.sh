#!/bin/bash
# Test cross-chain transfers
#
# This script performs test transfers in both directions:
# 1. Terra -> EVM (lock on Terra, approve on EVM)
# 2. EVM -> Terra (deposit on EVM, release on Terra)
#
# Prerequisites:
# - Both bridges deployed and configured
# - Relayer running (or run manually after lock/deposit)
#
# Usage:
#   ./scripts/test-transfer.sh

set -e

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_NODE="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_LCD="${TERRA_LCD_URL:-http://localhost:1317}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"

# Contract addresses
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
EVM_ROUTER_ADDRESS="${EVM_ROUTER_ADDRESS:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"

# Test accounts
EVM_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
EVM_TEST_ADDRESS="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
TERRA_KEY="${TERRA_KEY_NAME:-test1}"
TERRA_TEST_ADDRESS="terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"

# Transfer amounts
TERRA_TRANSFER_AMOUNT="1000000"  # 1 LUNA (in uluna)
EVM_TRANSFER_AMOUNT="1000000000000000000"  # 1 token (18 decimals)

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

# Check prerequisites
check_prereqs() {
    log_info "Checking prerequisites..."
    
    if [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_error "TERRA_BRIDGE_ADDRESS not set"
        exit 1
    fi
    
    # Check EVM RPC
    if ! cast block-number --rpc-url "$EVM_RPC_URL" > /dev/null 2>&1; then
        log_error "Cannot connect to EVM at $EVM_RPC_URL"
        exit 1
    fi
    
    # Check Terra RPC
    if ! curl -s "$TERRA_NODE/status" > /dev/null 2>&1; then
        log_error "Cannot connect to Terra at $TERRA_NODE"
        exit 1
    fi
    
    log_info "Prerequisites OK"
}

# Get balances
get_balances() {
    log_info "Current balances:"
    
    # EVM balance
    EVM_BALANCE=$(cast balance "$EVM_TEST_ADDRESS" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0")
    echo "  EVM ($EVM_TEST_ADDRESS): $EVM_BALANCE wei"
    
    # Terra balance
    TERRA_BALANCE=$(curl -s "$TERRA_LCD/cosmos/bank/v1beta1/balances/$TERRA_TEST_ADDRESS" 2>/dev/null | \
        jq -r '.balances[] | select(.denom=="uluna") | .amount' 2>/dev/null || echo "0")
    echo "  Terra ($TERRA_TEST_ADDRESS): ${TERRA_BALANCE:-0} uluna"
}

# Test 1: Terra -> EVM
test_terra_to_evm() {
    log_step "=== Test: Terra -> EVM Transfer ==="
    
    get_balances
    echo ""
    
    log_info "Locking $TERRA_TRANSFER_AMOUNT uluna on Terra..."
    
    # Lock tokens on Terra
    LOCK_MSG="{\"lock\":{\"dest_chain_id\":31337,\"recipient\":\"$EVM_TEST_ADDRESS\"}}"
    
    TX_RESULT=$(terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
        "$LOCK_MSG" \
        --amount "${TERRA_TRANSFER_AMOUNT}uluna" \
        --from "$TERRA_KEY" \
        --chain-id "$TERRA_CHAIN_ID" \
        --node "$TERRA_NODE" \
        --gas auto --gas-adjustment 1.4 \
        --fees 5000uluna \
        -y -o json)
    
    TX_HASH=$(echo "$TX_RESULT" | jq -r '.txhash')
    log_info "Lock TX: $TX_HASH"
    
    log_info "Waiting for confirmation..."
    sleep 6
    
    # Query transaction
    TX_QUERY=$(terrad query tx "$TX_HASH" --node "$TERRA_NODE" -o json 2>/dev/null || echo "{}")
    
    # Extract wasm events
    WASM_EVENTS=$(echo "$TX_QUERY" | jq -r '.logs[0].events[] | select(.type=="wasm")' 2>/dev/null || echo "")
    
    if [ -n "$WASM_EVENTS" ]; then
        log_info "Lock event detected!"
        echo "$WASM_EVENTS" | jq '.'
    else
        log_warn "Could not parse lock event"
    fi
    
    echo ""
    log_info "Lock complete. Waiting for relayer to process..."
    log_info "Run 'make relayer' in another terminal to process the transfer."
    echo ""
    
    # Wait for relayer (optional - adjust based on setup)
    read -p "Press Enter after relayer has processed (or Ctrl+C to skip)..." </dev/tty || true
    
    get_balances
}

# Test 2: EVM -> Terra
test_evm_to_terra() {
    log_step "=== Test: EVM -> Terra Transfer ==="
    
    get_balances
    echo ""
    
    # Compute Terra chain key
    TERRA_CHAIN_KEY=$(cast keccak "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'localterra' 'terra')")
    log_info "Terra Chain Key: $TERRA_CHAIN_KEY"
    
    # Encode Terra address as bytes32 (left-padded)
    # For simplicity, we'll use a hex representation
    TERRA_RECIPIENT_HEX="0x$(echo -n "$TERRA_TEST_ADDRESS" | xxd -p | tr -d '\n' | head -c 64)"
    # Pad to 32 bytes if needed
    while [ ${#TERRA_RECIPIENT_HEX} -lt 66 ]; do
        TERRA_RECIPIENT_HEX="${TERRA_RECIPIENT_HEX}00"
    done
    
    log_info "Encoded recipient: $TERRA_RECIPIENT_HEX"
    
    # Use bridge address if router not set
    ROUTER="${EVM_ROUTER_ADDRESS:-$EVM_BRIDGE_ADDRESS}"
    
    if [ -z "$ROUTER" ]; then
        log_error "Neither EVM_ROUTER_ADDRESS nor EVM_BRIDGE_ADDRESS set"
        exit 1
    fi
    
    log_info "Depositing on EVM via $ROUTER..."
    
    # Try deposit (this may fail if interface doesn't match - that's OK for testing)
    cast send "$ROUTER" \
        "deposit(address,uint256,bytes32,bytes32)" \
        "0x0000000000000000000000000000000000001234" \
        "$EVM_TRANSFER_AMOUNT" \
        "$TERRA_CHAIN_KEY" \
        "$TERRA_RECIPIENT_HEX" \
        --rpc-url "$EVM_RPC_URL" \
        --private-key "$EVM_PRIVATE_KEY" \
        --value 0 \
        2>&1 || log_warn "Deposit may have failed - check contract interface"
    
    echo ""
    log_info "Deposit submitted. Waiting for relayer to process..."
    log_info "Run 'make relayer' in another terminal to process the transfer."
    echo ""
    
    read -p "Press Enter after relayer has processed (or Ctrl+C to skip)..." </dev/tty || true
    
    get_balances
}

# Main menu
main() {
    log_info "=== CL8Y Bridge Test Transfers ==="
    echo ""
    
    check_prereqs
    
    echo ""
    echo "Select test to run:"
    echo "  1) Terra -> EVM (lock on Terra)"
    echo "  2) EVM -> Terra (deposit on EVM)"
    echo "  3) Run both tests"
    echo "  4) Show balances only"
    echo ""
    read -p "Choice [1-4]: " choice </dev/tty
    
    case "$choice" in
        1)
            test_terra_to_evm
            ;;
        2)
            test_evm_to_terra
            ;;
        3)
            test_terra_to_evm
            echo ""
            test_evm_to_terra
            ;;
        4)
            get_balances
            ;;
        *)
            log_error "Invalid choice"
            exit 1
            ;;
    esac
    
    echo ""
    log_info "=== Test Complete ==="
}

main "$@"
