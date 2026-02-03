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
    wait_for_service "PostgreSQL" "docker compose exec -T postgres pg_isready -U operator -d operator" 60 2
    
    # Wait for LocalTerra (if running)
    if docker compose ps --format json 2>/dev/null | grep -q "localterra"; then
        wait_for_service "LocalTerra" "curl -sf http://localhost:$E2E_TERRA_RPC_PORT/status" 180 5
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
        EVM_BRIDGE_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "Cl8YBridge") | .contractAddress' "$BROADCAST_FILE" | head -1)
        ACCESS_MANAGER_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "AccessManagerEnumerable") | .contractAddress' "$BROADCAST_FILE" | head -1)
        CHAIN_REGISTRY_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "ChainRegistry") | .contractAddress' "$BROADCAST_FILE" | head -1)
        TOKEN_REGISTRY_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "TokenRegistry") | .contractAddress' "$BROADCAST_FILE" | head -1)
        LOCK_UNLOCK_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "LockUnlock") | .contractAddress' "$BROADCAST_FILE" | head -1)
        MINT_BURN_ADDRESS=$(jq -r '.transactions[] | select(.contractName == "MintBurn") | .contractAddress' "$BROADCAST_FILE" | head -1)
        
        if [ -n "$EVM_BRIDGE_ADDRESS" ] && [ "$EVM_BRIDGE_ADDRESS" != "null" ]; then
            log_info "Bridge deployed at: $EVM_BRIDGE_ADDRESS"
        else
            log_warn "Could not extract bridge address from broadcast file"
            # Fallback to known deterministic address
            EVM_BRIDGE_ADDRESS="0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"
        fi
    else
        # Use deterministic addresses from Anvil
        log_info "Using deterministic Anvil addresses"
        ACCESS_MANAGER_ADDRESS="0x5FbDB2315678afecb367f032d93F642f64180aa3"
        CHAIN_REGISTRY_ADDRESS="0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"
        TOKEN_REGISTRY_ADDRESS="0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
        MINT_BURN_ADDRESS="0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9"
        LOCK_UNLOCK_ADDRESS="0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9"
        EVM_BRIDGE_ADDRESS="0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"
    fi
    
    log_info "EVM contracts deployed successfully"
}

# Run database migrations
run_database_migrations() {
    log_step "Running database migrations..."
    
    cd "$PROJECT_ROOT"
    
    # Wait a moment for postgres to be fully ready
    sleep 3
    
    # Determine the postgres port - use E2E port if set, otherwise default 5433
    local PG_PORT="${E2E_POSTGRES_PORT:-${POSTGRES_PORT:-5433}}"
    
    log_info "Using postgres port: $PG_PORT"
    
    # Prefer docker exec as it's most reliable
    if docker ps --format '{{.Names}}' | grep -q "cl8y-bridge-monorepo-postgres-1"; then
        run_migrations_docker
    elif command -v sqlx &> /dev/null; then
        sqlx migrate run --source packages/operator/migrations \
            --database-url "postgres://operator:operator@localhost:$PG_PORT/operator" 2>&1 || {
            log_warn "sqlx migrate failed, trying psql..."
            run_migrations_psql "$PG_PORT"
        }
    elif command -v psql &> /dev/null; then
        run_migrations_psql "$PG_PORT"
    else
        log_warn "No postgres client available, skipping migrations"
    fi
    
    log_info "Database migrations complete"
}

# Run migrations using psql directly
run_migrations_psql() {
    local PG_PORT="${1:-5433}"
    log_info "Running migrations with psql on port $PG_PORT..."
    
    # Concatenate and run all migration files
    for migration in "$PROJECT_ROOT"/packages/operator/migrations/*.sql; do
        if [ -f "$migration" ]; then
            PGPASSWORD=operator psql -h localhost -p "$PG_PORT" -U operator -d operator -f "$migration" 2>&1 || true
        fi
    done
}

# Run migrations using docker exec
run_migrations_docker() {
    log_info "Running migrations via docker exec..."
    
    # Copy migrations to container and run
    docker cp "$PROJECT_ROOT/packages/operator/migrations" cl8y-bridge-monorepo-postgres-1:/tmp/migrations
    
    for migration in 001_initial.sql 002_retry_after.sql 003_evm_to_evm.sql; do
        if docker exec cl8y-bridge-monorepo-postgres-1 test -f "/tmp/migrations/$migration"; then
            docker exec cl8y-bridge-monorepo-postgres-1 psql -U operator -d operator -f "/tmp/migrations/$migration" 2>&1 || true
        fi
    done
}

# Deploy Terra bridge contract
deploy_terra_contracts() {
    if [ "${E2E_SKIP_DEPLOY:-0}" = "1" ]; then
        log_warn "Skipping Terra contract deployment (E2E_SKIP_DEPLOY=1)"
        return 0
    fi
    
    log_step "Deploying Terra bridge contract to LocalTerra..."
    
    local CONTAINER_NAME="cl8y-bridge-monorepo-localterra-1"
    local WASM_PATH="$PROJECT_ROOT/packages/contracts-terraclassic/artifacts/bridge.wasm"
    local TEST_ADDRESS="terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"
    
    # Check if LocalTerra is running
    if ! docker ps --format '{{.Names}}' | grep -q "$CONTAINER_NAME"; then
        log_warn "LocalTerra container not running, skipping Terra deployment"
        return 0
    fi
    
    # Check if WASM exists
    if [ ! -f "$WASM_PATH" ]; then
        log_warn "Terra bridge WASM not found at $WASM_PATH"
        log_info "Build with: cd packages/contracts-terraclassic && cargo build --release --target wasm32-unknown-unknown"
        return 0
    fi
    
    # Check if contract already deployed (code_id 1 exists with contracts)
    local EXISTING_CONTRACT=$(docker exec "$CONTAINER_NAME" terrad query wasm list-contract-by-code 1 -o json 2>/dev/null | jq -r '.contracts[0] // empty' || echo "")
    
    if [ -n "$EXISTING_CONTRACT" ]; then
        log_info "Terra bridge already deployed at: $EXISTING_CONTRACT"
        TERRA_BRIDGE_ADDRESS="$EXISTING_CONTRACT"
        configure_terra_bridge
        return 0
    fi
    
    # Check if code already stored
    local CODE_ID=$(docker exec "$CONTAINER_NAME" terrad query wasm list-code -o json 2>/dev/null | jq -r '.code_infos[-1].code_id // empty' || echo "")
    
    if [ -z "$CODE_ID" ]; then
        # Copy WASM to container and store
        log_info "Copying WASM to container..."
        docker exec "$CONTAINER_NAME" mkdir -p /tmp/wasm
        docker cp "$WASM_PATH" "$CONTAINER_NAME:/tmp/wasm/bridge.wasm"
        
        log_info "Storing bridge contract..."
        local STORE_TX=$(docker exec "$CONTAINER_NAME" terrad tx wasm store /tmp/wasm/bridge.wasm \
            --from test1 \
            --chain-id localterra \
            --gas auto --gas-adjustment 1.5 \
            --fees 200000000uluna \
            --broadcast-mode sync \
            -y -o json --keyring-backend test 2>&1)
        
        # Extract txhash - handle various JSON response formats
        local TX_HASH=$(echo "$STORE_TX" | grep -o '"txhash":"[^"]*"' | cut -d'"' -f4 || echo "")
        if [ -z "$TX_HASH" ]; then
            # Try jq as fallback
            TX_HASH=$(echo "$STORE_TX" | jq -r '.txhash' 2>/dev/null || echo "")
        fi
        
        if [ -z "$TX_HASH" ] || [ "$TX_HASH" = "null" ]; then
            log_error "Failed to store Terra contract: $STORE_TX"
            return 1
        fi
        
        log_info "Store TX: $TX_HASH"
        log_info "Waiting for confirmation..."
        sleep 10
        
        CODE_ID=$(docker exec "$CONTAINER_NAME" terrad query wasm list-code -o json | jq -r '.code_infos[-1].code_id')
    fi
    
    log_info "Using code ID: $CODE_ID"
    
    # Instantiate the contract
    log_info "Instantiating bridge contract..."
    
    local INIT_MSG='{"admin":"'$TEST_ADDRESS'","operators":["'$TEST_ADDRESS'"],"min_signatures":1,"min_bridge_amount":"1000000","max_bridge_amount":"1000000000000000","fee_bps":30,"fee_collector":"'$TEST_ADDRESS'"}'
    
    local INST_TX=$(docker exec "$CONTAINER_NAME" terrad tx wasm instantiate "$CODE_ID" "$INIT_MSG" \
        --label "cl8y-bridge-e2e" \
        --admin "$TEST_ADDRESS" \
        --from test1 \
        --chain-id localterra \
        --gas auto --gas-adjustment 1.5 \
        --fees 50000000uluna \
        --broadcast-mode sync \
        -y -o json --keyring-backend test 2>&1)
    
    # Extract txhash - handle various JSON response formats
    local TX_HASH=$(echo "$INST_TX" | grep -o '"txhash":"[^"]*"' | cut -d'"' -f4 || echo "")
    if [ -z "$TX_HASH" ]; then
        TX_HASH=$(echo "$INST_TX" | jq -r '.txhash' 2>/dev/null || echo "")
    fi
    
    if [ -z "$TX_HASH" ] || [ "$TX_HASH" = "null" ]; then
        log_error "Failed to instantiate Terra contract: $INST_TX"
        return 1
    fi
    
    log_info "Instantiate TX: $TX_HASH"
    log_info "Waiting for confirmation..."
    sleep 8
    
    # Get contract address
    TERRA_BRIDGE_ADDRESS=$(docker exec "$CONTAINER_NAME" terrad query wasm list-contract-by-code "$CODE_ID" -o json | jq -r '.contracts[-1]')
    
    if [ -z "$TERRA_BRIDGE_ADDRESS" ] || [ "$TERRA_BRIDGE_ADDRESS" = "null" ]; then
        log_error "Failed to get Terra bridge address"
        return 1
    fi
    
    log_info "Terra bridge deployed at: $TERRA_BRIDGE_ADDRESS"
    
    # Configure the bridge
    configure_terra_bridge
}

# Configure Terra bridge (set withdraw delay)
configure_terra_bridge() {
    log_step "Configuring Terra bridge..."
    
    local CONTAINER_NAME="cl8y-bridge-monorepo-localterra-1"
    
    if [ -z "$TERRA_BRIDGE_ADDRESS" ]; then
        log_warn "TERRA_BRIDGE_ADDRESS not set, skipping configuration"
        return 0
    fi
    
    # Set withdraw delay to 300 seconds to match EVM
    log_info "Setting withdraw delay to 300 seconds..."
    local SET_DELAY_MSG='{"set_withdraw_delay":{"delay_seconds":300}}'
    
    docker exec "$CONTAINER_NAME" terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$SET_DELAY_MSG" \
        --from test1 \
        --chain-id localterra \
        --gas auto --gas-adjustment 1.5 \
        --fees 10000000uluna \
        --broadcast-mode sync \
        -y --keyring-backend test 2>&1 > /dev/null || true
    
    sleep 5
    
    # Verify configuration
    local QUERY='{"withdraw_delay":{}}'
    local QUERY_B64=$(echo -n "$QUERY" | base64 -w0)
    local DELAY=$(curl -sf "http://localhost:1317/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE_ADDRESS}/smart/${QUERY_B64}" 2>/dev/null | jq -r '.data.delay_seconds // empty' || echo "")
    
    if [ "$DELAY" = "300" ]; then
        log_info "Terra bridge configured: withdraw delay = 300s"
    else
        log_warn "Could not verify Terra bridge configuration (delay=$DELAY)"
    fi
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
EVM_BRIDGE_ADDRESS=${EVM_BRIDGE_ADDRESS:-0x5FC8d32690cc91D4c39d9d3abcBD16989F875707}
EVM_ROUTER_ADDRESS=${EVM_BRIDGE_ADDRESS:-0x5FC8d32690cc91D4c39d9d3abcBD16989F875707}
ACCESS_MANAGER_ADDRESS=${ACCESS_MANAGER_ADDRESS:-0x5FbDB2315678afecb367f032d93F642f64180aa3}
CHAIN_REGISTRY_ADDRESS=${CHAIN_REGISTRY_ADDRESS:-0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512}
TOKEN_REGISTRY_ADDRESS=${TOKEN_REGISTRY_ADDRESS:-0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0}
LOCK_UNLOCK_ADDRESS=${LOCK_UNLOCK_ADDRESS:-0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9}
MINT_BURN_ADDRESS=${MINT_BURN_ADDRESS:-0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9}
EVM_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Terra Configuration
TERRA_RPC_URL=http://localhost:$E2E_TERRA_RPC_PORT
TERRA_LCD_URL=http://localhost:$E2E_TERRA_LCD_PORT
TERRA_CHAIN_ID=localterra
TERRA_BRIDGE_ADDRESS=${TERRA_BRIDGE_ADDRESS:-}

# Database Configuration
DATABASE_URL=postgres://operator:operator@localhost:${E2E_POSTGRES_PORT:-5433}/operator

# API Configuration
API_PORT=$E2E_API_PORT
API_URL=http://localhost:$E2E_API_PORT

# Test Accounts
EVM_TEST_ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
TERRA_KEY_NAME=test1
TERRA_TEST_ADDRESS=terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v
EOF

    log_info "Environment exported to $ENV_FILE"
    
    # Also create .env.local for convenience
    cp "$ENV_FILE" "$PROJECT_ROOT/.env.local"
    log_info "Also copied to .env.local"
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
    if docker exec cl8y-bridge-monorepo-postgres-1 pg_isready -U operator -d operator &>/dev/null; then
        log_info "PostgreSQL: OK"
    else
        log_error "PostgreSQL: FAILED"
        failed=1
    fi
    
    # Check database tables via docker exec
    local TABLE_COUNT=$(docker exec cl8y-bridge-monorepo-postgres-1 psql -U operator -d operator -t -c \
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'" 2>/dev/null | tr -d ' ' || echo "0")
    # Handle empty or non-numeric
    if [[ "$TABLE_COUNT" =~ ^[0-9]+$ ]] && [ "$TABLE_COUNT" -gt 0 ]; then
        log_info "Database tables: $TABLE_COUNT tables"
    else
        log_warn "Database tables: 0 (migrations may have failed)"
    fi
    
    # Check EVM bridge
    if [ -n "$EVM_BRIDGE_ADDRESS" ]; then
        local DELAY=$(cast call "$EVM_BRIDGE_ADDRESS" "withdrawDelay()" --rpc-url http://localhost:8545 2>/dev/null | cast to-dec 2>/dev/null || echo "0")
        if [ "$DELAY" -gt 0 ]; then
            log_info "EVM Bridge: OK ($EVM_BRIDGE_ADDRESS, delay=${DELAY}s)"
        else
            log_warn "EVM Bridge: Could not verify"
        fi
    fi
    
    # Check Terra bridge
    if [ -n "$TERRA_BRIDGE_ADDRESS" ]; then
        local QUERY='{"config":{}}'
        local QUERY_B64=$(echo -n "$QUERY" | base64 -w0)
        if curl -sf "http://localhost:1317/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE_ADDRESS}/smart/${QUERY_B64}" &>/dev/null; then
            log_info "Terra Bridge: OK ($TERRA_BRIDGE_ADDRESS)"
        else
            log_warn "Terra Bridge: Could not verify"
        fi
    else
        log_warn "Terra Bridge: Not deployed"
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
    echo "  - PostgreSQL: localhost:5433"
    if docker compose ps --format json 2>/dev/null | grep -q "localterra"; then
        echo "  - LocalTerra RPC: http://localhost:$E2E_TERRA_RPC_PORT"
        echo "  - LocalTerra LCD: http://localhost:$E2E_TERRA_LCD_PORT"
    fi
    echo ""
    echo "Contract Addresses:"
    echo "  - EVM Bridge: ${EVM_BRIDGE_ADDRESS:-not deployed}"
    echo "  - Terra Bridge: ${TERRA_BRIDGE_ADDRESS:-not deployed}"
    echo ""
    echo "Environment files:"
    echo "  - $ENV_FILE"
    echo "  - $PROJECT_ROOT/.env.local"
    echo ""
    echo "To run E2E tests:"
    echo "  source .env.e2e && ./scripts/e2e-test.sh"
    echo "  # or"
    echo "  source .env.local && ./scripts/e2e-test.sh --full"
    echo ""
    echo "To tear down:"
    echo "  ./scripts/e2e-teardown.sh"
    echo "  # or"
    echo "  docker compose down -v"
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
    run_database_migrations
    deploy_evm_contracts
    deploy_terra_contracts
    export_env_file
    fund_test_accounts
    verify_setup
    
    # Remove trap on success
    trap - ERR
    
    print_summary
}

main "$@"
