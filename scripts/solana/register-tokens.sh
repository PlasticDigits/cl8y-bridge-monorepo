#!/usr/bin/env bash
# Register token mappings on the Solana bridge program
#
# Usage: ./scripts/solana/register-tokens.sh
#
# Env vars:
#   SOLANA_RPC_URL     - Solana RPC endpoint (default: http://localhost:8899)
#   SOLANA_KEYPAIR     - Path to admin keypair JSON (default: ~/.config/solana/id.json)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

SOLANA_RPC_URL="${SOLANA_RPC_URL:-http://localhost:8899}"
SOLANA_KEYPAIR="${SOLANA_KEYPAIR:-${HOME}/.config/solana/id.json}"

ADMIN_PUBKEY=$(solana-keygen pubkey "${SOLANA_KEYPAIR}")

echo "Registering Token Mappings on Solana Bridge"
echo "  Admin: ${ADMIN_PUBKEY}"
echo "  RPC:   ${SOLANA_RPC_URL}"
echo ""

cd "$REPO_ROOT/packages/contracts-solana"

echo "Sending register_token transaction via test runner..."
ANCHOR_PROVIDER_URL="${SOLANA_RPC_URL}" \
ANCHOR_WALLET="${SOLANA_KEYPAIR}" \
  npx ts-mocha -p ./tsconfig.json -t 1000000 tests/bridge.test.ts --grep "register_token"

echo ""
echo "Token mappings registered!"
