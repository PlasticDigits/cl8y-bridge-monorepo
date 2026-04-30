#!/usr/bin/env bash
# Sum per-tx `transaction.gas` limits from a forge `runBroadcastFull-latest.json` (parity replay bundle).
# Use after `runBroadcastFull` (or fork rehearsal) to size MIN_FULL_DEPLOY_BALANCE_WEI without guessing.
#
# Usage (repo root):
#   ./scripts/evm/parity-sum-broadcast-gas-limits.sh [path/to/runBroadcastFull-latest.json]
#
# Optional: export RPC_URL=... — multiplies total gas limits by `cast gas-price` for a crude native
# upper bound (legacy-style; refine with maxFee on EIP-1559 if you need tighter numbers).
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
JSON="${1:-$ROOT/packages/contracts-evm/broadcast/EvmParityReplay.s.sol/4326/runBroadcastFull-latest.json}"

if [[ ! -f "$JSON" ]]; then
  echo "No file: $JSON" >&2
  echo "Run a fork rehearsal or real broadcast first, or pass an explicit runBroadcastFull-latest.json path." >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required" >&2
  exit 1
fi

read -r TOTAL GAS_TXS < <(python3 <<PY
import json
path = r"""$JSON"""
with open(path) as f:
    data = json.load(f)
txs = data.get("transactions") or []
total = 0
n = 0
for t in txs:
    g = (t.get("transaction") or {}).get("gas")
    if g is None:
        continue
    total += int(g, 16) if isinstance(g, str) and str(g).startswith("0x") else int(g)
    n += 1
print(total, n)
PY
)

echo "File: $JSON"
echo "Transactions with a gas limit field: $GAS_TXS"
echo "Sum of transaction.gas limits (wei of gas units, not native): $TOTAL"

if [[ -n "${RPC_URL:-}" ]]; then
  if ! command -v cast >/dev/null 2>&1; then
    echo "RPC_URL set but cast not on PATH — skipping native rough bound" >&2
    exit 0
  fi
  GP=$(cast gas-price --rpc-url "$RPC_URL")
  # shellcheck disable=SC2016
  NATIVE_HI=$(python3 -c "g=int('$GP'); t=int('$TOTAL'); print(t * g)")
  echo "cast gas-price (wei per gas unit): $GP"
  echo "Crude upper bound (sum(limits) * gas-price): $NATIVE_HI wei native"
  echo "Suggested MIN_FULL_DEPLOY_BALANCE_WEI (1.2x that bound, integer): $(python3 -c "print(int(int('$NATIVE_HI') * 12 // 10))")"
fi
