#!/usr/bin/env bash
# Register full QA token mappings on the Solana bridge (same matrix as EVM/Terra).
#
# Usage:
#   QA_TOKEN_JSON must point to qa-tokens.json (written by register-tokens / qa:full-token-setup).
#   ./scripts/solana/register-tokens.sh
#
# Env vars:
#   QA_TOKEN_JSON      - Path to JSON from deploy-tokens (includes solana mints)
#   SOLANA_RPC_URL     - Solana RPC endpoint (default: http://localhost:8899)
#   SOLANA_KEYPAIR     - Admin keypair (default: ~/.config/solana/id.json)
#   ANCHOR_WALLET      - Same as SOLANA_KEYPAIR if set

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

SOLANA_RPC_URL="${SOLANA_RPC_URL:-http://localhost:8899}"
SOLANA_KEYPAIR="${SOLANA_KEYPAIR:-${HOME}/.config/solana/id.json}"

if [[ -z "${QA_TOKEN_JSON:-}" ]]; then
  echo "QA_TOKEN_JSON is not set. Run qa:full-token-setup or point it to .deploy/qa-tokens.json" >&2
  exit 1
fi

echo "Registering QA token mappings on Solana Bridge"
echo "  QA_TOKEN_JSON: ${QA_TOKEN_JSON}"
echo "  RPC:           ${SOLANA_RPC_URL}"
echo ""

cd "$REPO_ROOT/packages/contracts-solana"

ANCHOR_PROVIDER_URL="${SOLANA_RPC_URL}" \
ANCHOR_WALLET="${ANCHOR_WALLET:-${SOLANA_KEYPAIR}}" \
  npx tsx scripts/register-qa-tokens.ts

echo ""
echo "Solana QA token mappings registered."
