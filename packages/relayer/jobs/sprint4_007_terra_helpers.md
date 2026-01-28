---
output_dir: ../../../scripts/lib/
output_file: terra-helpers.sh
---

# Terra CLI Helper Functions

Create a shell library with helper functions for terrad CLI operations in E2E tests.

## Requirements

Create reusable functions for common Terra operations that can be sourced by other scripts.

## Functions to Implement

```bash
#!/bin/bash
# Terra CLI Helper Functions for E2E Testing
#
# Source this file in your test scripts:
#   source scripts/lib/terra-helpers.sh
#
# Required environment variables:
#   TERRA_RPC_URL - Terra RPC endpoint (e.g., http://localhost:26657)
#   TERRA_LCD_URL - Terra LCD endpoint (e.g., http://localhost:1317)
#   TERRA_CHAIN_ID - Chain ID (e.g., localterra)
#   TERRA_KEY_NAME - Key name in terrad keyring (e.g., test1)
#   TERRA_BRIDGE_ADDRESS - Bridge contract address

# Query Terra balance for an address
# Usage: terra_balance <address> <denom>
# Returns: Amount as string, or "0" if not found
terra_balance() {
    local address=$1
    local denom=${2:-uluna}
    
    curl -s "${TERRA_LCD_URL}/cosmos/bank/v1beta1/balances/${address}" | \
        jq -r ".balances[] | select(.denom==\"${denom}\") | .amount" 2>/dev/null || echo "0"
}

# Execute lock on Terra bridge
# Usage: terra_lock <amount> <denom> <dest_chain_id> <recipient>
# Returns: JSON result from terrad
terra_lock() {
    local amount=$1
    local denom=${2:-uluna}
    local dest_chain=$3
    local recipient=$4
    
    local msg="{\"lock\":{\"dest_chain_id\":${dest_chain},\"recipient\":\"${recipient}\"}}"
    
    terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
        "$msg" \
        --amount "${amount}${denom}" \
        --from "$TERRA_KEY_NAME" \
        --node "$TERRA_RPC_URL" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.4 \
        --fees 10000uluna \
        --yes --output json 2>&1
}

# Wait for a Terra transaction to be confirmed
# Usage: terra_wait_tx <tx_hash> [timeout_seconds]
# Returns: Transaction result JSON, or exits with error
terra_wait_tx() {
    local tx_hash=$1
    local timeout=${2:-60}
    local elapsed=0
    
    while [ $elapsed -lt $timeout ]; do
        result=$(terrad query tx "$tx_hash" --node "$TERRA_RPC_URL" --output json 2>/dev/null)
        if [ $? -eq 0 ] && [ -n "$result" ]; then
            echo "$result"
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done
    
    echo "Timeout waiting for tx: $tx_hash" >&2
    return 1
}

# Get the current Terra block height
# Usage: terra_height
# Returns: Block height as integer
terra_height() {
    curl -s "${TERRA_LCD_URL}/cosmos/base/tendermint/v1beta1/blocks/latest" | \
        jq -r '.block.header.height' 2>/dev/null || echo "0"
}

# Query bridge contract state
# Usage: terra_bridge_query <query_msg_json>
# Returns: Query result JSON
terra_bridge_query() {
    local query_msg=$1
    local query_base64=$(echo -n "$query_msg" | base64 -w0)
    
    curl -s "${TERRA_LCD_URL}/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE_ADDRESS}/smart/${query_base64}" | \
        jq -r '.data' 2>/dev/null
}

# Get bridge nonce
# Usage: terra_bridge_nonce
# Returns: Current nonce as integer
terra_bridge_nonce() {
    terra_bridge_query '{"nonce":{}}' | jq -r '.nonce // 0' 2>/dev/null || echo "0"
}

# Check if terrad is available (local or docker)
# Returns: 0 if available, 1 otherwise
terra_check_cli() {
    if command -v terrad &> /dev/null; then
        return 0
    elif docker compose ps | grep -q "terrad-cli"; then
        # Use docker exec for terrad
        alias terrad='docker compose exec -T terrad-cli terrad'
        return 0
    fi
    return 1
}

# Initialize Terra test key if not exists
# Usage: terra_init_test_key
terra_init_test_key() {
    local key_name=${1:-test1}
    local mnemonic=${2:-"notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius"}
    
    # Check if key exists
    if terrad keys show "$key_name" --keyring-backend test &>/dev/null; then
        echo "Key $key_name already exists"
        return 0
    fi
    
    # Create key from mnemonic
    echo "$mnemonic" | terrad keys add "$key_name" --recover --keyring-backend test
}

# Get address for a key
# Usage: terra_key_address <key_name>
terra_key_address() {
    local key_name=${1:-$TERRA_KEY_NAME}
    terrad keys show "$key_name" --keyring-backend test -a 2>/dev/null
}

# Send uluna to an address
# Usage: terra_send <to_address> <amount> [denom]
terra_send() {
    local to=$1
    local amount=$2
    local denom=${3:-uluna}
    
    terrad tx bank send "$TERRA_KEY_NAME" "$to" "${amount}${denom}" \
        --from "$TERRA_KEY_NAME" \
        --node "$TERRA_RPC_URL" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.4 \
        --fees 10000uluna \
        --yes --output json 2>&1
}

# Wait for Terra to be ready
# Usage: terra_wait_ready [timeout_seconds]
terra_wait_ready() {
    local timeout=${1:-120}
    local elapsed=0
    
    echo "Waiting for Terra to be ready..."
    while [ $elapsed -lt $timeout ]; do
        if curl -s "${TERRA_RPC_URL}/status" | jq -e '.result.sync_info.catching_up == false' &>/dev/null; then
            echo "Terra is ready!"
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
        echo -n "."
    done
    
    echo ""
    echo "Timeout waiting for Terra to be ready" >&2
    return 1
}

# Export all functions
export -f terra_balance terra_lock terra_wait_tx terra_height
export -f terra_bridge_query terra_bridge_nonce terra_check_cli
export -f terra_init_test_key terra_key_address terra_send terra_wait_ready
```

## Notes

- Functions should be POSIX-compatible where possible
- Use sensible defaults for optional parameters
- Include error handling and timeouts
- Support both local terrad and Docker-based execution
