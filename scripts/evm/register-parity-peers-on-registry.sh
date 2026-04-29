#!/usr/bin/env bash
# Register BSC, Terra Classic, and Solana on a **new chain's** ChainRegistry (GL-122).
# Uses production-identical (identifier, bytes4) pairs aligned with scripts/deploy-evm-full.sh Phase 6 + Solana helper.
#
# Usage:
#   export RPC_URL=https://...
#   export CHAIN_REGISTRY_ADDRESS=0x...   # proxy from parity broadcast / runbook
#   ./scripts/evm/register-parity-peers-on-registry.sh
#
# Signing: three interactive `cast send` prompts (ChainRegistry owner), unless overridden below.
#
# Overrides (optional):
#   PEER_BSC_IDENTIFIER PEER_BSC_BYTES4
#   PEER_TERRA_IDENTIFIER PEER_TERRA_BYTES4
#   PEER_SOLANA_IDENTIFIER PEER_SOLANA_BYTES4
#
set -euo pipefail

RPC_URL="${RPC_URL:?set RPC_URL}"
CHAIN_REGISTRY="${CHAIN_REGISTRY_ADDRESS:?set CHAIN_REGISTRY_ADDRESS}"

PEER_BSC_IDENTIFIER="${PEER_BSC_IDENTIFIER:-evm_56}"
PEER_BSC_BYTES4="${PEER_BSC_BYTES4:-0x00000038}"

PEER_TERRA_IDENTIFIER="${PEER_TERRA_IDENTIFIER:-terraclassic_columbus-5}"
PEER_TERRA_BYTES4="${PEER_TERRA_BYTES4:-0x00000001}"

PEER_SOLANA_IDENTIFIER="${PEER_SOLANA_IDENTIFIER:-solana_mainnet-beta}"
PEER_SOLANA_BYTES4="${PEER_SOLANA_BYTES4:-0x00000005}"

SIG='registerChain(string,bytes4)'

if [[ ! -c /dev/tty ]] || ! [[ -r /dev/tty && -w /dev/tty ]]; then
  echo "Interactive signing requires a TTY (/dev/tty); run from a real terminal." >&2
  exit 1
fi

register_one() {
  local label="$1" ident="$2" b4="$3"
  echo ""
  echo ">>> Register peer: $label"
  echo "    identifier=$ident bytes4=$b4"
  cast send --interactive --rpc-url "$RPC_URL" "$CHAIN_REGISTRY" "$SIG" "$ident" "$b4" </dev/tty
}

echo "=== ChainRegistry peer registration (GL-122) ==="
echo "RPC_URL=$RPC_URL"
echo "CHAIN_REGISTRY=$CHAIN_REGISTRY"

register_one "BSC mainnet-equivalent" "$PEER_BSC_IDENTIFIER" "$PEER_BSC_BYTES4"
register_one "Terra Classic" "$PEER_TERRA_IDENTIFIER" "$PEER_TERRA_BYTES4"
register_one "Solana mainnet-beta" "$PEER_SOLANA_IDENTIFIER" "$PEER_SOLANA_BYTES4"

echo ""
echo "Done — verify with \`cast call \"$CHAIN_REGISTRY\" \"isChainRegistered(bytes4)(bool)\" <bytes4>\` per peer."
