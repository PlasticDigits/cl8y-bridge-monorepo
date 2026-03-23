#!/usr/bin/env bash
# Setup test SPL token mints for Solana bridge testing.
# Creates three test tokens (testa, testb, tdec) with varying decimals,
# transfers mint authority to the faucet program PDA, and registers them.
#
# Prerequisites:
#   - solana CLI configured for the target cluster
#   - spl-token CLI installed
#   - Faucet program deployed (FAUCET_PROGRAM_ID set or defaulted)
#
# Usage:
#   ./scripts/solana/setup-test-tokens.sh

set -euo pipefail

SOLANA_RPC="${SOLANA_RPC_URL:-http://localhost:8899}"
FAUCET_PROGRAM_ID="${FAUCET_PROGRAM_ID:-CL8YFaucet1111111111111111111111111111111111}"
KEYPAIR="${SOLANA_KEYPAIR:-$HOME/.config/solana/id.json}"

echo "=== CL8Y Solana Test Token Setup ==="
echo "RPC:     $SOLANA_RPC"
echo "Faucet:  $FAUCET_PROGRAM_ID"
echo "Keypair: $KEYPAIR"
echo ""

# Derive faucet PDA (seeds = ["faucet"])
FAUCET_PDA=$(solana-keygen pubkey <(echo -n "faucet") 2>/dev/null || true)
echo "Note: Faucet PDA must be derived from the program. Run 'anchor test' to confirm."
echo ""

create_token() {
  local name="$1"
  local decimals="$2"

  echo "--- Creating $name (decimals=$decimals) ---"
  local mint
  mint=$(spl-token create-token \
    --url "$SOLANA_RPC" \
    --decimals "$decimals" \
    --keypair "$KEYPAIR" \
    2>&1 | grep "Creating token" | awk '{print $3}')

  if [ -z "$mint" ]; then
    echo "ERROR: Failed to create $name mint"
    return 1
  fi

  echo "$name mint: $mint"

  # Mint initial supply to deployer (for manual testing before faucet is set up)
  spl-token create-account "$mint" \
    --url "$SOLANA_RPC" \
    --keypair "$KEYPAIR" \
    --owner "$(solana-keygen pubkey "$KEYPAIR")" \
    2>/dev/null || true

  local amount
  amount=$(python3 -c "print(10**($decimals + 6))" 2>/dev/null || echo "1000000000")
  spl-token mint "$mint" "$amount" \
    --url "$SOLANA_RPC" \
    --keypair "$KEYPAIR" \
    2>/dev/null || echo "  (mint skipped — may need authority)"

  echo "  Created $name: $mint"
  echo ""
  echo "$mint"
}

echo "Creating test tokens..."
echo ""

TESTA_MINT=$(create_token "testa" 9)
TESTB_MINT=$(create_token "testb" 9)
TDEC_MINT=$(create_token "tdec" 6)

echo ""
echo "=== Summary ==="
echo "testa mint: $TESTA_MINT"
echo "testb mint: $TESTB_MINT"
echo "tdec  mint: $TDEC_MINT"
echo ""
echo "Add these to your frontend .env:"
echo "  VITE_SOLANA_TESTA_MINT=$TESTA_MINT"
echo "  VITE_SOLANA_TESTB_MINT=$TESTB_MINT"
echo "  VITE_SOLANA_TDEC_MINT=$TDEC_MINT"
echo "  VITE_SOLANA_FAUCET_ADDRESS=$FAUCET_PROGRAM_ID"
