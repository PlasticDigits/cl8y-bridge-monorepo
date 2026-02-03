#!/bin/bash
# Operator Control Script
#
# Manages the bridge operator service for development and testing.
#
# Usage:
#   ./scripts/operator-ctl.sh start    # Start operator in background
#   ./scripts/operator-ctl.sh stop     # Stop operator
#   ./scripts/operator-ctl.sh restart  # Restart operator
#   ./scripts/operator-ctl.sh status   # Check if operator is running
#   ./scripts/operator-ctl.sh logs     # Show operator logs
#   ./scripts/operator-ctl.sh build    # Build operator binary

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OPERATOR_DIR="$PROJECT_ROOT/packages/operator"
PID_FILE="$PROJECT_ROOT/.operator.pid"
LOG_FILE="$PROJECT_ROOT/.operator.log"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[OPERATOR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[OPERATOR]${NC} $1"; }
log_error() { echo -e "${RED}[OPERATOR]${NC} $1"; }

# Check if operator is running
is_running() {
    if [ -f "$PID_FILE" ]; then
        local pid
        pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            return 0
        else
            # Stale PID file
            rm -f "$PID_FILE"
        fi
    fi
    return 1
}

# Get operator PID
get_pid() {
    if [ -f "$PID_FILE" ]; then
        cat "$PID_FILE"
    fi
}

# Build operator
build_operator() {
    log_info "Building operator..."
    cd "$OPERATOR_DIR"
    cargo build --release 2>&1
    log_info "Build complete"
}

# Start operator
start_operator() {
    if is_running; then
        log_warn "Operator already running (PID: $(get_pid))"
        return 0
    fi

    log_info "Starting operator..."

    # Check if binary exists (binary is named cl8y-relayer)
    local binary="$OPERATOR_DIR/target/release/cl8y-relayer"
    if [ ! -f "$binary" ]; then
        log_warn "Binary not found, building..."
        build_operator
    fi

    # Check if debug binary exists instead
    if [ ! -f "$binary" ]; then
        binary="$OPERATOR_DIR/target/debug/cl8y-relayer"
    fi

    if [ ! -f "$binary" ]; then
        log_error "Operator binary not found. Run: ./scripts/operator-ctl.sh build"
        exit 1
    fi

    # Source environment if .env exists
    if [ -f "$PROJECT_ROOT/.env" ]; then
        set -a
        source "$PROJECT_ROOT/.env"
        set +a
    fi

    # Check required env vars
    if [ -z "$DATABASE_URL" ]; then
        export DATABASE_URL="postgres://operator:operator@localhost:5433/operator"
        log_warn "Using default DATABASE_URL"
    fi

    # Start in background
    cd "$PROJECT_ROOT"
    nohup "$binary" > "$LOG_FILE" 2>&1 &
    local pid=$!
    echo "$pid" > "$PID_FILE"

    # Wait a moment and check if it started
    sleep 2
    if is_running; then
        log_info "Operator started (PID: $pid)"
        log_info "Logs: $LOG_FILE"
    else
        log_error "Operator failed to start. Check logs: $LOG_FILE"
        cat "$LOG_FILE" | tail -20
        exit 1
    fi
}

# Stop operator
stop_operator() {
    if ! is_running; then
        log_warn "Operator not running"
        return 0
    fi

    local pid
    pid=$(get_pid)
    log_info "Stopping operator (PID: $pid)..."

    # Send SIGTERM for graceful shutdown
    kill -TERM "$pid" 2>/dev/null || true

    # Wait for process to exit
    local count=0
    while kill -0 "$pid" 2>/dev/null && [ $count -lt 10 ]; do
        sleep 1
        count=$((count + 1))
    done

    # Force kill if still running
    if kill -0 "$pid" 2>/dev/null; then
        log_warn "Force killing operator..."
        kill -KILL "$pid" 2>/dev/null || true
    fi

    rm -f "$PID_FILE"
    log_info "Operator stopped"
}

# Restart operator
restart_operator() {
    stop_operator
    sleep 1
    start_operator
}

# Show status
show_status() {
    if is_running; then
        local pid
        pid=$(get_pid)
        log_info "Operator is running (PID: $pid)"
        
        # Show memory usage if possible
        if command -v ps &> /dev/null; then
            local mem
            mem=$(ps -p "$pid" -o rss= 2>/dev/null || echo "unknown")
            if [ "$mem" != "unknown" ]; then
                mem=$((mem / 1024))
                echo "  Memory: ${mem}MB"
            fi
        fi
        
        # Show uptime from log
        if [ -f "$LOG_FILE" ]; then
            local start_time
            start_time=$(head -1 "$LOG_FILE" | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}' || echo "")
            if [ -n "$start_time" ]; then
                echo "  Started: $start_time"
            fi
        fi
        
        return 0
    else
        log_warn "Operator is not running"
        return 1
    fi
}

# Show logs
show_logs() {
    if [ -f "$LOG_FILE" ]; then
        local lines="${1:-50}"
        tail -n "$lines" "$LOG_FILE"
    else
        log_warn "No log file found"
    fi
}

# Follow logs
follow_logs() {
    if [ -f "$LOG_FILE" ]; then
        tail -f "$LOG_FILE"
    else
        log_warn "No log file found"
    fi
}

# Main
case "${1:-}" in
    start)
        start_operator
        ;;
    stop)
        stop_operator
        ;;
    restart)
        restart_operator
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs "${2:-50}"
        ;;
    logs-f|follow)
        follow_logs
        ;;
    build)
        build_operator
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|logs|logs-f|build}"
        echo ""
        echo "Commands:"
        echo "  start    Start operator in background"
        echo "  stop     Stop operator"
        echo "  restart  Restart operator"
        echo "  status   Check if operator is running"
        echo "  logs     Show last 50 lines of logs"
        echo "  logs-f   Follow logs (tail -f)"
        echo "  build    Build operator binary"
        exit 1
        ;;
esac
