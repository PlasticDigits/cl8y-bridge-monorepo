#!/usr/bin/env bash
# GL-121: BSC / opBNB parity — dry-run vs golden JSON, or segmented broadcast entrypoints.
# GL-122: For full preflight + orchestrated sequence (including step 18 gate + optional peers),
#          see scripts/evm/deploy-bsc-parity-orchestrate.sh and docs/deployment-megaeth.md §5.2a.
#
# If stock forge fails after "Script ran successfully" with constructor decode on large CREATE
# initcode (BridgeParityNonce10Outer), build a patched forge: scripts/evm/install-foundry-parity-fix.sh
# then run with FORGE=~/.local/bin/forge-parity (or export FORGE before calling this script).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PKG="$ROOT/packages/contracts-evm"
FORGE="${FORGE:-forge}"
cd "$PKG"

usage() {
  cat <<'EOF'
Usage:
  parity-replay.sh dry-check
  parity-replay.sh broadcast-full   [extra forge args...]   # recommended: one forge session (head + Nick + faucet + tail)
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
    "$FORGE" script script/EvmParityReplay.s.sol:EvmParityReplay --sig runDryCheck -vvv "$@"
    ;;
  broadcast-full)
    "$FORGE" script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastFull --broadcast "$@"
    ;;
  broadcast-head)
    "$FORGE" script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastHead --broadcast "$@"
    ;;
  broadcast-faucet19)
    "$FORGE" script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastFaucet19 --broadcast "$@"
    ;;
  broadcast-tail)
    "$FORGE" script script/EvmParityReplay.s.sol:EvmParityReplay --sig runBroadcastTail --broadcast "$@"
    ;;
  *)
    usage
    exit 1
    ;;
esac
