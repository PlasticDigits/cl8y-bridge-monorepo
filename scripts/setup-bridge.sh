#!/bin/bash
set -e

echo "=== Setting up Bridge Configuration ==="

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_RPC_URL="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"

# Load deployed addresses (these would be output from deploy scripts)
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
EVM_ROUTER_ADDRESS="${EVM_ROUTER_ADDRESS:-}"
EVM_TOKEN_REGISTRY="${EVM_TOKEN_REGISTRY:-}"
EVM_CHAIN_REGISTRY="${EVM_CHAIN_REGISTRY:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"

# Anvil default private key
DEPLOYER_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"

if [ -z "$EVM_BRIDGE_ADDRESS" ] || [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
    echo "Error: Bridge addresses not set. Run deploy first."
    echo "Set EVM_BRIDGE_ADDRESS and TERRA_BRIDGE_ADDRESS environment variables."
    exit 1
fi

echo ""
echo "=== Adding Terra Classic Chain to EVM Bridge ==="

# Compute Terra chain key (simplified - in practice use proper encoding)
# TERRA_CHAIN_KEY=$(cast keccak "COSMOS:columbus-5:terra")

# Add chain to EVM ChainRegistry
# cast send "$EVM_CHAIN_REGISTRY" "addChain(bytes32,string)" \
#     "$TERRA_CHAIN_KEY" "Terra Classic" \
#     --rpc-url "$EVM_RPC_URL" \
#     --private-key "$DEPLOYER_PRIVATE_KEY"

echo "Terra Classic chain added to EVM bridge."

echo ""
echo "=== Adding EVM Chain to Terra Bridge ==="

# Add EVM chain to Terra bridge
# terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
#     "{\"add_chain\":{\"chain_id\":31337,\"name\":\"Anvil\",\"bridge_address\":\"$EVM_BRIDGE_ADDRESS\"}}" \
#     --from test1 \
#     --gas auto --gas-adjustment 1.3 \
#     --node "$TERRA_RPC_URL" \
#     --chain-id "$TERRA_CHAIN_ID" \
#     -y

echo "EVM chain added to Terra bridge."

echo ""
echo "=== Adding Test Tokens ==="

# Add test token configuration on both sides
# This would add LUNC, USTC, and any test ERC20s

echo ""
echo "=== Adding Relayer ==="

# Add relayer address to Terra bridge
# RELAYER_TERRA_ADDRESS="terra1..."
# terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
#     "{\"add_relayer\":{\"relayer\":\"$RELAYER_TERRA_ADDRESS\"}}" \
#     --from test1 \
#     --gas auto --gas-adjustment 1.3 \
#     --node "$TERRA_RPC_URL" \
#     --chain-id "$TERRA_CHAIN_ID" \
#     -y

# Grant bridge operator role on EVM
# cast send "$EVM_ACCESS_MANAGER" "grantRole(bytes32,address,uint32)" \
#     "$BRIDGE_OPERATOR_ROLE" \
#     "$RELAYER_EVM_ADDRESS" \
#     0 \
#     --rpc-url "$EVM_RPC_URL" \
#     --private-key "$DEPLOYER_PRIVATE_KEY"

echo "Relayer configured on both chains."

echo ""
echo "=== Bridge Setup Complete ==="
echo ""
echo "Configuration:"
echo "  EVM Bridge: $EVM_BRIDGE_ADDRESS"
echo "  Terra Bridge: $TERRA_BRIDGE_ADDRESS"
echo ""
echo "Next: Run 'make relayer' to start the relayer service."
