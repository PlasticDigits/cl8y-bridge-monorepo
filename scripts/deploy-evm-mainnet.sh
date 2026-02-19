#!/bin/bash
# Deploy EVM bridge contracts to mainnets
#
# Supported Networks:
#   - BSC Mainnet (chainId: 56)
#   - opBNB Mainnet (chainId: 204)
#
# Prerequisites:
#   - Foundry installed
#   - DEPLOYER_ADDRESS environment variable set (private key entered interactively)
#   - Real BNB for gas
#   - ETHERSCAN_API_KEY for verification (V2 key, works across all chains)
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
BSC_MAINNET_MULTICHAIN_ID=56
BSC_MAINNET_EXPLORER="https://bscscan.com"
BSC_MAINNET_WETH="0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"

OPBNB_MAINNET_RPC="https://opbnb-mainnet-rpc.bnbchain.org"
OPBNB_MAINNET_CHAIN_ID=204
OPBNB_MAINNET_MULTICHAIN_ID=204
OPBNB_MAINNET_EXPLORER="https://opbnbscan.com"
OPBNB_MAINNET_WETH="0x4200000000000000000000000000000000000006"

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
        MULTICHAIN_ID="$BSC_MAINNET_MULTICHAIN_ID"
        EXPLORER="$BSC_MAINNET_EXPLORER"
        NETWORK_NAME="BSC Mainnet"
        WETH_ADDRESS="$BSC_MAINNET_WETH"
        CHAIN_IDENTIFIER="BSC"
        ;;
    opbnb)
        RPC_URL="$OPBNB_MAINNET_RPC"
        CHAIN_ID="$OPBNB_MAINNET_CHAIN_ID"
        MULTICHAIN_ID="$OPBNB_MAINNET_MULTICHAIN_ID"
        EXPLORER="$OPBNB_MAINNET_EXPLORER"
        NETWORK_NAME="opBNB Mainnet"
        WETH_ADDRESS="$OPBNB_MAINNET_WETH"
        CHAIN_IDENTIFIER="opBNB"
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
    
    if [ -z "$DEPLOYER_ADDRESS" ]; then
        log_error "DEPLOYER_ADDRESS environment variable is required"
        log_info "Set it with: export DEPLOYER_ADDRESS=0x..."
        exit 1
    fi
    
    if [ -z "$ADMIN_ADDRESS" ]; then
        log_error "ADMIN_ADDRESS environment variable is required (multi-sig recommended)"
        exit 1
    fi
    
    if [ -z "$OPERATOR_ADDRESS" ]; then
        log_error "OPERATOR_ADDRESS environment variable is required"
        exit 1
    fi
    
    if [ -z "$FEE_RECIPIENT_ADDRESS" ]; then
        log_error "FEE_RECIPIENT_ADDRESS environment variable is required"
        exit 1
    fi
    
    if [ -z "$ETHERSCAN_API_KEY" ]; then
        log_warn "ETHERSCAN_API_KEY not set - contract verification will fail"
        log_info "Get a V2 key at https://etherscan.io/myapikey (works across all chains)"
    fi
    
    # Check balance
    DEPLOYER="$DEPLOYER_ADDRESS"
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
    
    # Export env vars that Deploy.s.sol reads
    export WETH_ADDRESS
    export CHAIN_IDENTIFIER
    export THIS_CHAIN_ID="$MULTICHAIN_ID"
    export ADMIN_ADDRESS
    export OPERATOR_ADDRESS
    export FEE_RECIPIENT_ADDRESS
    
    # Deploy using the deploy script (interactive key entry via -i 1)
    log_info "Running deployment script..."
    log_info "WETH_ADDRESS=$WETH_ADDRESS"
    log_info "CHAIN_IDENTIFIER=$CHAIN_IDENTIFIER"
    log_info "THIS_CHAIN_ID=$THIS_CHAIN_ID"
    forge script script/Deploy.s.sol:Deploy \
        --rpc-url "$RPC_URL" \
        --sender "$DEPLOYER_ADDRESS" \
        -i 1 \
        --broadcast \
        --verify \
        --etherscan-api-key "${ETHERSCAN_API_KEY:-}" \
        --slow \
        -vvv || {
        log_error "Deployment failed"
        exit 1
    }
    
    # Extract addresses from broadcast files
    BROADCAST_FILE="$CONTRACTS_DIR/broadcast/Deploy.s.sol/$CHAIN_ID/run-latest.json"
    
    if [ -f "$BROADCAST_FILE" ]; then
        log_step "Extracting deployed addresses..."
        
        # Deploy.s.sol deploys in order: impl, proxy, impl, proxy, ...
        # Extract CREATE-only transactions to get the deployment sequence:
        #   [0] ChainRegistry impl    [1] ChainRegistry proxy
        #   [2] TokenRegistry impl    [3] TokenRegistry proxy
        #   [4] LockUnlock impl       [5] LockUnlock proxy
        #   [6] MintBurn impl         [7] MintBurn proxy
        #   [8] Bridge impl           [9] Bridge proxy
        CREATES=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[].contractAddress' "$BROADCAST_FILE")
        
        CHAIN_REGISTRY_IMPL=$(echo "$CREATES" | sed -n '1p')
        CHAIN_REGISTRY=$(echo "$CREATES" | sed -n '2p')
        TOKEN_REGISTRY_IMPL=$(echo "$CREATES" | sed -n '3p')
        TOKEN_REGISTRY=$(echo "$CREATES" | sed -n '4p')
        LOCK_UNLOCK_IMPL=$(echo "$CREATES" | sed -n '5p')
        LOCK_UNLOCK=$(echo "$CREATES" | sed -n '6p')
        MINT_BURN_IMPL=$(echo "$CREATES" | sed -n '7p')
        MINT_BURN=$(echo "$CREATES" | sed -n '8p')
        BRIDGE_IMPL=$(echo "$CREATES" | sed -n '9p')
        BRIDGE_ADDRESS=$(echo "$CREATES" | sed -n '10p')
        
        echo ""
        echo "========================================"
        echo "    MAINNET Deployment Complete"
        echo "========================================"
        echo ""
        echo "Network: $NETWORK_NAME (chainId: $CHAIN_ID)"
        echo "Explorer: $EXPLORER"
        echo ""
        echo "Proxy Addresses (use these):"
        echo "  Bridge:         ${BRIDGE_ADDRESS:-not found}"
        echo "  ChainRegistry:  ${CHAIN_REGISTRY:-not found}"
        echo "  TokenRegistry:  ${TOKEN_REGISTRY:-not found}"
        echo "  LockUnlock:     ${LOCK_UNLOCK:-not found}"
        echo "  MintBurn:       ${MINT_BURN:-not found}"
        echo ""
        echo "Implementation Addresses:"
        echo "  Bridge:         ${BRIDGE_IMPL:-not found}"
        echo "  ChainRegistry:  ${CHAIN_REGISTRY_IMPL:-not found}"
        echo "  TokenRegistry:  ${TOKEN_REGISTRY_IMPL:-not found}"
        echo "  LockUnlock:     ${LOCK_UNLOCK_IMPL:-not found}"
        echo "  MintBurn:       ${MINT_BURN_IMPL:-not found}"
        echo ""
        echo "Explorer Links:"
        [ -n "$BRIDGE_ADDRESS" ] && echo "  Bridge:         $EXPLORER/address/$BRIDGE_ADDRESS"
        [ -n "$CHAIN_REGISTRY" ] && echo "  ChainRegistry:  $EXPLORER/address/$CHAIN_REGISTRY"
        [ -n "$TOKEN_REGISTRY" ] && echo "  TokenRegistry:  $EXPLORER/address/$TOKEN_REGISTRY"
        [ -n "$LOCK_UNLOCK" ] && echo "  LockUnlock:     $EXPLORER/address/$LOCK_UNLOCK"
        [ -n "$MINT_BURN" ] && echo "  MintBurn:       $EXPLORER/address/$MINT_BURN"
        echo ""
        echo "Environment Variables for Operator:"
        echo "  EVM_RPC_URL=$RPC_URL"
        echo "  EVM_CHAIN_ID=$CHAIN_ID"
        echo "  EVM_BRIDGE_ADDRESS=${BRIDGE_ADDRESS:-}"
        echo ""
        
        # Save to .env file
        ENV_FILE="$PROJECT_ROOT/.env.$NETWORK-mainnet"
        cat > "$ENV_FILE" << EOF
# $NETWORK_NAME Deployment (MAINNET)
# Generated: $(date -Iseconds)
# PRODUCTION CONFIGURATION

EVM_RPC_URL=$RPC_URL
EVM_CHAIN_ID=$CHAIN_ID

# Proxy addresses (interact with these)
EVM_BRIDGE_ADDRESS=${BRIDGE_ADDRESS:-}
EVM_CHAIN_REGISTRY=${CHAIN_REGISTRY:-}
EVM_TOKEN_REGISTRY=${TOKEN_REGISTRY:-}
EVM_LOCK_UNLOCK=${LOCK_UNLOCK:-}
EVM_MINT_BURN=${MINT_BURN:-}

# Implementation addresses (for reference / verification)
EVM_BRIDGE_IMPL=${BRIDGE_IMPL:-}
EVM_CHAIN_REGISTRY_IMPL=${CHAIN_REGISTRY_IMPL:-}
EVM_TOKEN_REGISTRY_IMPL=${TOKEN_REGISTRY_IMPL:-}
EVM_LOCK_UNLOCK_IMPL=${LOCK_UNLOCK_IMPL:-}
EVM_MINT_BURN_IMPL=${MINT_BURN_IMPL:-}
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
