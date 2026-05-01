#!/usr/bin/env bash
# Economic CL8Y only: MegaETH factory mint (CL8Y-cb) <> BSC CL8Y <> Terra Classic CL8Y CW20.
#
# Does **not** touch test tokens (testa/testb/tdec). For those, use:
#   scripts/megaeth/register-megaeth-token-mappings.sh
#
# Skips opBNB and Solana (no CL8Y in the production matrix there).
#
# Rate limits (when INCLUDE_RATE_LIMITS=1, default):
#   - TokenRegistry (MegaETH + BSC): setRateLimit — default min=0, maxPerTx=0, maxPerPeriod=1000e18 (withdraw / 24h).
#   - TokenRateLimit guard (MegaETH + BSC): setLimitsBatch — same 1000e18 cap for deposit + withdraw / 24h each
#       (separate module from TokenRegistry; see deployment-solana-mainnet.md). Requires guard-stack admin on AccessManager.
#   - Terra bridge: set_rate_limit — max_per_tx=0, max_per_period=1000e18.
#   Does **not** set Bridge.guardBridge, TokenRegistry.rateLimitBridge, or register GuardBridge modules — chain-wide wiring is
#   `scripts/evm/megaeth-manager-followup.sh`. With VERIFY_CL8Y_ONCHAIN=1 (default), prints read-only cast summaries at the end.
#
# Prerequisites:
#   - When INCLUDE_MEGAETH_CL8Y_REGISTRY=1 (default), this script registers `MEGAETH_TOKEN_CL8Y` on
#     MegaETH TokenRegistry as MintBurn and authorizes MintBurn via the guard AccessManager (same
#     pattern as `megaeth-manager-followup.sh`). Set INCLUDE_MEGAETH_CL8Y_REGISTRY=0 to skip and
#     register manually.
#   - BSC CL8Y and Terra CL8Y CW20 must already exist; BSC CL8Y must be tokenRegistered.
#
# Signing:
#   - EVM TokenRegistry / MintBurn wiring: owner / manager (see megaeth-manager-followup defaults).
#   - EVM TokenRateLimit.setLimitsBatch: requires AccessManager authority on the guard module (may differ from TokenRegistry owner).
#   - Terra: `terrad tx wasm execute` (bridge admin key).
#
set -euo pipefail

export FOUNDRY_DISABLE_NIGHTLY_WARNING="${FOUNDRY_DISABLE_NIGHTLY_WARNING:-1}"

if [[ ! -c /dev/tty ]] || ! [[ -r /dev/tty && -w /dev/tty ]]; then
  echo "Interactive signing requires a TTY (/dev/tty); run from a real terminal." >&2
  exit 1
fi

MEGAETH_RPC="${MEGAETH_RPC:-https://mainnet.megaeth.com/rpc}"
BSC_RPC="${BSC_RPC:-https://bsc-dataseed1.binance.org}"

BSC_CHAIN_ID="${BSC_CHAIN_ID:-0x00000038}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-0x00000001}"
MEGAETH_CHAIN_ID="${MEGAETH_CHAIN_ID:-0x000010e6}"

BSC_TOKEN_REGISTRY="${BSC_TOKEN_REGISTRY:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"
MEGAETH_TOKEN_REGISTRY="${MEGAETH_TOKEN_REGISTRY:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"

MEGAETH_TOKEN_CL8Y="${MEGAETH_TOKEN_CL8Y:-0xfBAa45A537cF07dC768c469FfaC4e88208B0098D}"
BSC_TOKEN_CL8Y="${BSC_TOKEN_CL8Y:-0x8f452a1fdd388a45e1080992eff051b4dd9048d2}"
TERRA_TOKEN_CL8Y="${TERRA_TOKEN_CL8Y:-terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3}"

INCLUDE_TERRA="${INCLUDE_TERRA:-1}"
INCLUDE_EVM="${INCLUDE_EVM:-1}"
INCLUDE_RATE_LIMITS="${INCLUDE_RATE_LIMITS:-1}"
INCLUDE_MEGAETH_CL8Y_REGISTRY="${INCLUDE_MEGAETH_CL8Y_REGISTRY:-1}"
VERIFY_CL8Y_ONCHAIN="${VERIFY_CL8Y_ONCHAIN:-1}"
DRY_RUN="${DRY_RUN:-0}"

# TokenRegistry.withdraw caps (18 decimals): unlimited per-tx, 1000 CL8Y per 24h window.
CL8Y_MIN_PER_TX_WEI="${CL8Y_MIN_PER_TX_WEI:-0}"
CL8Y_MAX_PER_TX_WEI="${CL8Y_MAX_PER_TX_WEI:-0}"
CL8Y_MAX_PER_PERIOD_WEI="${CL8Y_MAX_PER_PERIOD_WEI:-1000000000000000000000}"
# TokenRateLimit guard: 24h deposit + withdraw caps (base units). Defaults to same as TokenRegistry maxPerPeriod.
CL8Y_GUARD_LIMIT_WEI="${CL8Y_GUARD_LIMIT_WEI:-$CL8Y_MAX_PER_PERIOD_WEI}"

# Terra SetRateLimit (string uint128 in JSON)
TERRA_CL8Y_MAX_PER_TX_STR="${TERRA_CL8Y_MAX_PER_TX_STR:-0}"
TERRA_CL8Y_MAX_PER_PERIOD_STR="${TERRA_CL8Y_MAX_PER_PERIOD_STR:-1000000000000000000000}"

TERRA_NODE_URL="${TERRA_NODE_URL:-https://terra-classic-rpc.publicnode.com:443}"
TERRA_BRIDGE_CONTRACT="${TERRA_BRIDGE_CONTRACT:-terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la}"
TERRA_WALLET="${TERRA_WALLET:-cl8y2_admin}"
TERRA_KEYRING_BACKEND="${TERRA_KEYRING_BACKEND:-file}"
TERRA_FEES="${TERRA_FEES:-10000000uluna}"
TERRA_GAS_ADJUSTMENT="${TERRA_GAS_ADJUSTMENT:-1.5}"
TERRA_TX_SLEEP_SECONDS="${TERRA_TX_SLEEP_SECONDS:-6}"

# MegaETH MintBurn wiring (defaults: megaeth-manager-followup.sh production MegaETH)
TOKEN_TYPE_MINT_BURN="${TOKEN_TYPE_MINT_BURN:-1}"
ROLE_MINT_BURN="${ROLE_MINT_BURN:-1}"
MINT_SELECTOR="${MINT_SELECTOR:-0x40c10f19}"
BURN_SELECTOR="${BURN_SELECTOR:-0x9dc29fac}"
MEGAETH_MINT_BURN_ADDRESS="${MEGAETH_MINT_BURN_ADDRESS:-0x0A1a4bd354983DBc7f487237CD1B408CD0003EBC}"
MEGAETH_GUARD_ACCESS_MANAGER="${MEGAETH_GUARD_ACCESS_MANAGER:-0xa958d75c61227606df21e3261ba80dc399d19676}"

# Parity guard module (BSC / MegaETH — same proxy address)
MEGAETH_TOKEN_RATE_LIMIT="${MEGAETH_TOKEN_RATE_LIMIT:-0xD72b2fe3012a2896aef7E3cA561aD11B1542a88c}"
BSC_TOKEN_RATE_LIMIT="${BSC_TOKEN_RATE_LIMIT:-0xD72b2fe3012a2896aef7E3cA561aD11B1542a88c}"

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

address_b32() {
  cast abi-encode "f(address)" "$1"
}

hex_b64() {
  python3 - "$1" <<'PY'
import base64
import sys

hex_value = sys.argv[1].strip()
if hex_value.startswith("0x"):
    hex_value = hex_value[2:]
print(base64.b64encode(bytes.fromhex(hex_value)).decode())
PY
}

terra_b32() {
  python3 - "$1" <<'PY'
import sys

CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"

def bech32_polymod(values):
    chk = 1
    for value in values:
        top = chk >> 25
        chk = (chk & 0x1ffffff) << 5 ^ value
        for i, generator in enumerate([0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]):
            if (top >> i) & 1:
                chk ^= generator
    return chk

def bech32_hrp_expand(hrp):
    return [ord(x) >> 5 for x in hrp] + [0] + [ord(x) & 31 for x in hrp]

def convertbits(data, frombits, tobits, pad=True):
    acc = 0
    bits = 0
    ret = []
    maxv = (1 << tobits) - 1
    for value in data:
        if value < 0 or value >> frombits:
            raise ValueError("invalid bech32 data")
        acc = (acc << frombits) | value
        bits += frombits
        while bits >= tobits:
            bits -= tobits
            ret.append((acc >> bits) & maxv)
    if pad:
        if bits:
            ret.append((acc << (tobits - bits)) & maxv)
    elif bits >= frombits or ((acc << (tobits - bits)) & maxv):
        raise ValueError("invalid padding")
    return bytes(ret)

addr = sys.argv[1].strip()
if addr.lower() != addr and addr.upper() != addr:
    raise SystemExit("mixed-case bech32 address")
addr = addr.lower()
pos = addr.rfind("1")
if pos < 1:
    raise SystemExit("invalid bech32 address")
hrp = addr[:pos]
data = [CHARSET.find(c) for c in addr[pos + 1:]]
if any(x == -1 for x in data):
    raise SystemExit("invalid bech32 character")
if bech32_polymod(bech32_hrp_expand(hrp) + data) != 1:
    raise SystemExit("invalid bech32 checksum")
raw = convertbits(data[:-6], 5, 8, False)
print("0x" + raw.rjust(32, b"\0").hex())
PY
}

terra_b32_b64() {
  hex_b64 "$(terra_b32 "$1")"
}

set_dest() {
  local label="$1" rpc="$2" registry="$3" local_token="$4" dest_chain="$5" dest_token_b32="$6" dest_decimals="$7"
  echo ">>> $label: setTokenDestinationWithDecimals"
  send "$rpc" "$registry" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
    "$local_token" "$dest_chain" "$dest_token_b32" "$dest_decimals"
}

set_incoming() {
  local label="$1" rpc="$2" registry="$3" src_chain="$4" local_token="$5" src_decimals="$6"
  echo ">>> $label: setIncomingTokenMapping"
  send "$rpc" "$registry" "setIncomingTokenMapping(bytes4,address,uint8)" \
    "$src_chain" "$local_token" "$src_decimals"
}

chain_registry_for() {
  local rpc="$1" registry="$2"
  cast call --rpc-url "$rpc" "$registry" "chainRegistry()(address)"
}

chain_registered() {
  local rpc="$1" chain_registry="$2" chain_id="$3"
  [[ "$(cast call --rpc-url "$rpc" "$chain_registry" "isChainRegistered(bytes4)(bool)" "$chain_id")" == "true" ]]
}

ensure_chain_registered() {
  local label="$1" rpc="$2" token_registry="$3" ident="$4" chain_id="$5"
  local chain_registry
  chain_registry="$(chain_registry_for "$rpc" "$token_registry")"

  if chain_registered "$rpc" "$chain_registry" "$chain_id"; then
    echo "Peer already registered on $label: $ident ($chain_id)"
    return 0
  fi

  echo "Registering peer on $label: $ident ($chain_id)"
  send "$rpc" "$chain_registry" "registerChain(string,bytes4)" "$ident" "$chain_id"
}

token_registered_on_registry() {
  local rpc="$1" registry="$2" token="$3"
  [[ "$(cast call --rpc-url "$rpc" "$registry" "tokenRegistered(address)(bool)" "$token")" == "true" ]]
}

set_registry_rate_limit_cl8y() {
  local label="$1" rpc="$2" registry="$3" token="$4"
  if ! token_registered_on_registry "$rpc" "$registry" "$token"; then
    echo "ERROR: $label — token $token is not registered on TokenRegistry at $registry." >&2
    echo "  Register it (e.g. registerToken + MintBurn wiring) before setRateLimit." >&2
    exit 1
  fi
  echo ">>> $label: setRateLimit — minPerTx=$CL8Y_MIN_PER_TX_WEI maxPerTx=$CL8Y_MAX_PER_TX_WEI maxPerPeriod=$CL8Y_MAX_PER_PERIOD_WEI (withdraw / 24h)"
  send "$rpc" "$registry" "setRateLimit(address,uint256,uint256,uint256)" \
    "$token" "$CL8Y_MIN_PER_TX_WEI" "$CL8Y_MAX_PER_TX_WEI" "$CL8Y_MAX_PER_PERIOD_WEI"
}

set_guard_token_rate_limit_cl8y() {
  local label="$1" rpc="$2" trl="$3" token="$4"
  echo ">>> $label: TokenRateLimit.setLimitsBatch — 24h deposit & withdraw cap = $CL8Y_GUARD_LIMIT_WEI wei each"
  send "$rpc" "$trl" "setLimitsBatch(address[],uint256[],uint256[])" \
    "[$token]" "[$CL8Y_GUARD_LIMIT_WEI]" "[$CL8Y_GUARD_LIMIT_WEI]"
}

terra_tx() {
  local label="$1" msg="$2"
  echo ">>> Terra: $label"
  if [[ "$DRY_RUN" == "1" ]]; then
    printf 'DRY_RUN terrad tx wasm execute %q %q --from %q --chain-id columbus-5 --node %q --gas auto --gas-adjustment %q --fees %q --keyring-backend %q -y\n' \
      "$TERRA_BRIDGE_CONTRACT" "$msg" "$TERRA_WALLET" "$TERRA_NODE_URL" "$TERRA_GAS_ADJUSTMENT" "$TERRA_FEES" "$TERRA_KEYRING_BACKEND"
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

megaeth_cast_call() {
  cast call --rpc-url "$MEGAETH_RPC" "$@"
}

megaeth_bool_call() {
  local out
  out="$(megaeth_cast_call "$@")"
  [[ "$(printf '%s\n' "$out" | sed -n '1p')" == "true" ]]
}

register_megaeth_cl8y_on_tokenregistry_if_needed() {
  if token_registered_on_registry "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_CL8Y"; then
    echo "MegaETH CL8Y: already registered on TokenRegistry"
    return 0
  fi
  echo ">>> MegaETH CL8Y: registerToken(address,uint8) — MintBurn type $TOKEN_TYPE_MINT_BURN"
  send "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "registerToken(address,uint8)" \
    "$MEGAETH_TOKEN_CL8Y" "$TOKEN_TYPE_MINT_BURN"
}

authorize_mintburn_for_megaeth_cl8y_if_needed() {
  if megaeth_bool_call "$MEGAETH_GUARD_ACCESS_MANAGER" \
    "canCall(address,address,bytes4)(bool,uint32)" \
    "$MEGAETH_MINT_BURN_ADDRESS" "$MEGAETH_TOKEN_CL8Y" "$MINT_SELECTOR"; then
    echo "MegaETH CL8Y: MintBurn already authorized (canCall mint)"
    return 0
  fi
  echo ">>> MegaETH CL8Y: grantRole — MintBurn role $ROLE_MINT_BURN for $MEGAETH_MINT_BURN_ADDRESS"
  send "$MEGAETH_RPC" "$MEGAETH_GUARD_ACCESS_MANAGER" "grantRole(uint64,address,uint32)" \
    "$ROLE_MINT_BURN" "$MEGAETH_MINT_BURN_ADDRESS" 0

  echo ">>> MegaETH CL8Y: setTargetFunctionRole — mint/burn selectors on token for role $ROLE_MINT_BURN"
  send "$MEGAETH_RPC" "$MEGAETH_GUARD_ACCESS_MANAGER" "setTargetFunctionRole(address,bytes4[],uint64)" \
    "$MEGAETH_TOKEN_CL8Y" "[$MINT_SELECTOR,$BURN_SELECTOR]" "$ROLE_MINT_BURN"
}

ensure_megaeth_cl8y_registry_and_mintburn() {
  [[ "$INCLUDE_MEGAETH_CL8Y_REGISTRY" == "1" ]] || return 0
  [[ "$INCLUDE_EVM" == "1" ]] || return 0

  echo "=== MegaETH CL8Y: TokenRegistry + MintBurn (if needed) ==="
  register_megaeth_cl8y_on_tokenregistry_if_needed
  authorize_mintburn_for_megaeth_cl8y_if_needed
}

preflight_bsc_cl8y_registered() {
  [[ "$INCLUDE_EVM" == "1" ]] || return 0
  if token_registered_on_registry "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$BSC_TOKEN_CL8Y"; then
    echo "BSC TokenRegistry: CL8Y is registered — OK"
    return 0
  fi
  cat >&2 <<EOF
ERROR: BSC CL8Y $BSC_TOKEN_CL8Y is not registered on BSC TokenRegistry $BSC_TOKEN_REGISTRY.
Fix production BSC registration before cross-chain CL8Y mappings from MegaETH.
EOF
  exit 1
}

ensure_megaeth_cl8y_registry_and_mintburn
preflight_bsc_cl8y_registered

echo "=== MegaETH economic CL8Y mappings ==="
echo "DRY_RUN=$DRY_RUN INCLUDE_EVM=$INCLUDE_EVM INCLUDE_TERRA=$INCLUDE_TERRA INCLUDE_RATE_LIMITS=$INCLUDE_RATE_LIMITS INCLUDE_MEGAETH_CL8Y_REGISTRY=$INCLUDE_MEGAETH_CL8Y_REGISTRY VERIFY_CL8Y_ONCHAIN=$VERIFY_CL8Y_ONCHAIN"

MEGAETH_CL8Y_B32="$(address_b32 "$MEGAETH_TOKEN_CL8Y")"
BSC_CL8Y_B32="$(address_b32 "$BSC_TOKEN_CL8Y")"

if [[ "$INCLUDE_EVM" == "1" ]]; then
  echo "=== EVM TokenRegistry (CL8Y only) ==="
  ensure_chain_registered "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "evm_56" "$BSC_CHAIN_ID"
  ensure_chain_registered "BSC" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "evm_4326" "$MEGAETH_CHAIN_ID"

  if [[ "$INCLUDE_TERRA" == "1" ]]; then
    ensure_chain_registered "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "terraclassic_columbus-5" "$TERRA_CHAIN_ID"
    TERRA_CL8Y_B32="$(terra_b32 "$TERRA_TOKEN_CL8Y")"
    set_dest "MegaETH CL8Y-cb -> Terra CL8Y" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_CL8Y" "$TERRA_CHAIN_ID" "$TERRA_CL8Y_B32" 18
    set_incoming "MegaETH CL8Y-cb <- Terra" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$TERRA_CHAIN_ID" "$MEGAETH_TOKEN_CL8Y" 18
  fi

  set_dest "MegaETH CL8Y-cb -> BSC CL8Y" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_CL8Y" "$BSC_CHAIN_ID" "$BSC_CL8Y_B32" 18
  set_incoming "MegaETH CL8Y-cb <- BSC" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$BSC_CHAIN_ID" "$MEGAETH_TOKEN_CL8Y" 18
  set_dest "BSC CL8Y -> MegaETH CL8Y-cb" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$BSC_TOKEN_CL8Y" "$MEGAETH_CHAIN_ID" "$MEGAETH_CL8Y_B32" 18
  set_incoming "BSC CL8Y <- MegaETH" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$MEGAETH_CHAIN_ID" "$BSC_TOKEN_CL8Y" 18

  if [[ "$INCLUDE_RATE_LIMITS" == "1" ]]; then
    echo "=== EVM TokenRegistry rate limits (CL8Y) ==="
    set_registry_rate_limit_cl8y "MegaETH CL8Y-cb" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_CL8Y"
    set_registry_rate_limit_cl8y "BSC CL8Y" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$BSC_TOKEN_CL8Y"
    echo "=== TokenRateLimit guard — CL8Y (deposit + withdraw / 24h) ==="
    set_guard_token_rate_limit_cl8y "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_RATE_LIMIT" "$MEGAETH_TOKEN_CL8Y"
    set_guard_token_rate_limit_cl8y "BSC" "$BSC_RPC" "$BSC_TOKEN_RATE_LIMIT" "$BSC_TOKEN_CL8Y"
  fi
fi

if [[ "$INCLUDE_TERRA" == "1" ]]; then
  echo "=== Terra Classic bridge (CL8Y only) ==="
  MEGAETH_CHAIN_B64="$(hex_b64 "$MEGAETH_CHAIN_ID")"
  TERRA_CL8Y_SRC_B64="$(terra_b32_b64 "$TERRA_TOKEN_CL8Y")"
  terra_tx "Terra CL8Y -> MegaETH CL8Y-cb" "$(printf '{"set_token_destination":{"token":"%s","dest_chain":"%s","dest_token":"%s","dest_decimals":18}}' "$TERRA_TOKEN_CL8Y" "$MEGAETH_CHAIN_B64" "$MEGAETH_CL8Y_B32")"
  terra_tx "Terra CL8Y <- MegaETH CL8Y-cb" "$(printf '{"set_incoming_token_mapping":{"src_chain":"%s","src_token":"%s","local_token":"%s","src_decimals":18}}' "$MEGAETH_CHAIN_B64" "$TERRA_CL8Y_SRC_B64" "$TERRA_TOKEN_CL8Y")"

  if [[ "$INCLUDE_RATE_LIMITS" == "1" ]]; then
    terra_tx "Terra CL8Y set_rate_limit (1000 CL8Y / 24h)" "$(printf '{"set_rate_limit":{"token":"%s","max_per_transaction":"%s","max_per_period":"%s"}}' "$TERRA_TOKEN_CL8Y" "$TERRA_CL8Y_MAX_PER_TX_STR" "$TERRA_CL8Y_MAX_PER_PERIOD_STR")"
  fi
fi

verify_cl8y_onchain_summary() {
  [[ "$VERIFY_CL8Y_ONCHAIN" == "1" ]] || return 0
  [[ "$INCLUDE_EVM" == "1" ]] || return 0

  echo ""
  echo "=== Read-only verification (CL8Y on EVM) ==="
  local zb="0x0000000000000000000000000000000000000000"
  local br_m br_b
  br_m="$(cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "rateLimitBridge()(address)" 2>/dev/null | sed -n '1p' | tr -d '\r')"
  br_b="$(cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "rateLimitBridge()(address)" 2>/dev/null | sed -n '1p' | tr -d '\r')"
  echo "MegaETH TokenRegistry.rateLimitBridge: $br_m"
  echo "BSC     TokenRegistry.rateLimitBridge: $br_b"
  if [[ "${br_m,,}" == "${zb,,}" ]] || [[ "${br_b,,}" == "${zb,,}" ]]; then
    echo "  WARN: rateLimitBridge is zero on at least one chain — TokenRegistry withdraw limits are not enforced until wired (megaeth-manager-followup wire_if_needed)." >&2
  fi

  echo "MegaETH tokenRegistered(CL8Y-cb): $(cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "tokenRegistered(address)(bool)" "$MEGAETH_TOKEN_CL8Y" 2>/dev/null | sed -n '1p')"
  echo "BSC     tokenRegistered(CL8Y):     $(cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "tokenRegistered(address)(bool)" "$BSC_TOKEN_CL8Y" 2>/dev/null | sed -n '1p')"

  echo "MegaETH getRateLimitConfig(CL8Y-cb) [minTx, maxTx, maxPeriod]:"
  cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "getRateLimitConfig(address)(uint256,uint256,uint256)" "$MEGAETH_TOKEN_CL8Y" 2>/dev/null || true
  echo "BSC getRateLimitConfig(CL8Y):"
  cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "getRateLimitConfig(address)(uint256,uint256,uint256)" "$BSC_TOKEN_CL8Y" 2>/dev/null || true

  echo "MegaETH TokenRateLimit deposit / withdraw limit (CL8Y-cb):"
  printf '  deposit: %s\n' "$(cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_RATE_LIMIT" "depositLimitPerToken(address)(uint256)" "$MEGAETH_TOKEN_CL8Y" 2>/dev/null | sed -n '1p')"
  printf '  withdraw: %s\n' "$(cast call --rpc-url "$MEGAETH_RPC" "$MEGAETH_TOKEN_RATE_LIMIT" "withdrawLimitPerToken(address)(uint256)" "$MEGAETH_TOKEN_CL8Y" 2>/dev/null | sed -n '1p')"
  echo "BSC TokenRateLimit deposit / withdraw limit (CL8Y):"
  printf '  deposit: %s\n' "$(cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_RATE_LIMIT" "depositLimitPerToken(address)(uint256)" "$BSC_TOKEN_CL8Y" 2>/dev/null | sed -n '1p')"
  printf '  withdraw: %s\n' "$(cast call --rpc-url "$BSC_RPC" "$BSC_TOKEN_RATE_LIMIT" "withdrawLimitPerToken(address)(uint256)" "$BSC_TOKEN_CL8Y" 2>/dev/null | sed -n '1p')"
}

verify_cl8y_onchain_summary

echo "MegaETH economic CL8Y mappings complete."
