#!/bin/bash
# Grant OPERATOR_ROLE to test account for E2E testing
#
# This allows the test account to call approveWithdraw() which is
# required for testing the watchtower pattern and canceler E2E tests.
#
# Usage: ./grant-operator-role.sh [account]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Default test account (Anvil account 0)
ACCOUNT="${1:-0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266}"

: "${EVM_RPC_URL:?EVM_RPC_URL is required}"
: "${ACCESS_MANAGER_ADDRESS:?ACCESS_MANAGER_ADDRESS is required}"
: "${EVM_PRIVATE_KEY:?EVM_PRIVATE_KEY is required}"

log_step "Granting OPERATOR_ROLE to $ACCOUNT..."

# The AccessManagerEnumerable uses role IDs
# OPERATOR_ROLE is typically role ID 1 (ADMIN_ROLE is 0)
OPERATOR_ROLE_ID=1

# Check current status
log_info "Checking current role status..."

CURRENT_ROLE=$(cast call "$ACCESS_MANAGER_ADDRESS" \
    "hasRole(uint64,address)(bool,uint32)" \
    "$OPERATOR_ROLE_ID" \
    "$ACCOUNT" \
    --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "false")

if echo "$CURRENT_ROLE" | grep -q "true"; then
    log_info "Account already has OPERATOR_ROLE"
    exit 0
fi

# Grant the role
# grantRole(uint64 roleId, address account, uint32 executionDelay)
log_info "Granting role..."
GRANT_TX=$(cast send "$ACCESS_MANAGER_ADDRESS" \
    "grantRole(uint64,address,uint32)" \
    "$OPERATOR_ROLE_ID" \
    "$ACCOUNT" \
    "0" \
    --rpc-url "$EVM_RPC_URL" \
    --private-key "$EVM_PRIVATE_KEY" \
    --json 2>&1)

if echo "$GRANT_TX" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
    log_pass "OPERATOR_ROLE granted successfully"
else
    log_error "Failed to grant role: $GRANT_TX"
    exit 1
fi

# Verify
NEW_STATUS=$(cast call "$ACCESS_MANAGER_ADDRESS" \
    "hasRole(uint64,address)(bool,uint32)" \
    "$OPERATOR_ROLE_ID" \
    "$ACCOUNT" \
    --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "")

log_info "New role status: $NEW_STATUS"
