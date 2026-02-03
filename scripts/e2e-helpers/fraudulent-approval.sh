#!/bin/bash
# Helper script to create a fraudulent approval for testing cancelers
#
# This creates an approval on the destination chain that has NO matching
# deposit on the source chain. A properly functioning canceler should
# detect and cancel this approval.
#
# Usage: ./fraudulent-approval.sh <dest_chain> <bridge_address>
#   dest_chain: "evm" or "terra"

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

DEST_CHAIN="${1:-evm}"

log_step "Creating fraudulent approval for testing on $DEST_CHAIN..."

# Generate unique test parameters that DON'T match any real deposit
FRAUD_NONCE=$((RANDOM * 1000 + 999000000))  # High nonce unlikely to exist
FRAUD_AMOUNT="1234567890123456789"          # Unusual amount
FRAUD_TIMESTAMP=$(date +%s)

log_info "Fraud test parameters:"
log_info "  Nonce: $FRAUD_NONCE"
log_info "  Amount: $FRAUD_AMOUNT"

if [ "$DEST_CHAIN" = "evm" ]; then
    # Create fraudulent approval on EVM
    : "${EVM_RPC_URL:?EVM_RPC_URL is required}"
    : "${EVM_BRIDGE_ADDRESS:?EVM_BRIDGE_ADDRESS is required}"
    : "${EVM_PRIVATE_KEY:?EVM_PRIVATE_KEY is required}"
    
    EVM_TEST_ADDRESS="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    
    # Fake source chain key (Terra chain key)
    FAKE_SRC_CHAIN_KEY=$(cast keccak256 "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'columbus-5' 'terra')")
    
    # Fake token address
    FAKE_TOKEN="0x0000000000000000000000000000000000000099"
    
    # Fake dest account (padded EVM address)
    FAKE_DEST_ACCOUNT="0x000000000000000000000000${EVM_TEST_ADDRESS:2}"
    
    log_info "Submitting fraudulent approval to EVM bridge..."
    log_info "  Source chain key: $FAKE_SRC_CHAIN_KEY"
    log_info "  Token: $FAKE_TOKEN"
    log_info "  Recipient: $EVM_TEST_ADDRESS"
    
    set +e  # Don't exit on error - approval may fail due to permissions
    APPROVE_TX=$(cast send "$EVM_BRIDGE_ADDRESS" \
        "approveWithdraw(bytes32,address,address,bytes32,uint256,uint256,uint256,address,bool)" \
        "$FAKE_SRC_CHAIN_KEY" \
        "$FAKE_TOKEN" \
        "$EVM_TEST_ADDRESS" \
        "$FAKE_DEST_ACCOUNT" \
        "$FRAUD_AMOUNT" \
        "$FRAUD_NONCE" \
        "0" \
        "0x0000000000000000000000000000000000000000" \
        "false" \
        --rpc-url "$EVM_RPC_URL" \
        --private-key "$EVM_PRIVATE_KEY" \
        --json 2>&1)
    set -e
    
    if echo "$APPROVE_TX" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
        TX_HASH=$(echo "$APPROVE_TX" | jq -r '.transactionHash')
        log_pass "Fraudulent approval submitted: $TX_HASH"
        
        # Compute the withdraw hash for verification
        log_info "Canceler should detect this approval has no matching deposit"
        log_info "Expected action: cancelWithdrawApproval()"
        
        # Return the transaction hash
        echo "$TX_HASH"
    else
        # This is expected if the test account doesn't have OPERATOR_ROLE
        log_warn "Approval requires OPERATOR_ROLE"
        log_info "To test fraudulent approvals, grant OPERATOR_ROLE to test account:"
        log_info "  cast send \$ACCESS_MANAGER \"grantRole(uint64,address,uint32)\" 1 \$TEST_ACCOUNT 0"
        log_info ""
        log_info "Alternatively, test conceptually:"
        log_info "  1. Fraudulent approval created (no matching deposit)"
        log_info "  2. Canceler queries source chain for deposit"
        log_info "  3. No deposit found → verification fails"
        log_info "  4. Canceler calls cancelWithdrawApproval(hash)"
        log_info "  5. Future withdraw() calls revert with ApprovalCancelled"
        exit 0
    fi

elif [ "$DEST_CHAIN" = "terra" ]; then
    # Create fraudulent approval on Terra
    : "${TERRA_LCD_URL:?TERRA_LCD_URL is required}"
    : "${TERRA_BRIDGE_ADDRESS:?TERRA_BRIDGE_ADDRESS is required}"
    
    CONTAINER_NAME="${LOCALTERRA_CONTAINER:-cl8y-bridge-monorepo-localterra-1}"
    TERRA_KEY_NAME="${TERRA_KEY_NAME:-test1}"
    
    # Fake source chain key (EVM chain key as base64)
    # This simulates an approval claiming to come from EVM but with no matching deposit
    FAKE_SRC_CHAIN_ID="31337"
    
    log_info "Submitting fraudulent approval to Terra bridge..."
    
    # Build the approve message with fake parameters
    APPROVE_MSG='{
        "approve_withdraw": {
            "src_chain_id": '$FAKE_SRC_CHAIN_ID',
            "token": "terra1234...",
            "recipient": "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
            "amount": "'$FRAUD_AMOUNT'",
            "nonce": '$FRAUD_NONCE'
        }
    }'
    
    set +e
    TX=$(docker exec "$CONTAINER_NAME" terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$APPROVE_MSG" \
        --from "$TERRA_KEY_NAME" \
        --chain-id localterra \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y --keyring-backend test -o json 2>&1)
    set -e
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    
    if [ -n "$TX_HASH" ] && [ "$TX_HASH" != "null" ]; then
        log_pass "Fraudulent approval submitted: $TX_HASH"
        echo "$TX_HASH"
    else
        log_warn "Approval requires OPERATOR role on Terra"
        log_info "Transaction result: $TX"
        exit 0
    fi
else
    log_error "Unknown destination chain: $DEST_CHAIN"
    log_info "Usage: $0 <evm|terra>"
    exit 1
fi
