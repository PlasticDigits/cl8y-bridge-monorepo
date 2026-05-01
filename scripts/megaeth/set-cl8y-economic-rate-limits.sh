#!/usr/bin/env bash
# CL8Y economic token — rate limits only (no TokenRegistry mappings, no registerToken).
#
# EVM (MegaETH + BSC):
#   - TokenRegistry.setRateLimit(token, minPerTx, maxPerTx, maxPerPeriod)
#       Defaults: min=1 wei, maxPerTx=1000 CL8Y, maxPerPeriod=1000 CL8Y (18 decimals).
#   - TokenRateLimit.setLimitsBatch — same 1000e18 for 24h deposit + withdraw each.
#
# Terra Classic:
#   - set_rate_limit (max_per_transaction + max_per_period only; no separate “min” in CosmWasm).
#       Defaults: both 1000e18 string (1000 CL8Y).
#
# Signers:
#   - TokenRegistry.setRateLimit: registry owner.
#   - TokenRateLimit: guard AccessManager authority.
#   - Terra: bridge admin key (--from TERRA_WALLET).
#
# Usage (repo root):
#   ./scripts/megaeth/set-cl8y-economic-rate-limits.sh
#   DRY_RUN=1 ./scripts/megaeth/set-cl8y-economic-rate-limits.sh
#
set -euo pipefail

export FOUNDRY_DISABLE_NIGHTLY_WARNING="${FOUNDRY_DISABLE_NIGHTLY_WARNING:-1}"

if [[ ! -c /dev/tty ]] || ! [[ -r /dev/tty && -w /dev/tty ]]; then
  echo "Interactive signing requires a TTY (/dev/tty); run from a real terminal." >&2
  exit 1
fi

MEGAETH_RPC="${MEGAETH_RPC:-https://mainnet.megaeth.com/rpc}"
BSC_RPC="${BSC_RPC:-https://bsc-dataseed1.binance.org}"

BSC_TOKEN_REGISTRY="${BSC_TOKEN_REGISTRY:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"
MEGAETH_TOKEN_REGISTRY="${MEGAETH_TOKEN_REGISTRY:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"

MEGAETH_TOKEN_CL8Y="${MEGAETH_TOKEN_CL8Y:-0xfBAa45A537cF07dC768c469FfaC4e88208B0098D}"
BSC_TOKEN_CL8Y="${BSC_TOKEN_CL8Y:-0x8f452a1fdd388a45e1080992eff051b4dd9048d2}"
TERRA_TOKEN_CL8Y="${TERRA_TOKEN_CL8Y:-terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3}"

MEGAETH_TOKEN_RATE_LIMIT="${MEGAETH_TOKEN_RATE_LIMIT:-0xD72b2fe3012a2896aef7E3cA561aD11B1542a88c}"
BSC_TOKEN_RATE_LIMIT="${BSC_TOKEN_RATE_LIMIT:-0xD72b2fe3012a2896aef7E3cA561aD11B1542a88c}"

# 18-decimal CL8Y: min 1 wei, max 1000 tokens per tx and per 24h on TokenRegistry withdraw logic.
CL8Y_MIN_PER_TX_WEI="${CL8Y_MIN_PER_TX_WEI:-1}"
CL8Y_MAX_PER_TX_WEI="${CL8Y_MAX_PER_TX_WEI:-1000000000000000000000}"
CL8Y_MAX_PER_PERIOD_WEI="${CL8Y_MAX_PER_PERIOD_WEI:-1000000000000000000000}"
CL8Y_GUARD_LIMIT_WEI="${CL8Y_GUARD_LIMIT_WEI:-$CL8Y_MAX_PER_PERIOD_WEI}"

TERRA_CL8Y_MAX_PER_TX_STR="${TERRA_CL8Y_MAX_PER_TX_STR:-1000000000000000000000}"
TERRA_CL8Y_MAX_PER_PERIOD_STR="${TERRA_CL8Y_MAX_PER_PERIOD_STR:-1000000000000000000000}"

INCLUDE_MEGAETH="${INCLUDE_MEGAETH:-1}"
INCLUDE_BSC="${INCLUDE_BSC:-1}"
INCLUDE_TERRA="${INCLUDE_TERRA:-1}"
DRY_RUN="${DRY_RUN:-0}"
VERIFY_ONCHAIN="${VERIFY_ONCHAIN:-1}"

TERRA_NODE_URL="${TERRA_NODE_URL:-https://terra-classic-rpc.publicnode.com:443}"
TERRA_BRIDGE_CONTRACT="${TERRA_BRIDGE_CONTRACT:-terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la}"
TERRA_WALLET="${TERRA_WALLET:-cl8y2_admin}"
TERRA_KEYRING_BACKEND="${TERRA_KEYRING_BACKEND:-file}"
TERRA_FEES="${TERRA_FEES:-10000000uluna}"
TERRA_GAS_ADJUSTMENT="${TERRA_GAS_ADJUSTMENT:-1.5}"
TERRA_TX_SLEEP_SECONDS="${TERRA_TX_SLEEP_SECONDS:-6}"

send() {
  local rpc="$1"
  shift
  if [[ "$DRY_RUN" == "1" ]]; then
    printf 'DRY_RUN cast send --interactive --rpc-url %q' "$rpc"
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi
  cast send --interactive --rpc-url "$rpc" "$@" </dev/tty
}

token_registered() {
  local rpc="$1" registry="$2" token="$3"
  [[ "$(cast call --rpc-url "$rpc" "$registry" "tokenRegistered(address)(bool)" "$token" | sed -n '1p')" == "true" ]]
}

terra_tx() {
  local label="$1" msg="$2"
  echo ">>> Terra: $label"
  if [[ "$DRY_RUN" == "1" ]]; then
    printf 'DRY_RUN terrad tx wasm execute %q %q ...\n' "$TERRA_BRIDGE_CONTRACT" "$msg"
    return 0
  fi
  terrad tx wasm execute "$TERRA_BRIDGE_CONTRACT" "$msg" \
    --from "$TERRA_WALLET" \
    --chain-id columbus-5 \
    --node "$TERRA_NODE_URL" \
    --gas auto --gas-adjustment "$TERRA_GAS_ADJUSTMENT" \
    --fees "$TERRA_FEES" \
    --keyring-backend "$TERRA_KEYRING_BACKEND" -y
  sleep "$TERRA_TX_SLEEP_SECONDS"
}

evm_registry_limits() {
  local label="$1" rpc="$2" registry="$3" token="$4"
  if ! token_registered "$rpc" "$registry" "$token"; then
    echo "ERROR: $label token $token is not registered on TokenRegistry $registry — setRateLimit will revert." >&2
    exit 1
  fi
  echo ">>> $label: TokenRegistry.setRateLimit min=$CL8Y_MIN_PER_TX_WEI maxTx=$CL8Y_MAX_PER_TX_WEI maxPeriod=$CL8Y_MAX_PER_PERIOD_WEI"
  send "$rpc" "$registry" "setRateLimit(address,uint256,uint256,uint256)" \
    "$token" "$CL8Y_MIN_PER_TX_WEI" "$CL8Y_MAX_PER_TX_WEI" "$CL8Y_MAX_PER_PERIOD_WEI"
}

evm_guard_limits() {
  local label="$1" rpc="$2" trl="$3" token="$4"
  echo ">>> $label: TokenRateLimit.setLimitsBatch deposit=$CL8Y_GUARD_LIMIT_WEI withdraw=$CL8Y_GUARD_LIMIT_WEI"
  send "$rpc" "$trl" "setLimitsBatch(address[],uint256[],uint256[])" \
    "[$token]" "[$CL8Y_GUARD_LIMIT_WEI]" "[$CL8Y_GUARD_LIMIT_WEI]"
}

echo "=== CL8Y economic rate limits only ==="
echo "DRY_RUN=$DRY_RUN INCLUDE_MEGAETH=$INCLUDE_MEGAETH INCLUDE_BSC=$INCLUDE_BSC INCLUDE_TERRA=$INCLUDE_TERRA"
echo "EVM: minPerTx=$CL8Y_MIN_PER_TX_WEI maxPerTx=$CL8Y_MAX_PER_TX_WEI maxPerPeriod=$CL8Y_MAX_PER_PERIOD_WEI"

if [[ "$INCLUDE_MEGAETH" == "1" ]]; then
  echo "=== MegaETH ==="
  evm_registry_limits "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_CL8Y"
  evm_guard_limits "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_RATE_LIMIT" "$MEGAETH_TOKEN_CL8Y"
fi

if [[ "$INCLUDE_BSC" == "1" ]]; then
  echo "=== BSC ==="
  evm_registry_limits "BSC" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$BSC_TOKEN_CL8Y"
  evm_guard_limits "BSC" "$BSC_RPC" "$BSC_TOKEN_RATE_LIMIT" "$BSC_TOKEN_CL8Y"
fi

if [[ "$INCLUDE_TERRA" == "1" ]]; then
  echo "=== Terra Classic (no min field — max per tx + max per period) ==="
  terra_tx "CL8Y set_rate_limit" "$(printf '{"set_rate_limit":{"token":"%s","max_per_transaction":"%s","max_per_period":"%s"}}' "$TERRA_TOKEN_CL8Y" "$TERRA_CL8Y_MAX_PER_TX_STR" "$TERRA_CL8Y_MAX_PER_PERIOD_STR")"
fi

if [[ "$VERIFY_ONCHAIN" == "1" ]]; then
  echo ""
  echo "=== Read-back (verify) ==="
  if [[ "$INCLUDE_MEGAETH" == "1" ]]; then
    echo "MegaETH getRateLimitConfig(CL8Y-cb):"
    cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "getRateLimitConfig(address)(uint256,uint256,uint256)" "$MEGAETH_TOKEN_CL8Y" || true
    echo "MegaETH TokenRateLimit deposit/withdraw:"
    cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_RATE_LIMIT" "depositLimitPerToken(address)(uint256)" "$MEGAETH_TOKEN_CL8Y" || true
    cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_RATE_LIMIT" "withdrawLimitPerToken(address)(uint256)" "$MEGAETH_TOKEN_CL8Y" || true
  fi
  if [[ "$INCLUDE_BSC" == "1" ]]; then
    echo "BSC getRateLimitConfig(CL8Y):"
    cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "getRateLimitConfig(address)(uint256,uint256,uint256)" "$BSC_TOKEN_CL8Y" || true
    echo "BSC TokenRateLimit deposit/withdraw:"
    cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_RATE_LIMIT" "depositLimitPerToken(address)(uint256)" "$BSC_TOKEN_CL8Y" || true
    cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_RATE_LIMIT" "withdrawLimitPerToken(address)(uint256)" "$BSC_TOKEN_CL8Y" || true
  fi
fi

echo "Done."
