#!/bin/bash
# CL8Y Bridge Contract Deployment Script
# 
# This script handles the deployment of CL8Y Bridge contracts to TerraClassic.
# 
# Prerequisites:
# - terrad CLI installed and configured
# - Wallet with sufficient LUNC for gas fees
# - Contracts compiled to WASM
#
# Usage:
#   ./deploy.sh <network> <wallet_name>
#   
#   network: testnet | mainnet
#   wallet_name: name of the key in terrad keyring

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARTIFACTS_DIR="${SCRIPT_DIR}/../artifacts"

# Network configurations
TESTNET_RPC="https://terra-classic-testnet-rpc.publicnode.com:443"
TESTNET_CHAIN_ID="rebel-2"
MAINNET_RPC="https://terra-classic-rpc.publicnode.com:443"
MAINNET_CHAIN_ID="columbus-5"

# Gas settings
GAS_PRICES="28.325uluna"
GAS_ADJUSTMENT="1.4"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

usage() {
    echo "Usage: $0 <network> <wallet_name>"
    echo ""
    echo "  network:     testnet | mainnet"
    echo "  wallet_name: name of the key in terrad keyring"
    echo ""
    echo "Example:"
    echo "  $0 testnet mykey"
    exit 1
}

# Validate arguments
if [ $# -lt 2 ]; then
    usage
fi

NETWORK="$1"
WALLET="$2"

# Set network configuration
case "$NETWORK" in
    testnet)
        RPC="$TESTNET_RPC"
        CHAIN_ID="$TESTNET_CHAIN_ID"
        log_info "Deploying to TESTNET (${CHAIN_ID})"
        ;;
    mainnet)
        RPC="$MAINNET_RPC"
        CHAIN_ID="$MAINNET_CHAIN_ID"
        log_warn "Deploying to MAINNET (${CHAIN_ID})"
        read -p "Are you sure you want to deploy to mainnet? (yes/no): " confirm
        if [ "$confirm" != "yes" ]; then
            log_info "Deployment cancelled"
            exit 0
        fi
        ;;
    *)
        log_error "Invalid network: $NETWORK"
        usage
        ;;
esac

# Common terrad flags
TERRAD_FLAGS="--node $RPC --chain-id $CHAIN_ID --gas-prices $GAS_PRICES --gas-adjustment $GAS_ADJUSTMENT --gas auto -y"

# Check if artifacts exist
check_artifacts() {
    local contracts=("bridge")
    
    for contract in "${contracts[@]}"; do
        local wasm_file="${ARTIFACTS_DIR}/${contract}.wasm"
        if [ ! -f "$wasm_file" ]; then
            log_error "WASM artifact not found: $wasm_file"
            log_info "Please run the optimizer first. See README.md for instructions."
            exit 1
        fi
    done
    
    log_info "All WASM artifacts found"
}

# Store a contract and return the code ID
store_contract() {
    local wasm_file="$1"
    local contract_name="$2"
    
    log_info "Storing $contract_name..."
    
    local result=$(terrad tx wasm store "$wasm_file" \
        --from "$WALLET" \
        $TERRAD_FLAGS \
        --output json)
    
    local txhash=$(echo "$result" | jq -r '.txhash')
    log_info "TX Hash: $txhash"
    
    # Wait for transaction to be included
    sleep 6
    
    local tx_result=$(terrad query tx "$txhash" --node "$RPC" --output json 2>/dev/null)
    local code_id=$(echo "$tx_result" | jq -r '.logs[0].events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value')
    
    if [ -z "$code_id" ] || [ "$code_id" == "null" ]; then
        log_error "Failed to get code ID for $contract_name"
        exit 1
    fi
    
    log_info "$contract_name stored with Code ID: $code_id"
    echo "$code_id"
}

# Instantiate a contract and return the contract address
instantiate_contract() {
    local code_id="$1"
    local init_msg="$2"
    local label="$3"
    
    log_info "Instantiating $label..."
    
    local result=$(terrad tx wasm instantiate "$code_id" "$init_msg" \
        --from "$WALLET" \
        --label "$label" \
        --admin "$WALLET" \
        $TERRAD_FLAGS \
        --output json)
    
    local txhash=$(echo "$result" | jq -r '.txhash')
    log_info "TX Hash: $txhash"
    
    # Wait for transaction to be included
    sleep 6
    
    local tx_result=$(terrad query tx "$txhash" --node "$RPC" --output json 2>/dev/null)
    local contract_addr=$(echo "$tx_result" | jq -r '.logs[0].events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value')
    
    if [ -z "$contract_addr" ] || [ "$contract_addr" == "null" ]; then
        log_error "Failed to get contract address for $label"
        exit 1
    fi
    
    log_info "$label instantiated at: $contract_addr"
    echo "$contract_addr"
}

# Main deployment flow
main() {
    log_info "Starting CL8Y Bridge deployment..."
    
    # Check artifacts
    check_artifacts
    
    # Get wallet address
    WALLET_ADDR=$(terrad keys show "$WALLET" -a)
    log_info "Deploying from wallet: $WALLET_ADDR"
    
    # Store all contracts
    log_info "=== Storing Contracts ==="
    
    BRIDGE_CODE_ID=$(store_contract "${ARTIFACTS_DIR}/bridge.wasm" "Bridge")
    
    log_info "=== Instantiating Contracts ==="
    
    # Instantiate Bridge with initial configuration
    # Update these values for your deployment
    BRIDGE_INIT="{
        \"admin\": \"$WALLET_ADDR\",
        \"relayers\": [\"$WALLET_ADDR\"],
        \"min_signatures\": 1,
        \"min_bridge_amount\": \"1000000\",
        \"max_bridge_amount\": \"1000000000000\",
        \"fee_bps\": 30,
        \"fee_collector\": \"$WALLET_ADDR\"
    }"
    # Remove whitespace for terrad
    BRIDGE_INIT=$(echo "$BRIDGE_INIT" | tr -d '\n' | tr -s ' ')
    
    BRIDGE_ADDR=$(instantiate_contract "$BRIDGE_CODE_ID" "$BRIDGE_INIT" "CL8Y Bridge")
    
    # Output summary
    log_info "=== Deployment Complete ==="
    echo ""
    echo "Network: $NETWORK ($CHAIN_ID)"
    echo ""
    echo "Code IDs:"
    echo "  Bridge: $BRIDGE_CODE_ID"
    echo ""
    echo "Contract Addresses:"
    echo "  Bridge: $BRIDGE_ADDR"
    echo ""
    
    # Save to file
    OUTPUT_FILE="${SCRIPT_DIR}/deployment-${NETWORK}-$(date +%Y%m%d-%H%M%S).json"
    cat > "$OUTPUT_FILE" << EOF
{
  "network": "$NETWORK",
  "chain_id": "$CHAIN_ID",
  "deployed_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "deployer": "$WALLET_ADDR",
  "code_ids": {
    "bridge": $BRIDGE_CODE_ID
  },
  "contracts": {
    "bridge": "$BRIDGE_ADDR"
  },
  "config": {
    "min_signatures": 1,
    "min_bridge_amount": "1000000",
    "max_bridge_amount": "1000000000000",
    "fee_bps": 30
  }
}
EOF
    
    log_info "Deployment info saved to: $OUTPUT_FILE"
    
    log_info "=== Post-Deployment Steps ==="
    echo ""
    echo "1. Add supported chains:"
    echo "   terrad tx wasm execute $BRIDGE_ADDR '{\"add_chain\":{\"chain_id\":1,\"name\":\"Ethereum\",\"bridge_address\":\"0x...\"}}' --from $WALLET $TERRAD_FLAGS"
    echo ""
    echo "2. Add supported tokens:"
    echo "   terrad tx wasm execute $BRIDGE_ADDR '{\"add_token\":{\"token\":\"uusd\",\"is_native\":true,\"evm_token_address\":\"0x...\",\"terra_decimals\":6,\"evm_decimals\":18}}' --from $WALLET $TERRAD_FLAGS"
    echo ""
    echo "3. Add additional relayers:"
    echo "   terrad tx wasm execute $BRIDGE_ADDR '{\"add_relayer\":{\"relayer\":\"terra1...\"}}' --from $WALLET $TERRAD_FLAGS"
    echo ""
}

main
