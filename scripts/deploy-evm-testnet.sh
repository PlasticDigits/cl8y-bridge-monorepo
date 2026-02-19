#!/bin/bash
# Deploy EVM bridge contracts to testnets
#
# Supported Networks:
#   - BSC Testnet (chainId: 97)
#   - opBNB Testnet (chainId: 5611)
#
# Prerequisites:
#   - Foundry installed
#   - DEPLOYER_ADDRESS environment variable set (private key entered interactively)
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
BSC_TESTNET_MULTICHAIN_ID=97
BSC_TESTNET_EXPLORER="https://testnet.bscscan.com"
BSC_TESTNET_WETH="0xae13d989daC2f0dEbFf460aC112a837C89BAa7cd"

OPBNB_TESTNET_RPC="https://opbnb-testnet-rpc.bnbchain.org"
OPBNB_TESTNET_CHAIN_ID=5611
OPBNB_TESTNET_MULTICHAIN_ID=5611
OPBNB_TESTNET_EXPLORER="https://opbnb-testnet.bscscan.com"
OPBNB_TESTNET_WETH="0x4200000000000000000000000000000000000006"

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
        MULTICHAIN_ID="$BSC_TESTNET_MULTICHAIN_ID"
        EXPLORER="$BSC_TESTNET_EXPLORER"
        NETWORK_NAME="BSC Testnet"
        WETH_ADDRESS="$BSC_TESTNET_WETH"
        CHAIN_IDENTIFIER="BSC"
        ;;
    opbnb)
        RPC_URL="$OPBNB_TESTNET_RPC"
        CHAIN_ID="$OPBNB_TESTNET_CHAIN_ID"
        MULTICHAIN_ID="$OPBNB_TESTNET_MULTICHAIN_ID"
        EXPLORER="$OPBNB_TESTNET_EXPLORER"
        NETWORK_NAME="opBNB Testnet"
        WETH_ADDRESS="$OPBNB_TESTNET_WETH"
        CHAIN_IDENTIFIER="opBNB"
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
    
    if [ -z "$DEPLOYER_ADDRESS" ]; then
        log_error "DEPLOYER_ADDRESS environment variable is required"
        log_info "Set it with: export DEPLOYER_ADDRESS=0x..."
        exit 1
    fi
    
    # Check balance
    DEPLOYER="$DEPLOYER_ADDRESS"
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
        -vvv || {
        log_error "Deployment failed"
        exit 1
    }
    
    # Extract addresses from broadcast files
    BROADCAST_FILE="$CONTRACTS_DIR/broadcast/Deploy.s.sol/$CHAIN_ID/run-latest.json"
    
    if [ -f "$BROADCAST_FILE" ]; then
        log_step "Extracting deployed addresses..."
        
        # Deploy.s.sol CREATE order: impl, proxy, impl, proxy, ...
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
        echo "         Deployment Complete"
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
        echo "Environment Variables for Operator:"
        echo "  EVM_RPC_URL=$RPC_URL"
        echo "  EVM_CHAIN_ID=$CHAIN_ID"
        echo "  EVM_BRIDGE_ADDRESS=${BRIDGE_ADDRESS:-}"
        echo ""
        
        # Save to .env file
        ENV_FILE="$PROJECT_ROOT/.env.$NETWORK"
        cat > "$ENV_FILE" << EOF
# $NETWORK_NAME Deployment
# Generated: $(date -Iseconds)

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
