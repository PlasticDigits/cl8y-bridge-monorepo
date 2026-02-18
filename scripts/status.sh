#!/bin/bash
# Status check script for all CL8Y Bridge services
#
# Checks the health of:
# - Anvil (EVM local chain)
# - LocalTerra (Terra Classic local chain)
# - PostgreSQL database
# - Operator service (if running)
# - Canceler service (if running)
# - Frontend (if running)
# - Contract addresses (loaded from .env.e2e.local, verified on-chain)
#
# Usage:
#   ./scripts/status.sh
#   ./scripts/status.sh --json   # Output as JSON

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Source .env.e2e.local if it exists (provides contract addresses, bridge vars, etc.)
if [ -f "$PROJECT_ROOT/.env.e2e.local" ]; then
    set -a
    source "$PROJECT_ROOT/.env.e2e.local"
    set +a
fi

# Map VITE_ vars to the names used by scripts/operator/canceler
[ -n "${VITE_EVM_BRIDGE_ADDRESS:-}" ] && EVM_BRIDGE_ADDRESS="${EVM_BRIDGE_ADDRESS:-$VITE_EVM_BRIDGE_ADDRESS}"
[ -n "${VITE_TERRA_BRIDGE_ADDRESS:-}" ] && TERRA_BRIDGE_ADDRESS="${TERRA_BRIDGE_ADDRESS:-$VITE_TERRA_BRIDGE_ADDRESS}"

# Configuration
EVM_RPC_URL="${EVM_RPC_URL:-http://localhost:8545}"
TERRA_RPC_URL="${TERRA_RPC_URL:-http://localhost:26657}"
TERRA_LCD_URL="${TERRA_LCD_URL:-http://localhost:1317}"
DATABASE_URL="${DATABASE_URL:-postgres://operator:operator@localhost:5433/operator}"
OPERATOR_API_URL="${OPERATOR_API_URL:-http://localhost:9092}"
CANCELER_HEALTH_URL="${CANCELER_HEALTH_URL:-http://localhost:9099}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:5173}"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# Output format
JSON_OUTPUT=false
if [ "$1" = "--json" ]; then
    JSON_OUTPUT=true
fi

# Service status tracking
declare -A SERVICE_STATUS

log_status() {
    local service="$1"
    local status="$2"
    local details="${3:-}"
    
    SERVICE_STATUS["$service"]="$status"
    
    if [ "$JSON_OUTPUT" = false ]; then
        local color=$RED
        if [ "$status" = "running" ]; then
            color=$GREEN
        elif [ "$status" = "partial" ]; then
            color=$YELLOW
        fi
        
        printf "  %-15s " "$service:"
        echo -e "${color}$status${NC} $details"
    fi
}

# Check Anvil (EVM)
check_anvil() {
    local block_number
    block_number=$(cast block-number --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "")
    
    if [ -n "$block_number" ]; then
        log_status "Anvil" "running" "(block $block_number)"
        return 0
    else
        log_status "Anvil" "stopped"
        return 1
    fi
}

# Check LocalTerra
check_localterra() {
    local status
    status=$(curl -sf "$TERRA_RPC_URL/status" 2>/dev/null | jq -r '.result.sync_info.latest_block_height' 2>/dev/null || echo "")
    
    if [ -n "$status" ]; then
        log_status "LocalTerra" "running" "(block $status)"
        return 0
    else
        log_status "LocalTerra" "stopped"
        return 1
    fi
}

# Check PostgreSQL
check_postgres() {
    if command -v psql &> /dev/null; then
        if psql "$DATABASE_URL" -c "SELECT 1" &> /dev/null; then
            local tables
            tables=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'" 2>/dev/null | tr -d ' ')
            log_status "PostgreSQL" "running" "($tables tables)"
            return 0
        fi
    fi
    
    # Fallback: Check if Docker container is running
    if docker compose ps 2>/dev/null | grep -q "postgres.*running"; then
        log_status "PostgreSQL" "running" "(docker)"
        return 0
    fi
    
    log_status "PostgreSQL" "stopped"
    return 1
}

# Check Operator
check_operator() {
    if curl -sf "$OPERATOR_API_URL/health" &> /dev/null; then
        log_status "Operator" "running"
        return 0
    elif pgrep -f "cl8y-relayer\|cl8y-operator" &> /dev/null; then
        log_status "Operator" "running" "(no API)"
        return 0
    else
        log_status "Operator" "stopped"
        return 1
    fi
}

# Check Canceler
check_canceler() {
    # Try the configured health URL first
    local health_response
    health_response=$(curl -sf "$CANCELER_HEALTH_URL/health" 2>/dev/null || echo "")
    if [ -n "$health_response" ]; then
        local canceler_id
        canceler_id=$(echo "$health_response" | jq -r '.canceler_id // empty' 2>/dev/null || echo "")
        if [ -n "$canceler_id" ]; then
            log_status "Canceler" "running" "($canceler_id)"
        else
            log_status "Canceler" "running"
        fi
        return 0
    fi

    # Check common canceler health ports (9099 default, 9200 override)
    for port in 9099 9200; do
        if [ "http://localhost:$port" != "$CANCELER_HEALTH_URL" ]; then
            health_response=$(curl -sf "http://localhost:$port/health" 2>/dev/null || echo "")
            if [ -n "$health_response" ]; then
                log_status "Canceler" "running" "(port $port)"
                return 0
            fi
        fi
    done

    if pgrep -f "cl8y-canceler" &> /dev/null; then
        log_status "Canceler" "running" "(no health)"
        return 0
    else
        log_status "Canceler" "stopped"
        return 1
    fi
}

# Check Frontend
check_frontend() {
    if curl -sf "$FRONTEND_URL" &> /dev/null; then
        log_status "Frontend" "running"
        return 0
    elif pgrep -f "vite.*frontend" &> /dev/null; then
        log_status "Frontend" "running" "(starting)"
        return 0
    else
        log_status "Frontend" "stopped"
        return 1
    fi
}

# Check Docker services
check_docker() {
    if ! docker info &> /dev/null; then
        log_status "Docker" "stopped"
        return 1
    fi
    
    cd "$PROJECT_ROOT"
    local running
    running=$(docker compose ps --format json 2>/dev/null | jq -s 'map(select(.State == "running")) | length' 2>/dev/null || echo "0")
    
    if [ "$running" -gt 0 ]; then
        log_status "Docker" "running" "($running containers)"
        return 0
    else
        log_status "Docker" "stopped" "(no containers)"
        return 1
    fi
}

# Verify contract addresses on-chain and display them
get_contracts() {
    if [ "$JSON_OUTPUT" = false ]; then
        echo ""
        echo -e "${BLUE}Contract Addresses:${NC}"

        if [ -n "${EVM_BRIDGE_ADDRESS:-}" ]; then
            # Verify the contract exists on-chain by checking code size
            local code_size
            code_size=$(cast codesize "$EVM_BRIDGE_ADDRESS" --rpc-url "$EVM_RPC_URL" 2>/dev/null || echo "0")
            if [ "$code_size" -gt 0 ] 2>/dev/null; then
                echo -e "  EVM Bridge:   $EVM_BRIDGE_ADDRESS ${GREEN}(verified on-chain)${NC}"
            else
                echo -e "  EVM Bridge:   $EVM_BRIDGE_ADDRESS ${RED}(NOT found on-chain)${NC}"
            fi
        else
            echo "  EVM Bridge:   (not set)"
        fi

        if [ -n "${TERRA_BRIDGE_ADDRESS:-}" ]; then
            local terra_info
            terra_info=$(curl -sf "${TERRA_LCD_URL}/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE_ADDRESS}" 2>/dev/null | jq -r '.contract_info.label // empty' 2>/dev/null || echo "")
            if [ -n "$terra_info" ]; then
                echo -e "  Terra Bridge: $TERRA_BRIDGE_ADDRESS ${GREEN}(verified: $terra_info)${NC}"
            else
                echo -e "  Terra Bridge: $TERRA_BRIDGE_ADDRESS ${YELLOW}(could not verify)${NC}"
            fi
        else
            echo "  Terra Bridge: (not set)"
        fi

        if [ -f "$PROJECT_ROOT/.env.e2e.local" ]; then
            echo -e "  ${BLUE}Source:${NC}       .env.e2e.local"
        fi
    fi
}

# Output JSON
output_json() {
    echo "{"
    echo "  \"services\": {"
    local first=true
    for service in "${!SERVICE_STATUS[@]}"; do
        if [ "$first" = false ]; then
            echo ","
        fi
        first=false
        echo -n "    \"$(echo "$service" | tr '[:upper:]' '[:lower:]')\": \"${SERVICE_STATUS[$service]}\""
    done
    echo ""
    echo "  },"
    echo "  \"contracts\": {"
    echo "    \"evm_bridge\": \"${EVM_BRIDGE_ADDRESS:-}\","
    echo "    \"terra_bridge\": \"${TERRA_BRIDGE_ADDRESS:-}\""
    echo "  }"
    echo "}"
}

# Main
main() {
    if [ "$JSON_OUTPUT" = false ]; then
        echo ""
        echo "========================================"
        echo "    CL8Y Bridge Service Status"
        echo "========================================"
        echo ""
        echo -e "${BLUE}Infrastructure:${NC}"
    fi
    
    check_docker || true
    check_anvil || true
    check_localterra || true
    check_postgres || true
    
    if [ "$JSON_OUTPUT" = false ]; then
        echo ""
        echo -e "${BLUE}Applications:${NC}"
    fi
    
    check_operator || true
    check_canceler || true
    check_frontend || true
    
    get_contracts
    
    if [ "$JSON_OUTPUT" = true ]; then
        output_json
    else
        echo ""
    fi
}

main "$@"
