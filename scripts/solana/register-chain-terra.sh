#!/usr/bin/env bash
# Register Solana chain on Terra Classic bridge
#
# Usage: ./scripts/solana/register-chain-terra.sh
#
# Env vars:
#   TERRA_NODE_URL - Tendermint RPC URL for terrad tx (NOT LCD REST). Default: TERRA_RPC_URL or http://localhost:26657
#   TERRA_RPC_URL  - Used if TERRA_NODE_URL is unset (same convention as scripts/qa/qa-host.env)
#   TERRA_CHAIN_ID - Chain ID (default: localterra; use columbus-5 for Terra Classic mainnet)
#   TERRA_WALLET   - terrad key name (--from). Mainnet: must be bridge admin (e.g. cl8y2_admin)
#   BRIDGE_CONTRACT - Bridge contract address
#   TERRA_KEYRING_BACKEND - Optional; pass through to terrad (must match `terrad keys list`)
#   TERRA_FEES     - Optional; fixed fee (e.g. 5000000uluna). Mutually exclusive with gas prices.
#   TERRA_GAS_PRICES - Optional; e.g. 28.325uluna for --gas auto fee simulation (see contracts-terraclassic/scripts/deploy.sh)
#   On columbus-5 / rebel-2, if neither TERRA_FEES nor TERRA_GAS_PRICES is set, defaults to --gas-prices 28.325uluna

set -euo pipefail

TERRA_NODE_URL="${TERRA_NODE_URL:-${TERRA_RPC_URL:-http://localhost:26657}}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"
BRIDGE_CONTRACT="${BRIDGE_CONTRACT:?BRIDGE_CONTRACT is required}"
TERRA_WALLET="${TERRA_WALLET:-validator}"

if [[ "${TERRA_CHAIN_ID}" == "columbus-5" && "${TERRA_WALLET}" == "validator" ]]; then
  echo "error: TERRA_WALLET is still 'validator' on columbus-5; set TERRA_WALLET to your bridge admin key (e.g. cl8y2_admin)" >&2
  exit 1
fi

SOLANA_CHAIN_ID="AAAABQ==" # base64 of [0,0,0,5]
SOLANA_IDENTIFIER="solana_mainnet-beta"

TX_EXTRA=()
if [[ -n "${TERRA_KEYRING_BACKEND:-}" ]]; then
  TX_EXTRA+=(--keyring-backend "${TERRA_KEYRING_BACKEND}")
fi
if [[ -n "${TERRA_FEES:-}" ]]; then
  TX_EXTRA+=(--fees "${TERRA_FEES}")
elif [[ -n "${TERRA_GAS_PRICES:-}" ]]; then
  TX_EXTRA+=(--gas-prices "${TERRA_GAS_PRICES}")
elif [[ "${TERRA_CHAIN_ID}" == "columbus-5" || "${TERRA_CHAIN_ID}" == "rebel-2" ]]; then
  TX_EXTRA+=(--gas-prices "${TERRA_GAS_PRICES_DEFAULT:-28.325uluna}")
fi

echo "Registering Solana chain on Terra bridge..."
echo "  Contract: ${BRIDGE_CONTRACT}"
echo "  Node (RPC): ${TERRA_NODE_URL}"
echo "  Chain ID (bytes): ${SOLANA_CHAIN_ID}"
echo "  Identifier: ${SOLANA_IDENTIFIER}"

MSG="{
  \"register_chain\": {
    \"chain_id\": \"${SOLANA_CHAIN_ID}\",
    \"identifier\": \"${SOLANA_IDENTIFIER}\"
  }
}"

TX_JSON=$(terrad tx wasm execute "${BRIDGE_CONTRACT}" "${MSG}" \
  --from "${TERRA_WALLET}" --node "${TERRA_NODE_URL}" --chain-id "${TERRA_CHAIN_ID}" \
  --gas auto --gas-adjustment 1.4 "${TX_EXTRA[@]}" -y -o json) || {
  echo "${TX_JSON:-terrad failed}" >&2
  exit 1
}

TX_CODE=$(echo "$TX_JSON" | jq -r '.code // empty')
if [[ -z "$TX_CODE" || "$TX_CODE" == "null" ]]; then
  echo "error: could not parse terrad JSON output (install jq?)" >&2
  echo "$TX_JSON" >&2
  exit 1
fi
if [[ "$TX_CODE" != "0" ]]; then
  echo "error: transaction failed (code=${TX_CODE})" >&2
  echo "$TX_JSON" | jq . >&2 2>/dev/null || echo "$TX_JSON" >&2
  exit 1
fi

echo "$TX_JSON" | jq .
echo "Done!"
