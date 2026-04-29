#!/usr/bin/env bash
# GL-121: BSC / opBNB parity — dry-run vs golden JSON, or segmented broadcast entrypoints.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PKG="$ROOT/packages/contracts-evm"
cd "$PKG"

usage() {
  cat <<'EOF'
Usage:
  parity-replay.sh dry-check
  parity-replay.sh broadcast-head   [extra forge args...]
  parity-replay.sh broadcast-faucet19 [extra forge args...]
  parity-replay.sh broadcast-tail   [extra forge args...]

Env (dry-check):
  DEPLOYER_ADDRESS                    required (historical BSC deployer for golden match)
  PARITY_RELAX_DEPLOYER_CHECK=true    optional

Broadcast env: see docs/deployment-megaeth.md §5.3
EOF
}

cmd="${1:-}"
shift || true
case "$cmd" in
  dry-check)
    export DEPLOYER_ADDRESS="${DEPLOYER_ADDRESS:?set DEPLOYER_ADDRESS}"
    forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runDryCheck -vvv "$@"
    ;;
  broadcast-head)
    forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastHead --broadcast "$@"
    ;;
  broadcast-faucet19)
    forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastFaucet19 --broadcast "$@"
    ;;
  broadcast-tail)
    forge script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastTail --broadcast "$@"
    ;;
  *)
    usage
    exit 1
    ;;
esac
