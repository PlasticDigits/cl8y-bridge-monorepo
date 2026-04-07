#!/usr/bin/env bash
# Register Solana chain on EVM ChainRegistry
#
# Usage: ./scripts/solana/register-chain-evm.sh
#
# Signing: `cast send --interactive` prompts for the ChainRegistry owner private key (not echoed).
# Do not put the key in environment variables (history, process listings).
#
# Env vars:
#   EVM_RPC_URL            - RPC endpoint (default: http://localhost:8545)
#   CHAIN_REGISTRY_ADDRESS - ChainRegistry proxy (same on BSC and opBNB for this deployment)
#
# For Ledger / keystore / AWS KMS, run `cast send` yourself with the same args, or wrap this script
# after editing the cast send line (see `cast send --help`).
#
# Run once per chain with the matching RPC, e.g. BSC then opBNB:
#   EVM_RPC_URL=https://bsc-dataseed1.binance.org CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e ./scripts/solana/register-chain-evm.sh
#   EVM_RPC_URL=https://opbnb-mainnet-rpc.bnbchain.org CHAIN_REGISTRY_ADDRESS=0x2e5d36c46680a38e7ae156fc9d109084c58c688e ./scripts/solana/register-chain-evm.sh

set -euo pipefail

EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
CHAIN_REGISTRY_ADDRESS="${CHAIN_REGISTRY_ADDRESS:?CHAIN_REGISTRY_ADDRESS env var is required}"

SOLANA_CHAIN_ID="0x00000005"
SOLANA_IDENTIFIER="solana_mainnet-beta"

if [[ ! -c /dev/tty ]] || ! [[ -r /dev/tty && -w /dev/tty ]]; then
  echo "A controlling terminal (/dev/tty) is required for interactive signing. Run from a real terminal (not piped stdin)." >&2
  exit 1
fi

echo "Registering Solana chain on EVM ChainRegistry..."
echo "  Chain ID: ${SOLANA_CHAIN_ID}"
echo "  Identifier: ${SOLANA_IDENTIFIER}"
echo "  Registry: ${CHAIN_REGISTRY_ADDRESS}"
echo "  RPC:      ${EVM_RPC_URL}"
echo "  Signing:  interactive — enter ChainRegistry owner private key when cast prompts (0x…)" >&2

TX_HASH=$(
  cast send \
    --rpc-url "${EVM_RPC_URL}" \
    --interactive \
    "${CHAIN_REGISTRY_ADDRESS}" \
    "registerChain(string,bytes4)" \
    "${SOLANA_IDENTIFIER}" \
    "${SOLANA_CHAIN_ID}" </dev/tty
)

echo "  TX: ${TX_HASH}"

# ChainRegistry has getChainIdFromHash(bytes32), not getChainId(string).
IDENTIFIER_HASH=$(cast call \
  --rpc-url "${EVM_RPC_URL}" \
  "${CHAIN_REGISTRY_ADDRESS}" \
  "computeIdentifierHash(string)(bytes32)" \
  "${SOLANA_IDENTIFIER}")

REGISTERED_ID=$(cast call \
  --rpc-url "${EVM_RPC_URL}" \
  "${CHAIN_REGISTRY_ADDRESS}" \
  "getChainIdFromHash(bytes32)(bytes4)" \
  "${IDENTIFIER_HASH}")

echo "  Identifier hash: ${IDENTIFIER_HASH}"
echo "  Mapped chain ID: ${REGISTERED_ID} (expect ${SOLANA_CHAIN_ID})"

REGISTERED_BOOL=$(cast call \
  --rpc-url "${EVM_RPC_URL}" \
  "${CHAIN_REGISTRY_ADDRESS}" \
  "isChainRegistered(bytes4)(bool)" \
  "${SOLANA_CHAIN_ID}")

echo "  isChainRegistered(${SOLANA_CHAIN_ID}): ${REGISTERED_BOOL}"
echo "Solana chain registered on EVM!"
