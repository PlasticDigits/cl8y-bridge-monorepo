#!/usr/bin/env bash
# Verify MegaETH mainnet (chain id 4326) contracts on mega.etherscan.io via forge.
#
# From repo root:
#   export ETHERSCAN_API_KEY='your_key'
#   ./scripts/evm/verify-megaeth-etherscan.sh
#
# Why not plain forge --guess-constructor-args?
#   MegaETH public RPC often cannot load historical txs by hash ("Transaction not found").
#   Forge still tries the RPC even when Etherscan supplies a creation tx hash.
#
# This script pulls creation bytecode from Etherscan API v2 (getcontractcreation /
# eth_getTransactionByHash), strips the locally compiled deployment bytecode prefix from
# packages/contracts-evm/out (requires `forge build`), and passes the remainder as
# --constructor-args — no archival RPC needed.
#
# Optional:
#   VERIFY_WATCH=1   — pass --watch (waits for explorer; slower)
#   SKIP_BUILD=1     — skip `forge build` if you already compiled
#
# Requires: python3 (stdlib only), curl not needed (urllib in helper).
#
# Addresses match README.md § MegaETH Mainnet.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CONTRACTS_DIR="$ROOT/packages/contracts-evm"
RESOLVER_PY="$SCRIPT_DIR/verify_megaeth_constructor_args.py"

if [[ -z "${ETHERSCAN_API_KEY:-}" ]]; then
  echo "error: set ETHERSCAN_API_KEY (Etherscan multichain key works for mega.etherscan.io)." >&2
  exit 1
fi

FOUNDRY_DISABLE_NIGHTLY_WARNING="${FOUNDRY_DISABLE_NIGHTLY_WARNING:-1}"
export FOUNDRY_DISABLE_NIGHTLY_WARNING

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required (constructor-args resolver)." >&2
  exit 1
fi

cd "$CONTRACTS_DIR"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  forge build
fi

COMMON_VERIFY=(
  --chain 4326
  --verifier etherscan
  --etherscan-api-key "$ETHERSCAN_API_KEY"
)

if [[ "${VERIFY_WATCH:-0}" == "1" ]]; then
  COMMON_VERIFY+=(--watch)
fi

# One line per contract: <address> <path>:<ContractName>
# Do not split address and spec across lines — bash `read` expects two fields per iteration.
while read -r ADDR SPEC; do
  [[ -z "$ADDR" || "$ADDR" =~ ^# ]] && continue
  if [[ -z "$SPEC" ]]; then
    echo "error: malformed table row (missing SPEC after $ADDR)" >&2
    exit 1
  fi
  echo ">>> Verifying $ADDR ($SPEC)"
  mapfile -t _lines < <(python3 "$RESOLVER_PY" "$ADDR" "$SPEC" "$CONTRACTS_DIR" "$ETHERSCAN_API_KEY")
  _creation_tx="${_lines[0]:-}"
  _ctor_hex="${_lines[1]:-}"
  echo "    creation tx ${_creation_tx}"
  extra=()
  if [[ -n "${_ctor_hex}" ]]; then
    extra+=(--constructor-args "${_ctor_hex}")
  fi
  forge verify-contract "$ADDR" "$SPEC" "${COMMON_VERIFY[@]}" "${extra[@]}"
  sleep 0.25
done <<'EOF'
# ERC-1967 proxies
0x2e5D36C46680A38e7Ae156fc9d109084C58c688e lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy
0x3d8820EC93748fd4df8eee6B763834a23938B207 lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy
0xD7b3Bf05987052009c350874E810Df98dA95D258 lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy
0x0A1a4bd354983DBc7f487237CD1B408CD0003EBC lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy
0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369 lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol:ERC1967Proxy
# UUPS implementations
0x6b1Aa0653D99D5Dec84db4A0283eFB41be826993 src/ChainRegistry.sol:ChainRegistry
0x734d6D554A3f7762D0DbC5538cBa8Ae9e01338f7 src/TokenRegistry.sol:TokenRegistry
0xb43C56D9920Ea8fF1f7eA4B86261f6d59Df04f66 src/LockUnlock.sol:LockUnlock
0x54D67c0Ec4cFe1d9eb945B35D1eBcc25c6abd2c9 src/MintBurn.sol:MintBurn
0x102a87e067aa4c6cc20D06207FB64E4A1A6CDbe6 src/Bridge.sol:Bridge
# Guard / infra
0xa958d75c61227606df21e3261ba80dc399d19676 src/AccessManagerEnumerable.sol:AccessManagerEnumerable
0x375401aaAB20b0827CFC7DBE822e352738D390a9 src/Create3Deployer.sol:Create3Deployer
0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F src/FactoryTokenCl8yBridged.sol:FactoryTokenCl8yBridged
0xeAFE298387F800101f1B9263208A0bfdc934A63e src/DatastoreSetAddress.sol:DatastoreSetAddress
0xD72b2fe3012a2896aef7E3cA561aD11B1542a88c src/TokenRateLimit.sol:TokenRateLimit
0x12FEDD29E71F66157E985AA1aAAE434253E39A22 src/GuardBridge.sol:GuardBridge
# Factory-created bridged ERC20s
0x7deF34032CC5D06bA84A8889bdCA7ee153127B23 src/TokenCl8yBridged.sol:TokenCl8yBridged
0xfBAa45A537cF07dC768c469FfaC4e88208B0098D src/TokenCl8yBridged.sol:TokenCl8yBridged
0xE19442D99Aa2209b08d69c518444C4C1DAfeEDb1 src/TokenCl8yBridged.sol:TokenCl8yBridged
0x840b1515f586c2ea31d55C91B355AFf36eA7af54 src/TokenCl8yBridged.sol:TokenCl8yBridged
EOF

echo "Done."
