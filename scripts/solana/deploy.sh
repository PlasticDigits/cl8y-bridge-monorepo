#!/usr/bin/env bash
# Deploy CL8Y Bridge Solana program (cl8y_bridge) only. cl8y_faucet is not built or deployed here
# (saves rent/fees and matches mainnet). For local all-program deploy use scripts/solana/anchor-deploy-localnet.sh;
# optional SPL faucet program deploy is documented in docs/solana-mainnet-faucet-deployment.md.
#
# Usage: ./scripts/solana/deploy.sh [localnet|devnet|mainnet-beta]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

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

cd "$REPO_ROOT/packages/contracts-solana"

# Bridge program pubkey must match declare_id! / Anchor.toml (committed under keys/localnet/).
KEYS_DIR="$REPO_ROOT/packages/contracts-solana/keys/localnet"
mkdir -p target/deploy
cp "$KEYS_DIR/cl8y_bridge-keypair.json" target/deploy/

# Build bridge only (smaller/faster; avoids faucet program rent on devnet/localnet when using this script).
echo "[1/4] Building Anchor program (cl8y_bridge)..."
# Smaller on-chain binary (less rent): size-optimized release profile (Cargo.toml) + no instruction-name logging.
anchor build -p cl8y_bridge -- --features no-log-ix-name

# Get program ID
PROGRAM_ID=$(solana-keygen pubkey target/deploy/cl8y_bridge-keypair.json)
echo "  Program ID: ${PROGRAM_ID}"

# Wallet for `anchor deploy` / tests (override with SOLANA_KEYPAIR, e.g. ~/.config/solana/id-deployer.json)
export ANCHOR_WALLET="${ANCHOR_WALLET:-${SOLANA_KEYPAIR:-${HOME}/.config/solana/id.json}}"
echo "  Signing with: ${ANCHOR_WALLET} ($(solana-keygen pubkey "${ANCHOR_WALLET}"))"

# Deploy: pass --provider.wallet so deploy fee payer matches SOLANA_KEYPAIR (Anchor.toml wallet is id.json)
echo "[2/4] Deploying cl8y_bridge to ${CLUSTER}..."
anchor deploy --provider.cluster "${RPC_URL}" --provider.wallet "${ANCHOR_WALLET}" --program-name cl8y_bridge

# Verify
echo "[3/4] Verifying deployment..."
solana program show "${PROGRAM_ID}" --url "${RPC_URL}"

# Hash parity: full golden vectors for localnet/devnet; minimal TS smoke for mainnet (single Solana token fixture)
echo "[4/4] Running hash parity verification..."
if [[ "${CLUSTER}" == "mainnet-beta" ]]; then
  ANCHOR_PROVIDER_URL="${RPC_URL}" \
    npx ts-mocha -p ./tsconfig.json -t 1000000 tests/hash_parity_mainnet_deploy.test.ts
else
  ANCHOR_PROVIDER_URL="${RPC_URL}" \
    npx ts-mocha -p ./tsconfig.json -t 1000000 tests/hash_parity.test.ts
fi

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
