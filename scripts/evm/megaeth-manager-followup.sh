#!/usr/bin/env bash
# MegaETH post-parity manager follow-up.
#
# Run after scripts/evm/megaeth-parity-quickstart.sh has completed. This script
# intentionally does not deploy parity contracts; it only sends owner/admin
# configuration transactions that would change the historical deployer nonce if
# included in the parity broadcast.

set -euo pipefail

if [[ ! -c /dev/tty ]] || ! [[ -r /dev/tty && -w /dev/tty ]]; then
  echo "Interactive signing requires a TTY (/dev/tty); run from a real terminal." >&2
  exit 1
fi

RPC_URL="${RPC_URL:-https://mainnet.megaeth.com/rpc}"
MANAGER_ADDRESS="${MANAGER_ADDRESS:-0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c}"
CANCELER_ADDRESS="${CANCELER_ADDRESS:-0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB}"

CHAIN_REGISTRY_ADDRESS="${CHAIN_REGISTRY_ADDRESS:-0x2e5D36C46680A38e7Ae156fc9d109084C58c688e}"
TOKEN_REGISTRY_ADDRESS="${TOKEN_REGISTRY_ADDRESS:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"
MINT_BURN_ADDRESS="${MINT_BURN_ADDRESS:-0x0A1a4bd354983DBc7f487237CD1B408CD0003EBC}"
BRIDGE_ADDRESS="${BRIDGE_ADDRESS:-0xb2A22c74dA8E3642e0EffC107d3Ac362ce885369}"

FACTORY_ADDRESS="${FACTORY_ADDRESS:-0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F}"
FACTORY_AUTHORITY_ADDRESS="${FACTORY_AUTHORITY_ADDRESS:-0xa958d75c61227606df21e3261ba80dc399d19676}"

GUARD_ACCESS_MANAGER_ADDRESS="${GUARD_ACCESS_MANAGER_ADDRESS:-0xa958d75c61227606df21e3261ba80dc399d19676}"
TOKEN_RATE_LIMIT_ADDRESS="${TOKEN_RATE_LIMIT_ADDRESS:-0xD72b2fe3012a2896aef7E3cA561aD11B1542a88c}"
GUARD_BRIDGE_ADDRESS="${GUARD_BRIDGE_ADDRESS:-0x12FEDD29E71F66157E985AA1aAAE434253E39A22}"

TOKEN_A_NAME="${TOKEN_A_NAME:-Token A V2}"
TOKEN_A_SYMBOL="${TOKEN_A_SYMBOL:-tokena}"
TOKEN_A_LOGO="${TOKEN_A_LOGO:-}"
TOKEN_B_NAME="${TOKEN_B_NAME:-Token B V2}"
TOKEN_B_SYMBOL="${TOKEN_B_SYMBOL:-tokenb}"
TOKEN_B_LOGO="${TOKEN_B_LOGO:-}"
TOKEN_C_NAME="${TOKEN_C_NAME:-Token C V2}"
TOKEN_C_SYMBOL="${TOKEN_C_SYMBOL:-tokenc}"
TOKEN_C_LOGO="${TOKEN_C_LOGO:-}"

SKIP_PEERS="${SKIP_PEERS:-0}"
SKIP_TOKENS="${SKIP_TOKENS:-0}"
SKIP_WIRING="${SKIP_WIRING:-0}"

CREATE_TOKEN_SELECTOR=0x94aed7d0
MINT_SELECTOR=0x40c10f19
BURN_SELECTOR=0x9dc29fac
ROLE_MINT_BURN=1
TOKEN_TYPE_MINT_BURN=1
ZERO_ADDRESS=0x0000000000000000000000000000000000000000

send() {
  cast send --interactive --rpc-url "$RPC_URL" "$@" </dev/tty
}

call() {
  cast call --rpc-url "$RPC_URL" "$@"
}

code_is_present() {
  local code
  code="$(cast code "$1" --rpc-url "$RPC_URL")"
  [[ "$code" != "0x" ]]
}

bool_call() {
  local out
  out="$(call "$@")"
  [[ "$(printf '%s\n' "$out" | sed -n '1p')" == "true" ]]
}

uint_call() {
  call "$@" | sed -n '1p'
}

address_call() {
  call "$@" | sed -n '1p'
}

require_code() {
  local label="$1" address="$2"
  if ! code_is_present "$address"; then
    echo "$label has no code at $address on $RPC_URL" >&2
    exit 1
  fi
}

register_peer_if_needed() {
  local label="$1" ident="$2" bytes4_id="$3"
  if bool_call "$CHAIN_REGISTRY_ADDRESS" "isChainRegistered(bytes4)(bool)" "$bytes4_id"; then
    echo "Peer already registered: $label ($bytes4_id)"
    return 0
  fi

  echo "Registering peer: $label ($ident / $bytes4_id)"
  send "$CHAIN_REGISTRY_ADDRESS" "registerChain(string,bytes4)" "$ident" "$bytes4_id"
}

create_or_read_token() {
  local index="$1" name="$2" symbol="$3" logo="$4"
  local count
  count="$(uint_call "$FACTORY_ADDRESS" "getTokensCount()(uint256)")"

  if (( count <= index )); then
    echo "Creating factory token index $index: $name / $symbol"
    send "$FACTORY_ADDRESS" "createToken(string,string,string)" "$name" "$symbol" "$logo"
  else
    echo "Factory token index $index already exists; reading it"
  fi

  address_call "$FACTORY_ADDRESS" "getTokenAt(uint256)(address)" "$index"
}

register_token_if_needed() {
  local token="$1"
  if bool_call "$TOKEN_REGISTRY_ADDRESS" "tokenRegistered(address)(bool)" "$token"; then
    echo "Token already registered: $token"
    return 0
  fi

  echo "Registering MintBurn token: $token"
  send "$TOKEN_REGISTRY_ADDRESS" "registerToken(address,uint8)" "$token" "$TOKEN_TYPE_MINT_BURN"
}

authorize_mint_burn_if_needed() {
  local token="$1"
  if bool_call "$FACTORY_AUTHORITY_ADDRESS" "canCall(address,address,bytes4)(bool,uint32)" "$MINT_BURN_ADDRESS" "$token" "$MINT_SELECTOR"; then
    echo "MintBurn already authorized for token: $token"
    return 0
  fi

  echo "Granting AccessManager role $ROLE_MINT_BURN to MintBurn"
  send "$FACTORY_AUTHORITY_ADDRESS" "grantRole(uint64,address,uint32)" "$ROLE_MINT_BURN" "$MINT_BURN_ADDRESS" 0

  echo "Mapping mint/burn selectors to role $ROLE_MINT_BURN on $token"
  send "$FACTORY_AUTHORITY_ADDRESS" \
    "setTargetFunctionRole(address,bytes4[],uint64)" \
    "$token" "[$MINT_SELECTOR,$BURN_SELECTOR]" "$ROLE_MINT_BURN"
}

wire_if_needed() {
  local current

  current="$(address_call "$TOKEN_REGISTRY_ADDRESS" "rateLimitBridge()(address)")"
  if [[ "${current,,}" != "${BRIDGE_ADDRESS,,}" ]]; then
    echo "Setting TokenRegistry.rateLimitBridge -> $BRIDGE_ADDRESS"
    send "$TOKEN_REGISTRY_ADDRESS" "setRateLimitBridge(address)" "$BRIDGE_ADDRESS"
  else
    echo "TokenRegistry.rateLimitBridge already set"
  fi

  current="$(address_call "$BRIDGE_ADDRESS" "guardBridge()(address)")"
  if [[ "${current,,}" != "${GUARD_BRIDGE_ADDRESS,,}" ]]; then
    echo "Setting Bridge.guardBridge -> $GUARD_BRIDGE_ADDRESS"
    send "$BRIDGE_ADDRESS" "setGuardBridge(address)" "$GUARD_BRIDGE_ADDRESS"
  else
    echo "Bridge.guardBridge already set"
  fi

  if bool_call "$BRIDGE_ADDRESS" "isCanceler(address)(bool)" "$CANCELER_ADDRESS"; then
    echo "Canceler already registered: $CANCELER_ADDRESS"
  else
    echo "Registering canceler: $CANCELER_ADDRESS"
    send "$BRIDGE_ADDRESS" "addCanceler(address)" "$CANCELER_ADDRESS"
  fi
}

wire_guard_modules_if_needed() {
  local datastore deposit_set withdraw_set
  datastore="$(address_call "$GUARD_BRIDGE_ADDRESS" "DATASTORE_ADDRESS()(address)")"
  deposit_set="$(cast keccak "GUARD_MODULES_DEPOSIT")"
  withdraw_set="$(cast keccak "GUARD_MODULES_WITHDRAW")"

  if bool_call "$datastore" "contains(address,bytes32,address)(bool)" "$GUARD_BRIDGE_ADDRESS" "$deposit_set" "$TOKEN_RATE_LIMIT_ADDRESS"; then
    echo "TokenRateLimit already registered for deposit guard"
  else
    echo "Registering TokenRateLimit as deposit guard"
    send "$GUARD_BRIDGE_ADDRESS" "addGuardModuleDeposit(address)" "$TOKEN_RATE_LIMIT_ADDRESS"
  fi

  if bool_call "$datastore" "contains(address,bytes32,address)(bool)" "$GUARD_BRIDGE_ADDRESS" "$withdraw_set" "$TOKEN_RATE_LIMIT_ADDRESS"; then
    echo "TokenRateLimit already registered for withdraw guard"
  else
    echo "Registering TokenRateLimit as withdraw guard"
    send "$GUARD_BRIDGE_ADDRESS" "addGuardModuleWithdraw(address)" "$TOKEN_RATE_LIMIT_ADDRESS"
  fi
}

echo "=== MegaETH manager follow-up ==="
echo "RPC_URL=$RPC_URL"
echo "MANAGER_ADDRESS=$MANAGER_ADDRESS"

require_code "ChainRegistry" "$CHAIN_REGISTRY_ADDRESS"
require_code "TokenRegistry" "$TOKEN_REGISTRY_ADDRESS"
require_code "MintBurn" "$MINT_BURN_ADDRESS"
require_code "Bridge" "$BRIDGE_ADDRESS"
require_code "Guard AccessManager" "$GUARD_ACCESS_MANAGER_ADDRESS"
require_code "TokenRateLimit" "$TOKEN_RATE_LIMIT_ADDRESS"
require_code "GuardBridge" "$GUARD_BRIDGE_ADDRESS"

if [[ "$SKIP_PEERS" != "1" ]]; then
  register_peer_if_needed "BSC mainnet-equivalent" "evm_56" "0x00000038"
  register_peer_if_needed "opBNB mainnet-equivalent" "evm_204" "0x000000cc"
  register_peer_if_needed "Terra Classic" "terraclassic_columbus-5" "0x00000001"
  register_peer_if_needed "Solana mainnet-beta" "solana_mainnet-beta" "0x00000005"
fi

if [[ "$SKIP_WIRING" != "1" ]]; then
  wire_guard_modules_if_needed
  wire_if_needed
fi

if [[ "$SKIP_TOKENS" != "1" ]]; then
  require_code "FactoryTokenCl8yBridged" "$FACTORY_ADDRESS"
  require_code "Factory authority AccessManager" "$FACTORY_AUTHORITY_ADDRESS"

  if ! bool_call "$FACTORY_AUTHORITY_ADDRESS" "canCall(address,address,bytes4)(bool,uint32)" "$MANAGER_ADDRESS" "$FACTORY_ADDRESS" "$CREATE_TOKEN_SELECTOR"; then
    echo "MANAGER_ADDRESS is not authorized to call FactoryTokenCl8yBridged.createToken." >&2
    echo "Authorize it on FACTORY_AUTHORITY_ADDRESS before rerunning, or use the authorized manager signer." >&2
    exit 1
  fi

  MEGAETH_TOKEN_A="$(create_or_read_token 0 "$TOKEN_A_NAME" "$TOKEN_A_SYMBOL" "$TOKEN_A_LOGO" | tail -n 1)"
  MEGAETH_TOKEN_B="$(create_or_read_token 1 "$TOKEN_B_NAME" "$TOKEN_B_SYMBOL" "$TOKEN_B_LOGO" | tail -n 1)"
  MEGAETH_TOKEN_C="$(create_or_read_token 2 "$TOKEN_C_NAME" "$TOKEN_C_SYMBOL" "$TOKEN_C_LOGO" | tail -n 1)"

  for token in "$MEGAETH_TOKEN_A" "$MEGAETH_TOKEN_B" "$MEGAETH_TOKEN_C"; do
    register_token_if_needed "$token"
    authorize_mint_burn_if_needed "$token"
  done

  echo ""
  echo "=== Token exports ==="
  echo "export MEGAETH_TOKEN_A=$MEGAETH_TOKEN_A"
  echo "export MEGAETH_TOKEN_B=$MEGAETH_TOKEN_B"
  echo "export MEGAETH_TOKEN_C=$MEGAETH_TOKEN_C"
fi

echo ""
echo "=== Verification ==="
echo "rateLimitBridge: $(address_call "$TOKEN_REGISTRY_ADDRESS" "rateLimitBridge()(address)")"
echo "guardBridge:     $(address_call "$BRIDGE_ADDRESS" "guardBridge()(address)")"
echo "canceler count:  $(uint_call "$BRIDGE_ADDRESS" "getCancelerCount()(uint256)")"
echo "factory count:   $(uint_call "$FACTORY_ADDRESS" "getTokensCount()(uint256)" 2>/dev/null || echo "unavailable")"
echo "Manager follow-up complete."
