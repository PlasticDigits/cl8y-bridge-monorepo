#!/usr/bin/env bash
# Verify ChainRegistry peer entries after registering MegaETH (or any evm_* peer).
#
# Usage:
#   export CHAIN_REGISTRY=0x...   # proxy on the chain you query
#   export RPC_URL=https://...
#   export PEER_IDENTIFIER=evm_4326
#   export PEER_BYTES4=0x000010e6
#   ./scripts/megaeth/verify-evm-peers.sh
#
# Requires: cast (Foundry)

set -euo pipefail

CHAIN_REGISTRY="${CHAIN_REGISTRY:?set CHAIN_REGISTRY}"
RPC_URL="${RPC_URL:?set RPC_URL}"
PEER_IDENTIFIER="${PEER_IDENTIFIER:?set PEER_IDENTIFIER}"
PEER_BYTES4="${PEER_BYTES4:?set PEER_BYTES4}"

echo "=== ChainRegistry peer verification ==="
echo "Registry: $CHAIN_REGISTRY"
echo "RPC:      $RPC_URL"
echo "Peer id:  $PEER_IDENTIFIER → $PEER_BYTES4"
echo ""

HASH=$(cast call --rpc-url "$RPC_URL" "$CHAIN_REGISTRY" \
  "computeIdentifierHash(string)(bytes32)" "$PEER_IDENTIFIER")
echo "computeIdentifierHash($PEER_IDENTIFIER) = $HASH"

RESOLVED=$(cast call --rpc-url "$RPC_URL" "$CHAIN_REGISTRY" \
  "getChainIdFromHash(bytes32)(bytes4)" "$HASH" | tr -d '\r\n')
echo "getChainIdFromHash(...) = $RESOLVED (expect $PEER_BYTES4)"

EXP=$(echo "$PEER_BYTES4" | tr '[:upper:]' '[:lower:]')
GOT=$(echo "$RESOLVED" | tr '[:upper:]' '[:lower:]')
if [ "$GOT" != "$EXP" ]; then
  echo "ERROR: resolved bytes4 mismatch (got $GOT, expect $EXP)" >&2
  exit 1
fi

BOOL=$(cast call --rpc-url "$RPC_URL" "$CHAIN_REGISTRY" \
  "isChainRegistered(bytes4)(bool)" "$PEER_BYTES4" | tr -d '\r\n')
echo "isChainRegistered($PEER_BYTES4) = $BOOL (expect true)"

case "$BOOL" in
  true|True) ;;
  *) echo "ERROR: chain not registered" >&2; exit 1 ;;
esac

echo "OK"
