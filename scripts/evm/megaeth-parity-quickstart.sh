#!/usr/bin/env bash
# MegaETH mainnet — one-shot GL-122: canonical parity env + deploy-bsc-parity-orchestrate.sh
# (preflight → dry-check → runBroadcastFull). From repo root.
#
# With no arguments: passes --rpc-url (default MegaETH public RPC), -vvv, -i, --sender (historical deployer).
# With arguments: forwards only those to deploy-bsc-parity-orchestrate.sh (you must pass --rpc-url and signing).
#
# Override RPC: export RPC_URL=https://... before running, or pass --rpc-url in args mode.
#
# Preflight: `bsc-parity-preflight.sh` defaults `MIN_FULL_DEPLOY_BALANCE_WEI` to 2e18 wei when unset;
# this script **exports** `MIN_FULL_DEPLOY_BALANCE_WEI=15000000000000000` (0.015 native) for MegaETH unless you override.
# MegaETH (measured 2026-04 on local anvil, chain id 4326, deployer nonce reset to 0):
#   - `runBroadcastFull` emits 45 transactions with summed gas limits of 38,020,544 gas units.
#   - At MegaETH's then-current `cast gas-price` of 1,000,000 wei, the crude upper bound is
#     ~3.9e13 wei; the default floor below is deliberately much higher to absorb gas-price drift.
# Override: export MIN_FULL_DEPLOY_BALANCE_WEI=… before invoking this script.
#
# Patched forge (optional): export FORGE=$HOME/.local/bin/forge-parity after install-foundry-parity-fix.sh
# if stock forge fails on parity broadcast metadata (see docs/deployment-megaeth.md §5.2a).
#
# Factory authority: runBroadcastFull rewrites the historical Nick CREATE2 factory authority to the
# guard-stack AccessManager by default. Set PARITY_PRESERVE_HISTORICAL_FACTORY_AUTHORITY=true only
# when you need byte-identical historical factory initcode.
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
if [[ -z "${FORGE:-}" && -x "$HOME/.local/bin/forge-parity" ]]; then
  export FORGE="$HOME/.local/bin/forge-parity"
  echo "Using patched Forge for parity replay: $FORGE"
else
  export FORGE="${FORGE:-forge}"
  if [[ "$FORGE" == "forge" ]]; then
    forge_version="$("$FORGE" --version 2>/dev/null || true)"
    if [[ "$forge_version" == *"5e88010a83d1b87b8f4d13058e42a2949d3e9dc0"* ]]; then
      cat >&2 <<'EOF'
Stock forge at commit 5e88010 is known to fail this parity broadcast after simulation
with "Failed to decode constructor arguments" on the nonce-10 Bridge parity CREATE.

Install the patched binary first:
  ./scripts/evm/install-foundry-parity-fix.sh

Then rerun this quickstart, or explicitly override FORGE if you have a fixed forge:
  FORGE=/path/to/forge ./scripts/evm/megaeth-parity-quickstart.sh ...
EOF
      exit 1
    fi
  fi
fi

export RPC_URL="${RPC_URL:-https://mainnet.megaeth.com/rpc}"

export DEPLOYER_ADDRESS="${DEPLOYER_ADDRESS:-0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e}"
export ADMIN_ADDRESS="${ADMIN_ADDRESS:-0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c}"
export OPERATOR_ADDRESS="${OPERATOR_ADDRESS:-0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD}"
export FEE_RECIPIENT_ADDRESS="${FEE_RECIPIENT_ADDRESS:-$OPERATOR_ADDRESS}"
export CANCELER_ADDRESS="${CANCELER_ADDRESS:-0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB}"

export PARITY_LEGACY_WETH_ADDRESS="${PARITY_LEGACY_WETH_ADDRESS:-0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c}"
export PARITY_LEGACY_CHAIN_IDENTIFIER="${PARITY_LEGACY_CHAIN_IDENTIFIER:-BSC}"
export PARITY_LEGACY_THIS_CHAIN_ID="${PARITY_LEGACY_THIS_CHAIN_ID:-56}"

export WETH_ADDRESS="${WETH_ADDRESS:-0x4200000000000000000000000000000000000006}"
export CHAIN_IDENTIFIER="${CHAIN_IDENTIFIER:-evm_4326}"
export THIS_CHAIN_ID="${THIS_CHAIN_ID:-4326}"

export GUARD_STACK_ACCESS_MANAGER_ADMIN="${GUARD_STACK_ACCESS_MANAGER_ADMIN:-0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c}"

export MIN_FULL_DEPLOY_BALANCE_WEI="${MIN_FULL_DEPLOY_BALANCE_WEI:-15000000000000000}"

ORCH="$ROOT/scripts/evm/deploy-bsc-parity-orchestrate.sh"

if [[ "$#" -gt 0 ]]; then
  exec "$ORCH" "$@"
else
  exec "$ORCH" --rpc-url "$RPC_URL" -vvv -i --sender "$DEPLOYER_ADDRESS"
fi
