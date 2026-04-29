#!/usr/bin/env bash
# MegaETH mainnet — one-shot GL-122: canonical parity env + deploy-bsc-parity-orchestrate.sh
# (preflight → dry-check → runBroadcastFull). From repo root.
#
# With no arguments: passes --rpc-url (default MegaETH public RPC), -vvv, -i 1, --sender (historical deployer).
# With arguments: forwards only those to deploy-bsc-parity-orchestrate.sh (you must pass --rpc-url and signing).
#
# Override RPC: export RPC_URL=https://... before running, or pass --rpc-url in args mode.
#
# Preflight: `bsc-parity-preflight.sh` defaults `MIN_FULL_DEPLOY_BALANCE_WEI` to 2e18 wei when unset;
# this script **exports** `MIN_FULL_DEPLOY_BALANCE_WEI=15000000000000000` (0.015 native) for MegaETH unless you override.
# MegaETH (measured 2026-04 on anvil --fork-url mainnet.megaeth.com/rpc, deployer nonce reset to 0):
#   - `forge script … runBroadcastHead --broadcast` reported ~3.5e-5 ETH simulated head spend.
#   - `RPC_URL=… ./scripts/evm/parity-sum-broadcast-gas-limits.sh …/runBroadcastHead-latest.json` gave
#     sum(gas limits)×gas-price ≈ 1.75e13 wei for the 19-tx head bundle (limits are loose upper bounds).
# Full `runBroadcastFull` adds Nick + faucet + a much heavier tail — default floor below is ~850× that head-only
# crude bound so small wallets still pass preflight; tighten with MIN_FULL after a full `run-latest.json` sum.
# Override: export MIN_FULL_DEPLOY_BALANCE_WEI=… before invoking this script.
#
# Patched forge (optional): export FORGE=$HOME/.local/bin/forge-parity after install-foundry-parity-fix.sh
# if stock forge fails on parity broadcast metadata (see docs/deployment-megaeth.md §5.2a).
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export FORGE="${FORGE:-forge}"

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
  exec "$ORCH" --rpc-url "$RPC_URL" -vvv -i 1 --sender "$DEPLOYER_ADDRESS"
fi
