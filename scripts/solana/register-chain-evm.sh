#!/usr/bin/env bash
# Register Solana chain on EVM ChainRegistry
#
# Usage: ./scripts/solana/register-chain-evm.sh
#
# Env vars:
#   EVM_RPC_URL            - RPC endpoint (default: http://localhost:8545)
#   PRIVATE_KEY            - Admin private key (hex, with 0x prefix)
#   CHAIN_REGISTRY_ADDRESS - ChainRegistry contract address

set -euo pipefail

EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
PRIVATE_KEY="${PRIVATE_KEY:?PRIVATE_KEY env var is required}"
CHAIN_REGISTRY_ADDRESS="${CHAIN_REGISTRY_ADDRESS:?CHAIN_REGISTRY_ADDRESS env var is required}"

SOLANA_CHAIN_ID="0x00000005"
SOLANA_IDENTIFIER="solana_mainnet-beta"

echo "Registering Solana chain on EVM ChainRegistry..."
echo "  Chain ID: ${SOLANA_CHAIN_ID}"
echo "  Identifier: ${SOLANA_IDENTIFIER}"
echo "  Registry: ${CHAIN_REGISTRY_ADDRESS}"

TX_HASH=$(cast send \
  --rpc-url "${EVM_RPC_URL}" \
  --private-key "${PRIVATE_KEY}" \
  "${CHAIN_REGISTRY_ADDRESS}" \
  "registerChain(string,bytes4)" \
  "${SOLANA_IDENTIFIER}" \
  "${SOLANA_CHAIN_ID}")

echo "  TX: ${TX_HASH}"

REGISTERED_ID=$(cast call \
  --rpc-url "${EVM_RPC_URL}" \
  "${CHAIN_REGISTRY_ADDRESS}" \
  "getChainId(string)(bytes4)" \
  "${SOLANA_IDENTIFIER}")

echo "  Registered chain ID: ${REGISTERED_ID}"
echo "Solana chain registered on EVM!"
