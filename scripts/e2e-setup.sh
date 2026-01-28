#!/bin/bash
# E2E Setup Script
#
# This script sets up the complete E2E testing environment:
# 1. Checks prerequisites
# 2. Starts Docker services with E2E profile
# 3. Waits for services to be healthy
# 4. Deploys EVM contracts
# 5. Exports addresses to .env.e2e
# 6. Funds test accounts
#
# Usage:
#   ./scripts/e2e-setup.sh
#
# Environment:
#   E2E_SKIP_DEPLOY=1  - Skip contract deployment (use existing)
#   E2E_VERBOSE=1      - Enable verbose output

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$PROJECT_ROOT/.env.e2e"

# E2E Port Configuration (dedicated ports to avoid conflicts)
export E2E_ANVIL_PORT="${E2E_ANVIL_PORT:-18545}"
export E2E_POSTGRES_PORT="${E2E_POSTGRES_PORT:-15433}"
export E2E_API_PORT="${E2E_API_PORT:-19090}"
export E2E_TERRA_RPC_PORT="${E2E_TERRA_RPC_PORT:-26657}"
export E2E_TERRA_LCD_PORT="${E2E_TERRA_LCD_PORT:-1317}"

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

# Cleanup function for trap
cleanup_on_error() {
    log_error "Setup failed! Cleaning up..."
    "$SCRIPT_DIR/e2e-teardown.sh" 2>/dev/null || true
    exit 1
}

# Check prerequisites
check_prerequisites() {
    log_step "Checking prerequisites..."
    
    local failed=0
    
    # Required tools
    for cmd in docker forge cast curl jq; do
        if ! command -v $cmd &> /dev/null; then
            log_error "$cmd is required but not installed"
            failed=1
        fi
    done
    
    # Docker must be running
    if ! docker info &> /dev/null; then
        log_error "Docker is not running"
        failed=1
    fi
    
    # Check docker compose
    if ! docker compose version &> /dev/null; then
        log_error "docker compose is required"
        failed=1
    fi
    
    if [ $failed -eq 1 ]; then
        exit 1
    fi
    
    log_info "Prerequisites OK"
}

# Clean up any existing E2E containers
cleanup_existing() {
    log_step "Cleaning up existing E2E containers..."
    
    # Stop any running E2E containers
    cd "$PROJECT_ROOT"
    docker compose --profile e2e down -v --remove-orphans 2>/dev/null || true
    
    # Remove the env file if it exists
    rm -f "$ENV_FILE"
    
    log_info "Cleanup complete"
}

# Start Docker services
start_services() {
    log_step "Starting Docker services with E2E profile..."
    
    cd "$PROJECT_ROOT"
    
    # Override ports for E2E to avoid conflicts
    POSTGRES_PORT=$E2E_POSTGRES_PORT docker compose --profile e2e up -d
    
    log_info "Services starting..."
}

# Wait for a service to be healthy
wait_for_service() {
    local name="$1"
    local check_cmd="$2"
    local timeout="${3:-120}"
    local interval="${4:-2}"
    local elapsed=0
    
    log_info "Waiting for $name to be ready (timeout: ${timeout}s)..."
    
    while [ $elapsed -lt $timeout ]; do
        if eval "$check_cmd" &>/dev/null; then
            log_info "$name is ready"
            return 0
        fi
        sleep $interval
        elapsed=$((elapsed + interval))
        echo -n "."
    done
    
    echo ""
    log_error "$name did not become ready within ${timeout}s"
    return 1
}

# Wait for all services to be healthy
wait_for_services() {
    log_step "Waiting for services to be healthy..."
    
    # Wait for Anvil (EVM)
    wait_for_service "Anvil" "cast block-number --rpc-url http://localhost:8545" 60 2
    
    # Wait for PostgreSQL
    wait_for_service "PostgreSQL" "docker compose exec -T postgres pg_isready -U relayer -d relayer" 60 2
    
    # Wait for LocalTerra (if using e2e profile)
    if docker compose ps --format json 2>/dev/null | grep -q "localterra"; then
        wait_for_service "LocalTerra" "curl -sf http://localhost:$E2E_TERRA_RPC_PORT/status" 120 5
    fi
    
    log_info "All services are healthy"
}

# Deploy EVM contracts
deploy_evm_contracts() {
    if [ "${E2E_SKIP_DEPLOY:-0}" = "1" ]; then
        log_warn "Skipping EVM contract deployment (E2E_SKIP_DEPLOY=1)"
        return 0
    fi
    
    log_step "Deploying EVM contracts to Anvil..."
    
    cd "$PROJECT_ROOT/packages/contracts-evm"
    
    # Use Anvil's default account for deployment
    export PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
    
    # Deploy the bridge contract
    DEPLOY_OUTPUT=$(forge script script/DeployLocal.s.sol:DeployLocal \
        --rpc-url http://localhost:8545 \
        --private-key "$PRIVATE_KEY" \
        --broadcast \
        2>&1) || {
        log_error "Failed to deploy EVM contracts"
        echo "$DEPLOY_OUTPUT"
        return 1
    }
    
    # Extract deployed addresses from output or broadcast files
    # The broadcast files are in broadcast/DeployLocal.s.sol/31337/run-latest.json
    BROADCAST_FILE="$PROJECT_ROOT/packages/contracts-evm/broadcast/DeployLocal.s.sol/31337/run-latest.json"
    
    if [ -f "$BROADCAST_FILE" ]; then
        # Extract bridge address from broadcast
        EVM_BRIDGE_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "CL8YBridge") | .contractAddress' "$BROADCAST_FILE" | head -1)
        
        if [ -n "$EVM_BRIDGE_ADDRESS" ] && [ "$EVM_BRIDGE_ADDRESS" != "null" ]; then
            log_info "Bridge deployed at: $EVM_BRIDGE_ADDRESS"
        else
            log_warn "Could not extract bridge address from broadcast file"
            # Try to get from recent deployments
            EVM_BRIDGE_ADDRESS=$(jq -r '.transactions[0].contractAddress' "$BROADCAST_FILE" 2>/dev/null || echo "")
        fi
    fi
    
    log_info "EVM contracts deployed successfully"
}

# Export environment variables for E2E tests
export_env_file() {
    log_step "Exporting E2E environment variables..."
    
    cat > "$ENV_FILE" << EOF
# E2E Test Environment
# Generated by e2e-setup.sh at $(date -Iseconds)

# EVM Configuration
EVM_RPC_URL=http://localhost:8545
EVM_CHAIN_ID=31337
EVM_BRIDGE_ADDRESS=${EVM_BRIDGE_ADDRESS:-}
EVM_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Terra Configuration
TERRA_RPC_URL=http://localhost:$E2E_TERRA_RPC_PORT
TERRA_LCD_URL=http://localhost:$E2E_TERRA_LCD_PORT
TERRA_CHAIN_ID=localterra
TERRA_BRIDGE_ADDRESS=${TERRA_BRIDGE_ADDRESS:-}

# Database Configuration
DATABASE_URL=postgres://relayer:relayer@localhost:$E2E_POSTGRES_PORT/relayer

# API Configuration
API_PORT=$E2E_API_PORT
API_URL=http://localhost:$E2E_API_PORT

# Test Accounts
EVM_TEST_ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
TERRA_KEY_NAME=test1
TERRA_TEST_ADDRESS=terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v
EOF

    log_info "Environment exported to $ENV_FILE"
}

# Fund test accounts
fund_test_accounts() {
    log_step "Funding test accounts..."
    
    # EVM accounts are automatically funded by Anvil with 10000 ETH
    local balance=$(cast balance 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url http://localhost:8545 2>/dev/null || echo "0")
    if [ "$balance" != "0" ]; then
        log_info "EVM test account funded with $(cast from-wei $balance) ETH"
    fi
    
    # Terra accounts would need to be funded via LocalTerra genesis or faucet
    # This is typically handled by LocalTerra's default configuration
    
    log_info "Test accounts ready"
}

# Verify setup
verify_setup() {
    log_step "Verifying E2E setup..."
    
    local failed=0
    
    # Check Anvil
    if cast block-number --rpc-url http://localhost:8545 &>/dev/null; then
        log_info "Anvil: OK (block $(cast block-number --rpc-url http://localhost:8545))"
    else
        log_error "Anvil: FAILED"
        failed=1
    fi
    
    # Check PostgreSQL
    if docker compose exec -T postgres pg_isready -U relayer -d relayer &>/dev/null; then
        log_info "PostgreSQL: OK"
    else
        log_error "PostgreSQL: FAILED"
        failed=1
    fi
    
    # Check env file
    if [ -f "$ENV_FILE" ]; then
        log_info "Environment file: $ENV_FILE"
    else
        log_error "Environment file not created"
        failed=1
    fi
    
    if [ $failed -eq 1 ]; then
        return 1
    fi
    
    log_info "Setup verification complete"
}

# Print summary
print_summary() {
    echo ""
    echo "========================================"
    echo "       E2E Setup Complete"
    echo "========================================"
    echo ""
    echo "Services running:"
    echo "  - Anvil (EVM): http://localhost:8545"
    echo "  - PostgreSQL: localhost:$E2E_POSTGRES_PORT"
    if docker compose ps --format json 2>/dev/null | grep -q "localterra"; then
        echo "  - LocalTerra RPC: http://localhost:$E2E_TERRA_RPC_PORT"
        echo "  - LocalTerra LCD: http://localhost:$E2E_TERRA_LCD_PORT"
    fi
    echo ""
    echo "Environment file: $ENV_FILE"
    echo ""
    echo "To run E2E tests:"
    echo "  source .env.e2e && ./scripts/e2e-test.sh"
    echo ""
    echo "To tear down:"
    echo "  ./scripts/e2e-teardown.sh"
    echo ""
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "     CL8Y Bridge E2E Setup"
    echo "========================================"
    echo ""
    
    # Set trap for cleanup on error
    trap cleanup_on_error ERR
    
    check_prerequisites
    cleanup_existing
    start_services
    wait_for_services
    deploy_evm_contracts
    export_env_file
    fund_test_accounts
    verify_setup
    
    # Remove trap on success
    trap - ERR
    
    print_summary
}

main "$@"
