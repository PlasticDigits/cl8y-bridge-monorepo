#!/usr/bin/env bash
# Register MegaETH (evm_4326, bytes4 0x000010e6) on an EVM ChainRegistry.
# Mirror pattern: scripts/solana/register-chain-evm.sh
#
# Usage:
#   source <(./scripts/megaeth/compute-megaeth-constants.sh)
#   export CHAIN_REGISTRY_ADDRESS=0x...   # BSC or opBNB ChainRegistry proxy
#   export EVM_RPC_URL=https://...
#   ./scripts/megaeth/register-megaeth-on-chain-registry.sh
#
# Signing: cast prompts for owner key (--interactive). Requires /dev/tty.

set -euo pipefail

if [[ ! -c /dev/tty ]] || ! [[ -r /dev/tty && -w /dev/tty ]]; then
  echo "A controlling terminal (/dev/tty) is required. Run from a real terminal." >&2
  exit 1
fi

CHAIN_REGISTRY_ADDRESS="${CHAIN_REGISTRY_ADDRESS:?set CHAIN_REGISTRY_ADDRESS}"
EVM_RPC_URL="${EVM_RPC_URL:?set EVM_RPC_URL}"

if [[ -z "${MEGAETH_IDENTIFIER:-}" ]] || [[ -z "${MEGAETH_V2_BYTES4:-}" ]]; then
  echo "Run: source <(./scripts/megaeth/compute-megaeth-constants.sh)" >&2
  exit 1
fi

echo "Registering MegaETH on ChainRegistry"
echo "  Identifier: $MEGAETH_IDENTIFIER"
echo "  Bytes4:     $MEGAETH_V2_BYTES4"
echo "  Registry:   $CHAIN_REGISTRY_ADDRESS"
echo "  RPC:        $EVM_RPC_URL"

TX_HASH=$(
  cast send \
    --rpc-url "${EVM_RPC_URL}" \
    --interactive \
    "${CHAIN_REGISTRY_ADDRESS}" \
    "registerChain(string,bytes4)" \
    "${MEGAETH_IDENTIFIER}" \
    "${MEGAETH_V2_BYTES4}" </dev/tty
)

echo "  TX: ${TX_HASH}"

export CHAIN_REGISTRY="$CHAIN_REGISTRY_ADDRESS"
export RPC_URL="$EVM_RPC_URL"
export PEER_IDENTIFIER="$MEGAETH_IDENTIFIER"
export PEER_BYTES4="$MEGAETH_V2_BYTES4"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"${SCRIPT_DIR}/verify-evm-peers.sh"
