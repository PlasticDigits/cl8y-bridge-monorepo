#!/usr/bin/env bash
# Run solana-test-validator bound to loopback only (safe on hosts with a public IP).
# RPC/faucet are only reachable via localhost — use SSH port forwarding or a VPN if you need remote access.
#
# Usage:
#   ./scripts/solana/run-test-validator.sh [--reset]
#
# Environment:
#   SOLANA_LEDGER_DIR   Ledger directory (default: ~/.local/share/cl8y-bridge/solana-test-ledger)
#   SOLANA_RPC_PORT     RPC port (default: 8899)
#   SOLANA_FAUCET_PORT  Faucet port (default: 9900)
#   SOLANA_BIND_ADDRESS Bind address — MUST stay loopback on internet-facing servers; default 127.0.0.1.
#                       If unset, we force 127.0.0.1. To bind to all interfaces (not recommended),
#                       set SOLANA_BIND_ADDRESS=0.0.0.0 and CL8Y_ALLOW_SOLANA_PUBLIC_BIND=1.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

SOLANA_LEDGER_DIR="${SOLANA_LEDGER_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/cl8y-bridge/solana-test-ledger}"
SOLANA_RPC_PORT="${SOLANA_RPC_PORT:-8899}"
SOLANA_FAUCET_PORT="${SOLANA_FAUCET_PORT:-9900}"

RESET=""
if [[ "${1:-}" == "--reset" ]]; then
  RESET="--reset"
  shift
fi

BIND="${SOLANA_BIND_ADDRESS:-127.0.0.1}"
if [[ "$BIND" == "0.0.0.0" && "${CL8Y_ALLOW_SOLANA_PUBLIC_BIND:-}" != "1" ]]; then
  echo "error: SOLANA_BIND_ADDRESS=0.0.0.0 would expose RPC/faucet on all interfaces." >&2
  echo "       On a public host use the default (127.0.0.1) or SSH tunnel. To override this guard, set CL8Y_ALLOW_SOLANA_PUBLIC_BIND=1." >&2
  exit 1
fi

if ! command -v solana-test-validator >/dev/null 2>&1; then
  echo "error: solana-test-validator not found in PATH (install Solana CLI or use: make solana-validator)" >&2
  exit 1
fi

if command -v ss >/dev/null 2>&1; then
  if ss -tln "sport = :${SOLANA_RPC_PORT}" 2>/dev/null | grep -q LISTEN; then
    echo "error: something is already listening on TCP port ${SOLANA_RPC_PORT} (stop it or set SOLANA_RPC_PORT)" >&2
    exit 1
  fi
fi

mkdir -p "$SOLANA_LEDGER_DIR"

echo "Starting solana-test-validator (bind=${BIND}, RPC=${SOLANA_RPC_PORT}, faucet=${SOLANA_FAUCET_PORT}, ledger=${SOLANA_LEDGER_DIR})"
echo "  Repo: $REPO_ROOT"
echo "  Stop with Ctrl+C or: kill -TERM <pid>"

exec solana-test-validator \
  --ledger "$SOLANA_LEDGER_DIR" \
  $RESET \
  --bind-address "$BIND" \
  --rpc-port "$SOLANA_RPC_PORT" \
  --faucet-port "$SOLANA_FAUCET_PORT" \
  --log
