#!/usr/bin/env bash
# Rehearse BSC parity `runBroadcastHead` on a local Anvil fork via `forge test` (avoids forge script’s
# post-run ERC1967Proxy constructor decode bug on large CREATE initcode; see foundry-rs/foundry#7144).
#
# Prerequisites: Anvil listening on PARITY_HEAD_REHEARSAL_RPC (default http://127.0.0.1:18545).
# The test deploys `MockWETH` and sets `PARITY_LEGACY_WETH_ADDRESS` itself; no impersonation needed.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT/packages/contracts-evm"
export PARITY_HEAD_REHEARSAL_RPC="${PARITY_HEAD_REHEARSAL_RPC:-http://127.0.0.1:18545}"
exec forge test --match-contract ParityHeadAnvilRehearsalTest -vv "$@"
