#!/usr/bin/env bash
# Initialize the Solana bridge program
#
# Usage: ./scripts/solana/initialize-bridge.sh
#
# Env vars:
#   SOLANA_RPC_URL     - Solana RPC endpoint (default: http://localhost:8899)
#   SOLANA_KEYPAIR     - Path to admin keypair JSON (default: ~/.config/solana/id.json)
#   SOLANA_PROGRAM_ID  - Deployed program ID
#   OPERATOR_PUBKEY    - Operator public key
#   FEE_BPS            - Fee in basis points (default: 50 = 0.5%)
#   WITHDRAW_DELAY     - Withdrawal delay in seconds (default: 300 = 5 minutes)

set -euo pipefail

SOLANA_RPC_URL="${SOLANA_RPC_URL:-http://localhost:8899}"
SOLANA_KEYPAIR="${SOLANA_KEYPAIR:-${HOME}/.config/solana/id.json}"
SOLANA_PROGRAM_ID="${SOLANA_PROGRAM_ID:?SOLANA_PROGRAM_ID is required}"
OPERATOR_PUBKEY="${OPERATOR_PUBKEY:?OPERATOR_PUBKEY is required}"
FEE_BPS="${FEE_BPS:-50}"
WITHDRAW_DELAY="${WITHDRAW_DELAY:-300}"

ADMIN_PUBKEY=$(solana-keygen pubkey "${SOLANA_KEYPAIR}")

echo "Initializing CL8Y Bridge on Solana"
echo "  Program:        ${SOLANA_PROGRAM_ID}"
echo "  Admin:          ${ADMIN_PUBKEY}"
echo "  Operator:       ${OPERATOR_PUBKEY}"
echo "  Fee:            ${FEE_BPS} bps"
echo "  Withdraw Delay: ${WITHDRAW_DELAY}s"

BRIDGE_PDA=$(solana find-program-derived-address \
  "${SOLANA_PROGRAM_ID}" \
  --input string:bridge 2>/dev/null | head -1 || echo "(compute manually)")
echo "  Bridge PDA:     ${BRIDGE_PDA}"

EXISTING=$(solana account "${BRIDGE_PDA}" --url "${SOLANA_RPC_URL}" 2>/dev/null || true)
if [ -n "${EXISTING}" ] && ! echo "${EXISTING}" | grep -q "Error"; then
  echo ""
  echo "Bridge PDA already exists - skipping initialization"
  exit 0
fi

echo ""
echo "Sending initialize transaction..."

cd packages/contracts-solana
anchor test --skip-deploy --skip-lint \
  --provider.cluster "${SOLANA_RPC_URL}" \
  --provider.wallet "${SOLANA_KEYPAIR}" \
  -- --grep "initialize"

echo "Bridge initialized!"
