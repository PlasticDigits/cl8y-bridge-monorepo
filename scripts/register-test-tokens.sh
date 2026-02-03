#!/bin/bash
# Register Test Tokens on Both Bridges
#
# This script registers test ERC20 and CW20 tokens on both chains,
# enabling cross-chain transfers in E2E tests.
#
# Prerequisites:
# - e2e-setup.sh has been run (or contracts deployed manually)
# - Test tokens deployed (make deploy-test-token, deploy-terra-local.sh --cw20)
# - Environment variables set (source .env.e2e)
#
# Usage:
#   ./scripts/register-test-tokens.sh
#   ./scripts/register-test-tokens.sh --evm-only
#   ./scripts/register-test-tokens.sh --terra-only

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Source environment
if [ -f "$PROJECT_ROOT/.env.e2e" ]; then
    set -a
    source "$PROJECT_ROOT/.env.e2e"
    set +a
fi

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

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_LCD_URL="${TERRA_LCD_URL:-http://localhost:1317}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-localterra}"

# Deployed addresses (from deploy scripts)
EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-}"
TOKEN_REGISTRY_ADDRESS="${TOKEN_REGISTRY_ADDRESS:-}"
LOCK_UNLOCK_ADDRESS="${LOCK_UNLOCK_ADDRESS:-}"
MINT_BURN_ADDRESS="${MINT_BURN_ADDRESS:-}"
TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-}"

# Test token addresses (from deploy-test-token and deploy-terra-local.sh --cw20)
TEST_TOKEN_ADDRESS="${TEST_TOKEN_ADDRESS:-}"
TERRA_CW20_ADDRESS="${TERRA_CW20_ADDRESS:-}"

# Private keys
EVM_PRIVATE_KEY="${EVM_PRIVATE_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
TERRA_KEY_NAME="${TERRA_KEY_NAME:-test1}"

# LocalTerra container
CONTAINER_NAME="${LOCALTERRA_CONTAINER:-cl8y-bridge-monorepo-localterra-1}"

# Parse arguments
EVM_ONLY=false
TERRA_ONLY=false
for arg in "$@"; do
    case $arg in
        --evm-only) EVM_ONLY=true ;;
        --terra-only) TERRA_ONLY=true ;;
    esac
done

# Compute chain keys
compute_evm_chain_key() {
    local chain_id="$1"
    cast keccak256 "$(cast abi-encode 'f(string,uint256)' 'EVM' "$chain_id")"
}

compute_terra_chain_key() {
    local chain_id="$1"
    cast keccak256 "$(cast abi-encode 'f(string,string,string)' 'COSMOS' "$chain_id" 'terra')"
}

# Check prerequisites
check_prereqs() {
    log_step "Checking prerequisites..."
    
    local failed=0
    
    if [ -z "$EVM_BRIDGE_ADDRESS" ]; then
        log_error "EVM_BRIDGE_ADDRESS not set"
        failed=1
    fi
    
    if [ -z "$TOKEN_REGISTRY_ADDRESS" ]; then
        log_error "TOKEN_REGISTRY_ADDRESS not set"
        failed=1
    fi
    
    if [ "$TERRA_ONLY" != true ] && [ -z "$TEST_TOKEN_ADDRESS" ]; then
        log_warn "TEST_TOKEN_ADDRESS not set - deploy with: make deploy-test-token"
        log_info "Skipping EVM token registration"
        EVM_ONLY=false
    fi
    
    if [ "$EVM_ONLY" != true ] && [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_warn "TERRA_BRIDGE_ADDRESS not set"
    fi
    
    if [ "$EVM_ONLY" != true ] && [ -z "$TERRA_CW20_ADDRESS" ]; then
        log_warn "TERRA_CW20_ADDRESS not set - deploy with: ./scripts/deploy-terra-local.sh --cw20"
    fi
    
    if [ $failed -eq 1 ]; then
        exit 1
    fi
    
    log_info "Prerequisites OK"
}

# Register ERC20 token on EVM TokenRegistry for Terra destination
register_evm_to_terra() {
    if [ "$TERRA_ONLY" = true ]; then
        return
    fi
    
    if [ -z "$TEST_TOKEN_ADDRESS" ]; then
        log_warn "Skipping EVM→Terra registration (no TEST_TOKEN_ADDRESS)"
        return
    fi
    
    log_step "Registering ERC20 token on EVM for Terra destination..."
    
    # Compute Terra chain key
    TERRA_CHAIN_KEY=$(compute_terra_chain_key "$TERRA_CHAIN_ID")
    log_info "Terra chain key: $TERRA_CHAIN_KEY"
    
    # Encode destination token (CW20 address or native denom)
    # For CW20: use the contract address as bytes32
    # For native: use the denom (e.g., "uluna") padded to bytes32
    local DEST_TOKEN
    if [ -n "$TERRA_CW20_ADDRESS" ]; then
        # CW20 token - encode as bytes32
        DEST_TOKEN=$(printf '%s' "$TERRA_CW20_ADDRESS" | xxd -p | tr -d '\n')
        DEST_TOKEN="0x$(printf '%-64s' "$DEST_TOKEN" | tr ' ' '0')"
        log_info "Dest token (CW20): $DEST_TOKEN"
    else
        # Native uluna - encode as bytes32
        DEST_TOKEN=$(printf '%s' "uluna" | xxd -p | tr -d '\n')
        DEST_TOKEN="0x$(printf '%-64s' "$DEST_TOKEN" | tr ' ' '0')"
        log_info "Dest token (native): $DEST_TOKEN"
    fi
    
    # Check if already registered
    REGISTERED=$(cast call "$TOKEN_REGISTRY_ADDRESS" "isTokenRegistered(address)" "$TEST_TOKEN_ADDRESS" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
    
    if [ "$REGISTERED" != "0" ]; then
        log_info "Token already registered on EVM TokenRegistry"
    else
        log_info "Registering token on TokenRegistry..."
        
        # TokenType.LockUnlock = 0
        TX=$(cast send "$TOKEN_REGISTRY_ADDRESS" \
            "registerToken(address,bytes32,bytes32,uint8)" \
            "$TEST_TOKEN_ADDRESS" \
            "$TERRA_CHAIN_KEY" \
            "$DEST_TOKEN" \
            "0" \
            --rpc-url "$EVM_RPC_URL" \
            --private-key "$EVM_PRIVATE_KEY" \
            --json 2>&1)
        
        if echo "$TX" | jq -e '.status == "0x1"' > /dev/null 2>&1; then
            log_info "Token registered on EVM TokenRegistry"
        else
            log_warn "Registration may have failed: $TX"
        fi
    fi
    
    # Grant MINTER role to LockUnlock if using MintBurn
    # For LockUnlock tokens, the LockUnlock contract holds the tokens
    log_info "EVM→Terra token registration complete"
}

# Register token on Terra bridge for EVM destination
register_terra_to_evm() {
    if [ "$EVM_ONLY" = true ]; then
        return
    fi
    
    if [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_warn "Skipping Terra→EVM registration (no TERRA_BRIDGE_ADDRESS)"
        return
    fi
    
    log_step "Registering token on Terra for EVM destination..."
    
    # EVM chain ID (Anvil = 31337)
    EVM_CHAIN_ID="${EVM_CHAIN_ID:-31337}"
    
    # Destination token on EVM
    local DEST_TOKEN_EVM
    if [ -n "$TEST_TOKEN_ADDRESS" ]; then
        DEST_TOKEN_EVM="$TEST_TOKEN_ADDRESS"
    else
        # Placeholder if no ERC20 deployed
        DEST_TOKEN_EVM="0x0000000000000000000000000000000000000001"
    fi
    
    # Build register message
    if [ -n "$TERRA_CW20_ADDRESS" ]; then
        # Register CW20 token
        REGISTER_MSG='{
            "register_token": {
                "token": "'$TERRA_CW20_ADDRESS'",
                "dest_chain_id": '$EVM_CHAIN_ID',
                "dest_token": "'$DEST_TOKEN_EVM'",
                "token_type": "lock_unlock"
            }
        }'
    else
        # Register native uluna
        REGISTER_MSG='{
            "register_native": {
                "denom": "uluna",
                "dest_chain_id": '$EVM_CHAIN_ID',
                "dest_token": "'$DEST_TOKEN_EVM'",
                "token_type": "lock_unlock"
            }
        }'
    fi
    
    log_info "Register message: $REGISTER_MSG"
    
    # Check if LocalTerra container is running
    if ! docker ps --format '{{.Names}}' | grep -q "$CONTAINER_NAME"; then
        log_warn "LocalTerra container not running, skipping Terra registration"
        return
    fi
    
    # Execute registration
    TX=$(docker exec "$CONTAINER_NAME" terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$REGISTER_MSG" \
        --from "$TERRA_KEY_NAME" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y --keyring-backend test -o json 2>&1)
    
    TX_HASH=$(echo "$TX" | jq -r '.txhash' 2>/dev/null || echo "")
    
    if [ -n "$TX_HASH" ] && [ "$TX_HASH" != "null" ]; then
        log_info "Registration TX: $TX_HASH"
        sleep 5
        log_info "Terra→EVM token registration submitted"
    else
        log_warn "Registration may have failed or already exists: $TX"
    fi
}

# Verify registrations
verify_registrations() {
    log_step "Verifying token registrations..."
    
    # Verify EVM registration
    if [ -n "$TEST_TOKEN_ADDRESS" ]; then
        REGISTERED=$(cast call "$TOKEN_REGISTRY_ADDRESS" "isTokenRegistered(address)" "$TEST_TOKEN_ADDRESS" --rpc-url "$EVM_RPC_URL" 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
        if [ "$REGISTERED" != "0" ]; then
            log_info "EVM TokenRegistry: $TEST_TOKEN_ADDRESS ✓"
        else
            log_warn "EVM TokenRegistry: $TEST_TOKEN_ADDRESS not registered"
        fi
    fi
    
    # Verify Terra registration (query registered tokens)
    if [ -n "$TERRA_BRIDGE_ADDRESS" ] && docker ps --format '{{.Names}}' | grep -q "$CONTAINER_NAME"; then
        QUERY='{"registered_tokens":{"limit":10}}'
        QUERY_B64=$(echo -n "$QUERY" | base64 -w0)
        RESULT=$(curl -sf "${TERRA_LCD_URL}/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE_ADDRESS}/smart/${QUERY_B64}" 2>/dev/null | jq '.data' 2>/dev/null || echo "{}")
        log_info "Terra registered tokens: $RESULT"
    fi
}

# Main
main() {
    log_info "=== CL8Y Bridge Token Registration ==="
    
    check_prereqs
    register_evm_to_terra
    register_terra_to_evm
    verify_registrations
    
    echo ""
    log_info "=== Token Registration Complete ==="
    echo ""
    echo "Registered tokens:"
    echo "  ERC20 on EVM: ${TEST_TOKEN_ADDRESS:-not deployed}"
    echo "  CW20 on Terra: ${TERRA_CW20_ADDRESS:-not deployed}"
    echo ""
    echo "Next steps:"
    echo "  1. Start operator: make operator-start"
    echo "  2. Run transfer test: ./scripts/e2e-test.sh --full --with-operator"
    echo ""
}

main "$@"
