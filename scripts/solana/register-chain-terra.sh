#!/usr/bin/env bash
# Register Solana chain on Terra Classic bridge
#
# Usage: ./scripts/solana/register-chain-terra.sh
#
# Env vars:
#   TERRA_NODE_URL - Terra RPC endpoint
#   TERRA_WALLET - Wallet name
#   BRIDGE_CONTRACT - Bridge contract address

set -euo pipefail

TERRA_NODE_URL="${TERRA_NODE_URL:-http://localhost:1317}"
BRIDGE_CONTRACT="${BRIDGE_CONTRACT:?BRIDGE_CONTRACT is required}"
TERRA_WALLET="${TERRA_WALLET:-validator}"

SOLANA_CHAIN_ID="AAAABQ==" # base64 of [0,0,0,5]
SOLANA_IDENTIFIER="solana_mainnet-beta"

echo "Registering Solana chain on Terra bridge..."
echo "  Contract: ${BRIDGE_CONTRACT}"
echo "  Chain ID: ${SOLANA_CHAIN_ID}"

terrad tx wasm execute "${BRIDGE_CONTRACT}" "{
  \"register_chain\": {
    \"chain_id\": \"${SOLANA_CHAIN_ID}\",
    \"identifier\": \"${SOLANA_IDENTIFIER}\"
  }
}" --from "${TERRA_WALLET}" --node "${TERRA_NODE_URL}" --chain-id localterra --gas auto --gas-adjustment 1.4 -y

echo "Done!"
