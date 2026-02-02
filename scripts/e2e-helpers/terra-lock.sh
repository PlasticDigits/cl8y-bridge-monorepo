#!/bin/bash
# Helper script to perform a Terra lock (deposit)
#
# Usage: ./terra-lock.sh <dest_chain_id> <recipient> <amount> <denom>

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

DEST_CHAIN_ID="${1:-}"
RECIPIENT="${2:-}"
AMOUNT="${3:-}"
DENOM="${4:-uluna}"

if [ -z "$DEST_CHAIN_ID" ] || [ -z "$RECIPIENT" ] || [ -z "$AMOUNT" ]; then
    echo "Usage: $0 <dest_chain_id> <recipient> <amount> [denom]"
    exit 1
fi

# Check required env vars
: "${TERRA_LCD_URL:?TERRA_LCD_URL is required}"
: "${TERRA_BRIDGE_ADDRESS:?TERRA_BRIDGE_ADDRESS is required}"

log_step "Performing Terra lock..."
log_info "Dest Chain ID: $DEST_CHAIN_ID"
log_info "Recipient: $RECIPIENT"
log_info "Amount: $AMOUNT $DENOM"

# Build the lock message
LOCK_MSG=$(cat <<EOF
{"lock":{"dest_chain_id":$DEST_CHAIN_ID,"recipient":"$RECIPIENT"}}
EOF
)

log_info "Lock message: $LOCK_MSG"

# Execute via terrad CLI in docker
RESULT=$(docker compose exec -T terrad-cli terrad tx wasm execute \
    "$TERRA_BRIDGE_ADDRESS" \
    "$LOCK_MSG" \
    --amount "${AMOUNT}${DENOM}" \
    --from test1 \
    --chain-id localterra \
    --node http://localterra:26657 \
    --gas auto \
    --gas-adjustment 1.5 \
    --broadcast-mode sync \
    -y --output json 2>&1)

TX_HASH=$(echo "$RESULT" | jq -r '.txhash // "unknown"' 2>/dev/null || echo "unknown")

if [ "$TX_HASH" != "unknown" ] && [ -n "$TX_HASH" ]; then
    log_pass "Lock transaction submitted!"
    log_info "Transaction hash: $TX_HASH"
    echo "$TX_HASH"
else
    log_error "Lock failed: $RESULT"
    exit 1
fi
