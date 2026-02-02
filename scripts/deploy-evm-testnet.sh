#!/bin/bash
# Deploy EVM bridge contracts to testnets
#
# Supported Networks:
#   - BSC Testnet (chainId: 97)
#   - opBNB Testnet (chainId: 5611)
#
# Prerequisites:
#   - Foundry installed
#   - PRIVATE_KEY environment variable set
#   - Testnet tokens for gas (tBNB)
#
# Usage:
#   ./scripts/deploy-evm-testnet.sh bsc    # Deploy to BSC Testnet
#   ./scripts/deploy-evm-testnet.sh opbnb  # Deploy to opBNB Testnet

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$PROJECT_ROOT/packages/contracts-evm"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

# Network configurations
BSC_TESTNET_RPC="https://data-seed-prebsc-1-s1.binance.org:8545"
BSC_TESTNET_CHAIN_ID=97
BSC_TESTNET_EXPLORER="https://testnet.bscscan.com"

OPBNB_TESTNET_RPC="https://opbnb-testnet-rpc.bnbchain.org"
OPBNB_TESTNET_CHAIN_ID=5611
OPBNB_TESTNET_EXPLORER="https://opbnb-testnet.bscscan.com"

# Parse arguments
NETWORK="${1:-}"

if [ -z "$NETWORK" ]; then
    echo "Usage: $0 <network>"
    echo ""
    echo "Networks:"
    echo "  bsc     - BSC Testnet (chainId: 97)"
    echo "  opbnb   - opBNB Testnet (chainId: 5611)"
    exit 1
fi

# Set network parameters
case "$NETWORK" in
    bsc)
        RPC_URL="$BSC_TESTNET_RPC"
        CHAIN_ID="$BSC_TESTNET_CHAIN_ID"
        EXPLORER="$BSC_TESTNET_EXPLORER"
        NETWORK_NAME="BSC Testnet"
        ;;
    opbnb)
        RPC_URL="$OPBNB_TESTNET_RPC"
        CHAIN_ID="$OPBNB_TESTNET_CHAIN_ID"
        EXPLORER="$OPBNB_TESTNET_EXPLORER"
        NETWORK_NAME="opBNB Testnet"
        ;;
    *)
        log_error "Unknown network: $NETWORK"
        exit 1
        ;;
esac

# Check prerequisites
check_prereqs() {
    log_step "Checking prerequisites..."
    
    if ! command -v forge &> /dev/null; then
        log_error "Foundry (forge) is required but not installed"
        exit 1
    fi
    
    if [ -z "$PRIVATE_KEY" ]; then
        log_error "PRIVATE_KEY environment variable is required"
        exit 1
    fi
    
    # Check balance
    DEPLOYER=$(cast wallet address --private-key "$PRIVATE_KEY")
    log_info "Deployer address: $DEPLOYER"
    
    BALANCE=$(cast balance "$DEPLOYER" --rpc-url "$RPC_URL" 2>/dev/null || echo "0")
    BALANCE_ETH=$(cast from-wei "$BALANCE" 2>/dev/null || echo "0")
    log_info "Balance: $BALANCE_ETH BNB"
    
    if [ "$BALANCE" = "0" ]; then
        log_error "Deployer has no balance. Get testnet tokens from faucet."
        case "$NETWORK" in
            bsc) log_info "Faucet: https://testnet.bnbchain.org/faucet-smart" ;;
            opbnb) log_info "Faucet: https://opbnb-testnet-bridge.bnbchain.org/deposit" ;;
        esac
        exit 1
    fi
    
    log_info "Prerequisites OK"
}

# Deploy contracts
deploy_contracts() {
    log_step "Deploying contracts to $NETWORK_NAME (chainId: $CHAIN_ID)..."
    
    cd "$CONTRACTS_DIR"
    
    # Build first
    log_info "Building contracts..."
    forge build
    
    # Deploy using the deploy script
    log_info "Running deployment script..."
    DEPLOY_OUTPUT=$(forge script script/DeployPart1.s.sol:DeployPart1 \
        --rpc-url "$RPC_URL" \
        --private-key "$PRIVATE_KEY" \
        --broadcast \
        --verify \
        --etherscan-api-key "${BSCSCAN_API_KEY:-}" \
        -vvv 2>&1) || {
        log_error "Deployment failed"
        echo "$DEPLOY_OUTPUT"
        exit 1
    }
    
    echo "$DEPLOY_OUTPUT"
    
    # Extract addresses from broadcast files
    BROADCAST_FILE="$CONTRACTS_DIR/broadcast/DeployPart1.s.sol/$CHAIN_ID/run-latest.json"
    
    if [ -f "$BROADCAST_FILE" ]; then
        log_step "Extracting deployed addresses..."
        
        BRIDGE_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "Cl8YBridge" or .contractName == "CL8YBridge") | .contractAddress' "$BROADCAST_FILE" | head -1)
        TOKEN_REGISTRY=$(jq -r '.transactions[] | select(.contractName == "TokenRegistry") | .contractAddress' "$BROADCAST_FILE" | head -1)
        CHAIN_REGISTRY=$(jq -r '.transactions[] | select(.contractName == "ChainRegistry") | .contractAddress' "$BROADCAST_FILE" | head -1)
        BRIDGE_ROUTER=$(jq -r '.transactions[] | select(.contractName == "BridgeRouter") | .contractAddress' "$BROADCAST_FILE" | head -1)
        MINT_BURN=$(jq -r '.transactions[] | select(.contractName == "MintBurn") | .contractAddress' "$BROADCAST_FILE" | head -1)
        LOCK_UNLOCK=$(jq -r '.transactions[] | select(.contractName == "LockUnlock") | .contractAddress' "$BROADCAST_FILE" | head -1)
        
        echo ""
        echo "========================================"
        echo "         Deployment Complete"
        echo "========================================"
        echo ""
        echo "Network: $NETWORK_NAME (chainId: $CHAIN_ID)"
        echo "Explorer: $EXPLORER"
        echo ""
        echo "Contract Addresses:"
        echo "  CL8YBridge:     ${BRIDGE_ADDRESS:-not found}"
        echo "  BridgeRouter:   ${BRIDGE_ROUTER:-not found}"
        echo "  TokenRegistry:  ${TOKEN_REGISTRY:-not found}"
        echo "  ChainRegistry:  ${CHAIN_REGISTRY:-not found}"
        echo "  MintBurn:       ${MINT_BURN:-not found}"
        echo "  LockUnlock:     ${LOCK_UNLOCK:-not found}"
        echo ""
        echo "Verification Commands:"
        if [ -n "$BRIDGE_ADDRESS" ]; then
            echo "  forge verify-contract $BRIDGE_ADDRESS Cl8YBridge --chain-id $CHAIN_ID"
        fi
        echo ""
        echo "Environment Variables for Operator:"
        echo "  EVM_RPC_URL=$RPC_URL"
        echo "  EVM_CHAIN_ID=$CHAIN_ID"
        echo "  EVM_BRIDGE_ADDRESS=${BRIDGE_ADDRESS:-}"
        echo "  EVM_ROUTER_ADDRESS=${BRIDGE_ROUTER:-}"
        echo ""
        
        # Save to .env file
        ENV_FILE="$PROJECT_ROOT/.env.$NETWORK"
        cat > "$ENV_FILE" << EOF
# $NETWORK_NAME Deployment
# Generated: $(date -Iseconds)

EVM_RPC_URL=$RPC_URL
EVM_CHAIN_ID=$CHAIN_ID
EVM_BRIDGE_ADDRESS=${BRIDGE_ADDRESS:-}
EVM_ROUTER_ADDRESS=${BRIDGE_ROUTER:-}
EVM_TOKEN_REGISTRY=${TOKEN_REGISTRY:-}
EVM_CHAIN_REGISTRY=${CHAIN_REGISTRY:-}
EVM_MINT_BURN=${MINT_BURN:-}
EVM_LOCK_UNLOCK=${LOCK_UNLOCK:-}
EOF
        log_info "Addresses saved to $ENV_FILE"
        
    else
        log_warn "Could not find broadcast file at $BROADCAST_FILE"
    fi
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "   CL8Y Bridge EVM Testnet Deployment"
    echo "========================================"
    echo ""
    
    log_info "Target: $NETWORK_NAME (chainId: $CHAIN_ID)"
    echo ""
    
    check_prereqs
    deploy_contracts
}

main
