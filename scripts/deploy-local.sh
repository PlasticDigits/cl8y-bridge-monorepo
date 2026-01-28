#!/bin/bash
set -e

echo "=== CL8Y Bridge Local Deployment ==="

# Check prerequisites
command -v forge >/dev/null 2>&1 || { echo "forge not found. Install Foundry first."; exit 1; }
command -v terrad >/dev/null 2>&1 || { echo "terrad not found. Install Terra CLI first."; exit 1; }

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_RPC_URL="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"

# Anvil default private key (account 0)
DEPLOYER_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"

echo ""
echo "=== Deploying EVM Contracts ==="
cd "$(dirname "$0")/../packages/contracts-evm"

# Deploy to Anvil
forge script script/DeployLocal.s.sol:DeployLocal \
    --broadcast \
    --rpc-url "$EVM_RPC_URL" \
    --private-key "$DEPLOYER_PRIVATE_KEY" \
    -vvv

# Extract deployed addresses
echo "EVM contracts deployed!"

echo ""
echo "=== Deploying Terra Classic Contracts ==="
cd "$(dirname "$0")/../packages/contracts-terraclassic"

# Build contracts
echo "Building Terra contracts..."
cargo build --release --target wasm32-unknown-unknown

# Optimize (if docker available)
if command -v docker >/dev/null 2>&1; then
    echo "Optimizing contracts..."
    docker run --rm -v "$(pwd)":/code \
        --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
        --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
        cosmwasm/optimizer:0.15.0
fi

# Deploy to LocalTerra
echo "Deploying to LocalTerra..."
./scripts/deploy.sh local

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "Next steps:"
echo "1. Run: make setup-bridge"
echo "2. Run: make relayer"
echo "3. Run: make test-transfer"
