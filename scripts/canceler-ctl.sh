#!/bin/bash
# Canceler Control Script
#
# Manages canceler node instances for development and testing.
# Supports running multiple canceler instances.
#
# Usage:
#   ./scripts/canceler-ctl.sh start [id]    # Start canceler (default id: 1)
#   ./scripts/canceler-ctl.sh stop [id]     # Stop canceler
#   ./scripts/canceler-ctl.sh restart [id]  # Restart canceler
#   ./scripts/canceler-ctl.sh status        # Check all canceler statuses
#   ./scripts/canceler-ctl.sh logs [id]     # Show canceler logs
#   ./scripts/canceler-ctl.sh stop-all      # Stop all cancelers
#   ./scripts/canceler-ctl.sh build         # Build canceler binary

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CANCELER_DIR="$PROJECT_ROOT/packages/canceler"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[CANCELER]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[CANCELER]${NC} $1"; }
log_error() { echo -e "${RED}[CANCELER]${NC} $1"; }

# Get files for a specific canceler instance
get_pid_file() { echo "$PROJECT_ROOT/.canceler-${1}.pid"; }
get_log_file() { echo "$PROJECT_ROOT/.canceler-${1}.log"; }

# Check if a canceler is running
is_running() {
    local id="${1:-1}"
    local pid_file
    pid_file=$(get_pid_file "$id")
    
    if [ -f "$pid_file" ]; then
        local pid
        pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            return 0
        else
            rm -f "$pid_file"
        fi
    fi
    return 1
}

# Get canceler PID
get_pid() {
    local id="${1:-1}"
    local pid_file
    pid_file=$(get_pid_file "$id")
    
    if [ -f "$pid_file" ]; then
        cat "$pid_file"
    fi
}

# Build canceler
build_canceler() {
    log_info "Building canceler..."
    cd "$CANCELER_DIR"
    cargo build --release 2>&1
    log_info "Build complete"
}

# Start canceler
start_canceler() {
    local id="${1:-1}"
    
    if is_running "$id"; then
        log_warn "Canceler $id already running (PID: $(get_pid "$id"))"
        return 0
    fi

    log_info "Starting canceler $id..."

    # Check if binary exists
    local binary="$CANCELER_DIR/target/release/cl8y-canceler"
    if [ ! -f "$binary" ]; then
        binary="$CANCELER_DIR/target/debug/cl8y-canceler"
    fi

    if [ ! -f "$binary" ]; then
        log_warn "Binary not found, building..."
        build_canceler
        binary="$CANCELER_DIR/target/release/cl8y-canceler"
    fi

    # Source environment if .env exists
    if [ -f "$PROJECT_ROOT/.env" ]; then
        set -a
        source "$PROJECT_ROOT/.env"
        set +a
    fi

    # Check required env vars
    local missing_vars=""
    [ -z "$EVM_RPC_URL" ] && missing_vars="$missing_vars EVM_RPC_URL"
    [ -z "$EVM_BRIDGE_ADDRESS" ] && missing_vars="$missing_vars EVM_BRIDGE_ADDRESS"
    [ -z "$TERRA_LCD_URL" ] && missing_vars="$missing_vars TERRA_LCD_URL"
    [ -z "$TERRA_BRIDGE_ADDRESS" ] && missing_vars="$missing_vars TERRA_BRIDGE_ADDRESS"
    
    if [ -n "$missing_vars" ]; then
        log_error "Missing environment variables:$missing_vars"
        log_error "Set them in .env or export before running"
        exit 1
    fi

    # Default values for optional vars
    [ -z "$EVM_CHAIN_ID" ] && export EVM_CHAIN_ID="31337"
    [ -z "$TERRA_CHAIN_ID" ] && export TERRA_CHAIN_ID="localterra"
    [ -z "$TERRA_RPC_URL" ] && export TERRA_RPC_URL="http://localhost:26657"
    [ -z "$POLL_INTERVAL_MS" ] && export POLL_INTERVAL_MS="5000"
    
    # Use different keys for different instances if provided
    local key_suffix=""
    [ "$id" != "1" ] && key_suffix="_$id"
    
    local evm_key_var="EVM_PRIVATE_KEY${key_suffix}"
    local terra_mnemonic_var="TERRA_MNEMONIC${key_suffix}"
    
    # Fall back to default if instance-specific not set
    [ -z "${!evm_key_var}" ] && evm_key_var="EVM_PRIVATE_KEY"
    [ -z "${!terra_mnemonic_var}" ] && terra_mnemonic_var="TERRA_MNEMONIC"
    
    export EVM_PRIVATE_KEY="${!evm_key_var}"
    export TERRA_MNEMONIC="${!terra_mnemonic_var}"
    
    if [ -z "$EVM_PRIVATE_KEY" ] || [ -z "$TERRA_MNEMONIC" ]; then
        log_error "EVM_PRIVATE_KEY and TERRA_MNEMONIC required"
        exit 1
    fi

    local pid_file log_file
    pid_file=$(get_pid_file "$id")
    log_file=$(get_log_file "$id")

    # Start in background
    cd "$PROJECT_ROOT"
    nohup "$binary" > "$log_file" 2>&1 &
    local pid=$!
    echo "$pid" > "$pid_file"

    # Wait and check if it started
    sleep 2
    if is_running "$id"; then
        log_info "Canceler $id started (PID: $pid)"
        log_info "Logs: $log_file"
    else
        log_error "Canceler $id failed to start. Check logs:"
        cat "$log_file" | tail -20
        exit 1
    fi
}

# Stop canceler
stop_canceler() {
    local id="${1:-1}"
    
    if ! is_running "$id"; then
        log_warn "Canceler $id not running"
        return 0
    fi

    local pid pid_file
    pid=$(get_pid "$id")
    pid_file=$(get_pid_file "$id")
    
    log_info "Stopping canceler $id (PID: $pid)..."

    kill -TERM "$pid" 2>/dev/null || true

    local count=0
    while kill -0 "$pid" 2>/dev/null && [ $count -lt 10 ]; do
        sleep 1
        count=$((count + 1))
    done

    if kill -0 "$pid" 2>/dev/null; then
        log_warn "Force killing canceler $id..."
        kill -KILL "$pid" 2>/dev/null || true
    fi

    rm -f "$pid_file"
    log_info "Canceler $id stopped"
}

# Stop all cancelers
stop_all_cancelers() {
    log_info "Stopping all cancelers..."
    
    for pid_file in "$PROJECT_ROOT"/.canceler-*.pid; do
        [ -f "$pid_file" ] || continue
        local id
        id=$(basename "$pid_file" | sed 's/.canceler-\([0-9]*\).pid/\1/')
        stop_canceler "$id"
    done
}

# Show status of all cancelers
show_status() {
    local running=0
    local stopped=0
    
    echo "Canceler Status:"
    echo "----------------"
    
    # Check for running instances
    for pid_file in "$PROJECT_ROOT"/.canceler-*.pid; do
        [ -f "$pid_file" ] || continue
        local id pid
        id=$(basename "$pid_file" | sed 's/.canceler-\([0-9]*\).pid/\1/')
        
        if is_running "$id"; then
            pid=$(get_pid "$id")
            echo -e "  ${GREEN}●${NC} Canceler $id: running (PID: $pid)"
            running=$((running + 1))
        else
            echo -e "  ${RED}○${NC} Canceler $id: stopped (stale PID file)"
            stopped=$((stopped + 1))
        fi
    done
    
    if [ $running -eq 0 ] && [ $stopped -eq 0 ]; then
        echo "  No cancelers configured"
    fi
    
    echo ""
    echo "Summary: $running running, $stopped stopped"
    
    return $((running > 0 ? 0 : 1))
}

# Show logs
show_logs() {
    local id="${1:-1}"
    local lines="${2:-50}"
    local log_file
    log_file=$(get_log_file "$id")
    
    if [ -f "$log_file" ]; then
        tail -n "$lines" "$log_file"
    else
        log_warn "No log file for canceler $id"
    fi
}

# Follow logs
follow_logs() {
    local id="${1:-1}"
    local log_file
    log_file=$(get_log_file "$id")
    
    if [ -f "$log_file" ]; then
        tail -f "$log_file"
    else
        log_warn "No log file for canceler $id"
    fi
}

# Main
case "${1:-}" in
    start)
        start_canceler "${2:-1}"
        ;;
    stop)
        stop_canceler "${2:-1}"
        ;;
    restart)
        stop_canceler "${2:-1}"
        sleep 1
        start_canceler "${2:-1}"
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs "${2:-1}" "${3:-50}"
        ;;
    logs-f|follow)
        follow_logs "${2:-1}"
        ;;
    stop-all)
        stop_all_cancelers
        ;;
    build)
        build_canceler
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|logs|logs-f|stop-all|build} [id]"
        echo ""
        echo "Commands:"
        echo "  start [id]    Start canceler instance (default: 1)"
        echo "  stop [id]     Stop canceler instance"
        echo "  restart [id]  Restart canceler instance"
        echo "  status        Show status of all cancelers"
        echo "  logs [id]     Show last 50 lines of logs"
        echo "  logs-f [id]   Follow logs (tail -f)"
        echo "  stop-all      Stop all canceler instances"
        echo "  build         Build canceler binary"
        echo ""
        echo "Multiple Instances:"
        echo "  ./scripts/canceler-ctl.sh start 1   # First canceler"
        echo "  ./scripts/canceler-ctl.sh start 2   # Second canceler"
        echo "  Set EVM_PRIVATE_KEY_2, TERRA_MNEMONIC_2 for instance 2"
        exit 1
        ;;
esac
