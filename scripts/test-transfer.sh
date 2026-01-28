#!/bin/bash
set -e

echo "=== CL8Y Bridge Test Transfer ==="

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_RPC_URL="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_LCD_URL="${TERRA_LCD_URL:-http://localhost:1317}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"

# Addresses (set these from deployment output)
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
EVM_ROUTER_ADDRESS="${EVM_ROUTER_ADDRESS:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"

# Test accounts
EVM_TEST_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
EVM_TEST_ADDRESS="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
TERRA_TEST_ADDRESS="terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"

echo ""
echo "=== Test 1: EVM to Terra Classic ==="

# Get initial balances
echo "Checking initial balances..."

# EVM balance
# EVM_BALANCE=$(cast balance "$EVM_TEST_ADDRESS" --rpc-url "$EVM_RPC_URL")
# echo "EVM Test Account: $EVM_BALANCE wei"

# Terra balance
# TERRA_BALANCE=$(curl -s "$TERRA_LCD_URL/cosmos/bank/v1beta1/balances/$TERRA_TEST_ADDRESS" | jq -r '.balances[] | select(.denom=="uluna") | .amount')
# echo "Terra Test Account: $TERRA_BALANCE uluna"

echo ""
echo "Initiating deposit on EVM..."

# Compute Terra chain key and recipient bytes32
# TERRA_CHAIN_KEY="0x..."
# TERRA_RECIPIENT_BYTES32="0x..."

# Deposit native ETH
# cast send "$EVM_ROUTER_ADDRESS" "depositNative(bytes32,bytes32)" \
#     "$TERRA_CHAIN_KEY" \
#     "$TERRA_RECIPIENT_BYTES32" \
#     --value 1ether \
#     --rpc-url "$EVM_RPC_URL" \
#     --private-key "$EVM_TEST_PRIVATE_KEY"

echo "Deposit submitted. Waiting for relayer..."
sleep 10

# Check Terra balance increased
echo "Checking Terra balance after transfer..."
# NEW_TERRA_BALANCE=$(curl -s "$TERRA_LCD_URL/cosmos/bank/v1beta1/balances/$TERRA_TEST_ADDRESS" | jq -r '.balances[] | select(.denom=="uluna") | .amount')
# echo "Terra Test Account: $NEW_TERRA_BALANCE uluna"

echo ""
echo "=== Test 2: Terra Classic to EVM ==="

echo ""
echo "Initiating lock on Terra..."

# Lock LUNC for bridging to EVM
# terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
#     "{\"lock\":{\"dest_chain_id\":31337,\"recipient\":\"$EVM_TEST_ADDRESS\"}}" \
#     --amount 1000000uluna \
#     --from test1 \
#     --gas auto --gas-adjustment 1.3 \
#     --node "$TERRA_RPC_URL" \
#     --chain-id "$TERRA_CHAIN_ID" \
#     -y

echo "Lock submitted. Waiting for relayer approval..."
sleep 10

# Check approval status
echo "Checking approval status on EVM..."
# APPROVAL=$(cast call "$EVM_BRIDGE_ADDRESS" "getWithdrawApproval(bytes32)" "$WITHDRAW_HASH" --rpc-url "$EVM_RPC_URL")
# echo "Approval: $APPROVAL"

echo ""
echo "Waiting for withdrawal delay..."
sleep 5

echo "Executing withdrawal..."
# cast send "$EVM_ROUTER_ADDRESS" "withdraw(bytes32,address,address,uint256,uint256)" \
#     "$TERRA_CHAIN_KEY" \
#     "$TOKEN_ADDRESS" \
#     "$EVM_TEST_ADDRESS" \
#     "$AMOUNT" \
#     "$NONCE" \
#     --value 0.001ether \
#     --rpc-url "$EVM_RPC_URL" \
#     --private-key "$EVM_TEST_PRIVATE_KEY"

echo ""
echo "=== Test Transfer Complete ==="
echo ""
echo "Note: This is a template script. Update addresses and parameters"
echo "based on your actual deployment."
