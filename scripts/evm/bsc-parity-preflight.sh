#!/usr/bin/env bash
# Preflight native balances for mainnet-parity EVM deploy (GL-122): deployer, operator, canceler.
#
# Usage:
#   export RPC_URL=https://...
#   ./scripts/evm/bsc-parity-preflight.sh
#
# Optional:
#   MIN_FULL_DEPLOY_BALANCE_WEI — minimum deployer balance (wei). Default: 2e18 (2 native units, 18 decimals).
#                                 Override lower for cheap/funded testnets after you have a measured estimate.
#                                 See docs/deployment-megaeth.md §5.0 and scripts/evm/parity-sum-broadcast-gas-limits.sh.
#
# Exit 1 if any check fails.

set -euo pipefail

# Canonical mainnet parity roles (must match operator runbook / GL-122)
DEPLOYER="${DEPLOYER_ADDRESS:-0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e}"
ADMIN="${ADMIN_ADDRESS:-0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c}"
OPERATOR="${OPERATOR_ADDRESS:-0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD}"
CANCELER="${CANCELER_ADDRESS:-0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB}"

RPC_URL="${RPC_URL:?set RPC_URL}"

MIN_FULL="${MIN_FULL_DEPLOY_BALANCE_WEI:-2000000000000000000}"

if ! command -v cast >/dev/null 2>&1; then
  echo "cast (Foundry) is required on PATH" >&2
  exit 1
fi

fail() {
  echo "PREFLIGHT FAIL: $*" >&2
  exit 1
}

echo "=== BSC parity preflight (native balance on RPC_URL) ==="
echo "RPC_URL=$RPC_URL"
echo ""

d_wei=$(cast balance "$DEPLOYER" --rpc-url "$RPC_URL" 2>/dev/null || echo "0")
o_wei=$(cast balance "$OPERATOR" --rpc-url "$RPC_URL" 2>/dev/null || echo "0")
c_wei=$(cast balance "$CANCELER" --rpc-url "$RPC_URL" 2>/dev/null || echo "0")

echo "Deployer  ($DEPLOYER): $d_wei wei"
echo "Operator  ($OPERATOR): $o_wei wei"
echo "Canceler  ($CANCELER): $c_wei wei"
echo "Admin     ($ADMIN) — informational (ADMIN_ADDRESS for ownership transfer after deploy)"
echo ""

python3 <<PY
import sys

def chk(name, bal_s, need_positive=True, min_full=None):
    bal_s = bal_s.strip().split()[0] if bal_s else "0"
    try:
        bal = int(bal_s)
    except ValueError:
        print(f"PREFLIGHT FAIL: could not parse {name} balance: {bal_s!r}", file=sys.stderr)
        sys.exit(1)
    if need_positive and bal <= 0:
        print(f"PREFLIGHT FAIL: {name} balance must be > 0 wei", file=sys.stderr)
        sys.exit(1)
    if min_full is not None and bal < min_full:
        print(f"PREFLIGHT FAIL: deployer balance {bal} < MIN_FULL_DEPLOY_BALANCE_WEI ({min_full})", file=sys.stderr)
        sys.exit(1)

min_full = int("${MIN_FULL}")
chk("deployer", """${d_wei}""", need_positive=True, min_full=min_full)
chk("operator", """${o_wei}""", need_positive=True)
chk("canceler", """${c_wei}""", need_positive=True)
PY

echo "PREFLIGHT OK — deployer/operator/canceler non-zero; deployer meets MIN_FULL_DEPLOY_BALANCE_WEI (${MIN_FULL})."
exit 0
