#!/bin/bash
# Deploy EVM bridge contracts to mainnets
#
# Supported Networks:
#   - BSC Mainnet (chainId: 56)
#   - opBNB Mainnet (chainId: 204)
#
# Prerequisites:
#   - Foundry installed
#   - PRIVATE_KEY environment variable set
#   - Real BNB for gas
#   - BSCSCAN_API_KEY for verification
#
# Usage:
#   ./scripts/deploy-evm-mainnet.sh bsc    # Deploy to BSC Mainnet
#   ./scripts/deploy-evm-mainnet.sh opbnb  # Deploy to opBNB Mainnet
#
# ⚠️  DANGER: This deploys to MAINNET with REAL funds!

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
BSC_MAINNET_RPC="https://bsc-dataseed1.binance.org"
BSC_MAINNET_CHAIN_ID=56
BSC_MAINNET_EXPLORER="https://bscscan.com"

OPBNB_MAINNET_RPC="https://opbnb-mainnet-rpc.bnbchain.org"
OPBNB_MAINNET_CHAIN_ID=204
OPBNB_MAINNET_EXPLORER="https://opbnbscan.com"

# Parse arguments
NETWORK="${1:-}"
SKIP_CONFIRMATION=false

for arg in "$@"; do
    case $arg in
        --yes|-y) SKIP_CONFIRMATION=true ;;
    esac
done

if [ -z "$NETWORK" ]; then
    echo "Usage: $0 <network> [--yes]"
    echo ""
    echo "Networks:"
    echo "  bsc     - BSC Mainnet (chainId: 56)"
    echo "  opbnb   - opBNB Mainnet (chainId: 204)"
    echo ""
    echo "Options:"
    echo "  --yes   - Skip confirmation prompts"
    exit 1
fi

# Set network parameters
case "$NETWORK" in
    bsc)
        RPC_URL="$BSC_MAINNET_RPC"
        CHAIN_ID="$BSC_MAINNET_CHAIN_ID"
        EXPLORER="$BSC_MAINNET_EXPLORER"
        NETWORK_NAME="BSC Mainnet"
        ;;
    opbnb)
        RPC_URL="$OPBNB_MAINNET_RPC"
        CHAIN_ID="$OPBNB_MAINNET_CHAIN_ID"
        EXPLORER="$OPBNB_MAINNET_EXPLORER"
        NETWORK_NAME="opBNB Mainnet"
        ;;
    *)
        log_error "Unknown network: $NETWORK"
        exit 1
        ;;
esac

# Safety confirmation
safety_confirmation() {
    echo ""
    echo -e "${RED}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║                    ⚠️  MAINNET DEPLOYMENT ⚠️                 ║${NC}"
    echo -e "${RED}╠════════════════════════════════════════════════════════════╣${NC}"
    echo -e "${RED}║  You are about to deploy to: ${NETWORK_NAME}                   ${NC}"
    echo -e "${RED}║  Chain ID: ${CHAIN_ID}                                            ${NC}"
    echo -e "${RED}║                                                            ║${NC}"
    echo -e "${RED}║  This will use REAL funds and cannot be undone!            ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    
    if [ "$SKIP_CONFIRMATION" = false ]; then
        read -p "Type 'DEPLOY TO MAINNET' to confirm: " confirmation
        if [ "$confirmation" != "DEPLOY TO MAINNET" ]; then
            log_error "Deployment cancelled"
            exit 1
        fi
        echo ""
    fi
}

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
    
    if [ -z "$BSCSCAN_API_KEY" ]; then
        log_warn "BSCSCAN_API_KEY not set - contract verification will fail"
    fi
    
    # Check balance
    DEPLOYER=$(cast wallet address --private-key "$PRIVATE_KEY")
    log_info "Deployer address: $DEPLOYER"
    
    BALANCE=$(cast balance "$DEPLOYER" --rpc-url "$RPC_URL" 2>/dev/null || echo "0")
    BALANCE_ETH=$(cast from-wei "$BALANCE" 2>/dev/null || echo "0")
    log_info "Balance: $BALANCE_ETH BNB"
    
    if [ "$BALANCE" = "0" ]; then
        log_error "Deployer has no balance!"
        exit 1
    fi
    
    # Estimate gas cost (rough estimate)
    ESTIMATED_COST="0.05"
    log_info "Estimated deployment cost: ~$ESTIMATED_COST BNB"
    
    if [ "$SKIP_CONFIRMATION" = false ]; then
        read -p "Continue with deployment? (y/N): " continue_confirm
        if [ "$continue_confirm" != "y" ] && [ "$continue_confirm" != "Y" ]; then
            log_info "Deployment cancelled"
            exit 0
        fi
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
        --slow \
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
        echo "    🎉 MAINNET Deployment Complete 🎉"
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
        echo "Explorer Links:"
        if [ -n "$BRIDGE_ADDRESS" ]; then
            echo "  Bridge:   $EXPLORER/address/$BRIDGE_ADDRESS"
        fi
        if [ -n "$BRIDGE_ROUTER" ]; then
            echo "  Router:   $EXPLORER/address/$BRIDGE_ROUTER"
        fi
        echo ""
        echo "Verification Commands:"
        if [ -n "$BRIDGE_ADDRESS" ]; then
            echo "  forge verify-contract $BRIDGE_ADDRESS Cl8YBridge --chain-id $CHAIN_ID --etherscan-api-key \$BSCSCAN_API_KEY"
        fi
        echo ""
        echo "Environment Variables for Operator:"
        echo "  EVM_RPC_URL=$RPC_URL"
        echo "  EVM_CHAIN_ID=$CHAIN_ID"
        echo "  EVM_BRIDGE_ADDRESS=${BRIDGE_ADDRESS:-}"
        echo "  EVM_ROUTER_ADDRESS=${BRIDGE_ROUTER:-}"
        echo ""
        
        # Save to .env file
        ENV_FILE="$PROJECT_ROOT/.env.$NETWORK-mainnet"
        cat > "$ENV_FILE" << EOF
# $NETWORK_NAME Deployment (MAINNET)
# Generated: $(date -Iseconds)
# ⚠️ PRODUCTION CONFIGURATION

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
        
        echo ""
        echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║  IMPORTANT: Save these addresses securely!                 ║${NC}"
        echo -e "${GREEN}║  You will need them for operator and canceler config.      ║${NC}"
        echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
        echo ""
        
    else
        log_warn "Could not find broadcast file at $BROADCAST_FILE"
    fi
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "   CL8Y Bridge EVM MAINNET Deployment"
    echo "========================================"
    echo ""
    
    safety_confirmation
    
    log_info "Target: $NETWORK_NAME (chainId: $CHAIN_ID)"
    echo ""
    
    check_prereqs
    deploy_contracts
}

main
