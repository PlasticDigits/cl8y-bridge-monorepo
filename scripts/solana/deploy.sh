#!/usr/bin/env bash
# Deploy CL8Y Bridge Solana program
#
# Usage: ./scripts/solana/deploy.sh [localnet|devnet|mainnet-beta]

set -euo pipefail

CLUSTER="${1:-localnet}"

case "${CLUSTER}" in
  localnet)
    RPC_URL="http://localhost:8899"
    ;;
  devnet)
    RPC_URL="https://api.devnet.solana.com"
    ;;
  mainnet-beta)
    RPC_URL="https://api.mainnet-beta.solana.com"
    ;;
  *)
    echo "Unknown cluster: ${CLUSTER}"
    echo "Usage: $0 [localnet|devnet|mainnet-beta]"
    exit 1
    ;;
esac

echo "============================================"
echo " CL8Y Bridge Solana Deployment"
echo " Cluster: ${CLUSTER}"
echo " RPC: ${RPC_URL}"
echo "============================================"
echo

cd packages/contracts-solana

# Build
echo "[1/4] Building Anchor program..."
anchor build

# Get program ID
PROGRAM_ID=$(solana-keygen pubkey target/deploy/cl8y_bridge-keypair.json)
echo "  Program ID: ${PROGRAM_ID}"

# Deploy
echo "[2/4] Deploying to ${CLUSTER}..."
anchor deploy --provider.cluster "${RPC_URL}"

# Verify
echo "[3/4] Verifying deployment..."
solana program show "${PROGRAM_ID}" --url "${RPC_URL}"

# Run hash parity test (call ts-mocha directly; anchor test passes -- args to cargo-build-sbf, not mocha)
echo "[4/4] Running hash parity verification..."
npx ts-mocha -p ./tsconfig.json -t 1000000 tests/hash_parity.test.ts

echo
echo "============================================"
echo " Deployment Complete!"
echo " Program ID: ${PROGRAM_ID}"
echo " Cluster: ${CLUSTER}"
echo "============================================"
echo
echo "Next steps:"
echo "  1. Register chain on EVM: ./scripts/solana/register-chain-evm.sh"
echo "  2. Register chain on Terra: ./scripts/solana/register-chain-terra.sh"
echo "  3. Register token mappings: ./scripts/solana/register-tokens.sh"
echo "  4. Configure operator with SOLANA_PROGRAM_ID=${PROGRAM_ID}"
echo "  5. Configure canceler with SOLANA_PROGRAM_ID=${PROGRAM_ID}"
