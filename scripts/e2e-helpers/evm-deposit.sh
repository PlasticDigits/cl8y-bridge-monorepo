#!/bin/bash
# Helper script to perform an EVM deposit
#
# Usage: ./evm-deposit.sh <token> <amount> <dest_chain_key> <dest_account>

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

TOKEN="${1:-}"
AMOUNT="${2:-}"
DEST_CHAIN_KEY="${3:-}"
DEST_ACCOUNT="${4:-}"

if [ -z "$TOKEN" ] || [ -z "$AMOUNT" ] || [ -z "$DEST_CHAIN_KEY" ] || [ -z "$DEST_ACCOUNT" ]; then
    echo "Usage: $0 <token> <amount> <dest_chain_key> <dest_account>"
    exit 1
fi

# Check required env vars
: "${EVM_RPC_URL:?EVM_RPC_URL is required}"
: "${EVM_ROUTER_ADDRESS:?EVM_ROUTER_ADDRESS is required}"
: "${EVM_PRIVATE_KEY:?EVM_PRIVATE_KEY is required}"
: "${LOCK_UNLOCK_ADDRESS:?LOCK_UNLOCK_ADDRESS is required}"

log_step "Performing EVM deposit..."
log_info "Token: $TOKEN"
log_info "Amount: $AMOUNT"
log_info "Dest Chain Key: $DEST_CHAIN_KEY"
log_info "Dest Account: $DEST_ACCOUNT"

# First approve the LockUnlock contract to spend tokens
# Note: Users must approve the downstream contract (MintBurn or LockUnlock), not the router
log_info "Approving LockUnlock to spend tokens..."
APPROVE_TX=$(cast send "$TOKEN" "approve(address,uint256)" \
    "$LOCK_UNLOCK_ADDRESS" \
    "$AMOUNT" \
    --rpc-url "$EVM_RPC_URL" \
    --private-key "$EVM_PRIVATE_KEY" \
    --json 2>&1)

if ! echo "$APPROVE_TX" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
    log_error "Approval failed: $APPROVE_TX"
    exit 1
fi
log_info "Approval successful"

# Perform the deposit
log_info "Depositing tokens..."
DEPOSIT_TX=$(cast send "$EVM_ROUTER_ADDRESS" \
    "deposit(address,uint256,bytes32,bytes32)" \
    "$TOKEN" \
    "$AMOUNT" \
    "$DEST_CHAIN_KEY" \
    "$DEST_ACCOUNT" \
    --rpc-url "$EVM_RPC_URL" \
    --private-key "$EVM_PRIVATE_KEY" \
    --json 2>&1)

if echo "$DEPOSIT_TX" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
    TX_HASH=$(echo "$DEPOSIT_TX" | jq -r '.transactionHash')
    log_pass "Deposit successful!"
    log_info "Transaction hash: $TX_HASH"
    echo "$TX_HASH"
else
    log_error "Deposit failed: $DEPOSIT_TX"
    exit 1
fi
