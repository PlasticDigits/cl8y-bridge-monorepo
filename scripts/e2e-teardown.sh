#!/bin/bash
# E2E Teardown Script
#
# This script cleans up the E2E testing environment:
# 1. Stops all E2E Docker services
# 2. Removes test data and volumes
# 3. Cleans up temporary files
# 4. Reports any orphaned processes
#
# Usage:
#   ./scripts/e2e-teardown.sh
#
# Options:
#   --keep-volumes  Don't remove Docker volumes (faster restarts)
#   --force         Force stop without graceful shutdown

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$PROJECT_ROOT/.env.e2e"

# Parse arguments
KEEP_VOLUMES=0
FORCE_STOP=0
for arg in "$@"; do
    case $arg in
        --keep-volumes) KEEP_VOLUMES=1 ;;
        --force) FORCE_STOP=1 ;;
    esac
done

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

# Stop any running relayer processes
stop_relayer() {
    log_step "Stopping relayer processes..."
    
    # Find and kill any relayer processes
    pkill -f "cl8y-relayer" 2>/dev/null && log_info "Stopped relayer" || true
    
    # Also check for cargo run of relayer
    pkill -f "target/.*relayer" 2>/dev/null || true
}

# Stop Docker services
stop_services() {
    log_step "Stopping Docker services..."
    
    cd "$PROJECT_ROOT"
    
    if [ $FORCE_STOP -eq 1 ]; then
        log_warn "Force stopping services..."
        docker compose --profile e2e kill 2>/dev/null || true
    fi
    
    if [ $KEEP_VOLUMES -eq 1 ]; then
        log_info "Keeping volumes for faster restart"
        docker compose --profile e2e down --remove-orphans 2>/dev/null || true
    else
        docker compose --profile e2e down -v --remove-orphans 2>/dev/null || true
    fi
    
    log_info "Docker services stopped"
}

# Clean up temporary files
cleanup_files() {
    log_step "Cleaning up temporary files..."
    
    # Remove environment file
    if [ -f "$ENV_FILE" ]; then
        rm -f "$ENV_FILE"
        log_info "Removed $ENV_FILE"
    fi
    
    # Clean up any test artifacts
    rm -f "$PROJECT_ROOT"/.e2e-* 2>/dev/null || true
    rm -f "$PROJECT_ROOT"/e2e-*.log 2>/dev/null || true
    
    log_info "Temporary files cleaned"
}

# Check for orphaned processes
check_orphans() {
    log_step "Checking for orphaned processes..."
    
    local orphans=0
    
    # Check for leftover anvil processes
    if pgrep -f "anvil.*8545" > /dev/null 2>&1; then
        log_warn "Found orphaned Anvil process"
        orphans=1
    fi
    
    # Check for leftover relayer processes  
    if pgrep -f "cl8y-relayer" > /dev/null 2>&1; then
        log_warn "Found orphaned relayer process"
        orphans=1
    fi
    
    # Check for processes on E2E ports
    for port in 18545 15433 19090; do
        if lsof -i :$port > /dev/null 2>&1; then
            log_warn "Port $port still in use"
            orphans=1
        fi
    done
    
    if [ $orphans -eq 0 ]; then
        log_info "No orphaned processes found"
    else
        log_warn "Some processes may still be running. Use 'docker compose down' or kill them manually."
    fi
}

# Print summary
print_summary() {
    echo ""
    echo "========================================"
    echo "       E2E Teardown Complete"
    echo "========================================"
    echo ""
    echo "Cleaned up:"
    echo "  - Docker E2E services stopped"
    if [ $KEEP_VOLUMES -eq 0 ]; then
        echo "  - Docker volumes removed"
    fi
    echo "  - Temporary files removed"
    echo ""
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "     CL8Y Bridge E2E Teardown"
    echo "========================================"
    echo ""
    
    stop_relayer
    stop_services
    cleanup_files
    check_orphans
    
    print_summary
}

main "$@"
