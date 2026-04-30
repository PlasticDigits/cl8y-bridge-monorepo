#!/usr/bin/env bash
# Register MegaETH token mappings on EVM TokenRegistry contracts, Terra Classic, and Solana.
#
# Default scope:
#   - EVM TokenRegistry mappings for tokena/tokenb/tdec across MegaETH, BSC, opBNB, Terra, Solana
#   - Terra Classic set_token_destination + set_incoming_token_mapping for MegaETH
#   - Solana register_token mappings for MegaETH via register-mainnet-tokens.ts
#
# Token C is the MegaETH counterpart for the existing tdec token routes.
#
# Signing:
#   - EVM: interactive cast prompts. Use the EVM TokenRegistry owner/admin signer.
#   - Terra Classic: terrad with --keyring-backend file by default.
#   - Solana: decrypts ~/.config/solana/id-deployer.json.gpg, runs Anchor scripts, then shreds plaintext.

set -euo pipefail

export FOUNDRY_DISABLE_NIGHTLY_WARNING="${FOUNDRY_DISABLE_NIGHTLY_WARNING:-1}"

if [[ ! -c /dev/tty ]] || ! [[ -r /dev/tty && -w /dev/tty ]]; then
  echo "Interactive signing requires a TTY (/dev/tty); run from a real terminal." >&2
  exit 1
fi

MEGAETH_RPC="${MEGAETH_RPC:-https://mainnet.megaeth.com/rpc}"
BSC_RPC="${BSC_RPC:-https://bsc-dataseed1.binance.org}"
OPBNB_RPC="${OPBNB_RPC:-https://opbnb-mainnet-rpc.bnbchain.org}"

BSC_CHAIN_ID="${BSC_CHAIN_ID:-0x00000038}"
OPBNB_CHAIN_ID="${OPBNB_CHAIN_ID:-0x000000cc}"
TERRA_CHAIN_ID="${TERRA_CHAIN_ID:-0x00000001}"
SOLANA_CHAIN_ID="${SOLANA_CHAIN_ID:-0x00000005}"
MEGAETH_CHAIN_ID="${MEGAETH_CHAIN_ID:-0x000010e6}"

BSC_TOKEN_REGISTRY="${BSC_TOKEN_REGISTRY:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"
OPBNB_TOKEN_REGISTRY="${OPBNB_TOKEN_REGISTRY:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"
MEGAETH_TOKEN_REGISTRY="${MEGAETH_TOKEN_REGISTRY:-0x3d8820EC93748fd4df8eee6B763834a23938B207}"

BSC_TOKEN_A="${BSC_TOKEN_A:-0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c}"
BSC_TOKEN_B="${BSC_TOKEN_B:-0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52}"
BSC_TDEC="${BSC_TDEC:-0xe159c7a58d694fafba82221905d5a49e7f314330}"
OPBNB_TOKEN_A="${OPBNB_TOKEN_A:-0xF073d5685594F465a66EA54516f0D2f76b6cc6F3}"
OPBNB_TOKEN_B="${OPBNB_TOKEN_B:-0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e}"
OPBNB_TDEC="${OPBNB_TDEC:-0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd}"
MEGAETH_TOKEN_A="${MEGAETH_TOKEN_A:-0x7deF34032CC5D06bA84A8889bdCA7ee153127B23}"
MEGAETH_TOKEN_B="${MEGAETH_TOKEN_B:-0xE19442D99Aa2209b08d69c518444C4C1DAfeEDb1}"
MEGAETH_TOKEN_C="${MEGAETH_TOKEN_C:-0x840b1515f586c2ea31d55C91B355AFf36eA7af54}"

TERRA_TOKEN_A="${TERRA_TOKEN_A:-terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh}"
TERRA_TOKEN_B="${TERRA_TOKEN_B:-terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3}"
TERRA_TDEC="${TERRA_TDEC:-terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv}"
SOLANA_TOKEN_A="${SOLANA_TOKEN_A:-6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E}"
SOLANA_TOKEN_B="${SOLANA_TOKEN_B:-EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX}"
SOLANA_TDEC="${SOLANA_TDEC:-765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR}"

INCLUDE_TERRA="${INCLUDE_TERRA:-1}"
INCLUDE_SOLANA="${INCLUDE_SOLANA:-1}"
INCLUDE_EVM="${INCLUDE_EVM:-1}"
DRY_RUN="${DRY_RUN:-0}"

TERRA_NODE_URL="${TERRA_NODE_URL:-https://terra-classic-rpc.publicnode.com:443}"
TERRA_BRIDGE_CONTRACT="${TERRA_BRIDGE_CONTRACT:-terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la}"
TERRA_WALLET="${TERRA_WALLET:-cl8y2_admin}"
TERRA_KEYRING_BACKEND="${TERRA_KEYRING_BACKEND:-file}"
TERRA_FEES="${TERRA_FEES:-10000000uluna}"
TERRA_GAS_ADJUSTMENT="${TERRA_GAS_ADJUSTMENT:-1.5}"
TERRA_TX_SLEEP_SECONDS="${TERRA_TX_SLEEP_SECONDS:-6}"

SOLANA_PROGRAM_ID="${SOLANA_PROGRAM_ID:-4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt}"
ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL:-https://solana-rpc.publicnode.com}"
GPG_DEPLOYER="${GPG_DEPLOYER:-$HOME/.config/solana/id-deployer.json.gpg}"
SOLANA_KEYPAIR_PLAIN="${SOLANA_KEYPAIR_PLAIN:-$HOME/.config/solana/id-deployer.json}"
SHRED_SOLANA_KEY_AFTER="${SHRED_SOLANA_KEY_AFTER:-1}"

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

base58_b32() {
  python3 - "$1" <<'PY'
import sys

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
s = sys.argv[1].strip()
n = 0
for char in s:
    n = n * 58 + ALPHABET.index(char)
raw = n.to_bytes((n.bit_length() + 7) // 8, "big") if n else b""
raw = b"\0" * (len(s) - len(s.lstrip("1"))) + raw
if len(raw) != 32:
    raise SystemExit(f"expected 32-byte Solana pubkey, got {len(raw)} bytes")
print("0x" + raw.hex())
PY
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

solana_cleanup_needed=0
cleanup_solana_key() {
  if [[ "$solana_cleanup_needed" == "1" && "$SHRED_SOLANA_KEY_AFTER" == "1" ]]; then
    shred -u "$SOLANA_KEYPAIR_PLAIN" 2>/dev/null || rm -f "$SOLANA_KEYPAIR_PLAIN"
  fi
}
trap cleanup_solana_key EXIT

run_solana_mappings() {
  echo "=== Solana token mappings ==="
  if [[ "$DRY_RUN" == "1" ]]; then
    echo "DRY_RUN gpg --decrypt $GPG_DEPLOYER > $SOLANA_KEYPAIR_PLAIN"
    echo "DRY_RUN cd packages/contracts-solana && anchor build && npx tsx scripts/register-mainnet-chains.ts && npx tsx scripts/register-mainnet-tokens.ts"
    echo "DRY_RUN shred -u $SOLANA_KEYPAIR_PLAIN"
    return 0
  fi

  gpg --decrypt "$GPG_DEPLOYER" > "$SOLANA_KEYPAIR_PLAIN"
  chmod 600 "$SOLANA_KEYPAIR_PLAIN"
  solana_cleanup_needed=1

  export SOLANA_PROGRAM_ID
  export ANCHOR_PROVIDER_URL
  export SOLANA_KEYPAIR="$SOLANA_KEYPAIR_PLAIN"
  export ANCHOR_WALLET="$SOLANA_KEYPAIR"
  export MEGAETH_TOKEN_A MEGAETH_TOKEN_B MEGAETH_TOKEN_C

  (
    cd "$(dirname "$0")/../../packages/contracts-solana"
    anchor build
    npx tsx scripts/register-mainnet-chains.ts
    npx tsx scripts/register-mainnet-tokens.ts
  )
}

echo "=== MegaETH token mappings ==="
echo "DRY_RUN=$DRY_RUN INCLUDE_EVM=$INCLUDE_EVM INCLUDE_TERRA=$INCLUDE_TERRA INCLUDE_SOLANA=$INCLUDE_SOLANA"

BSC_TOKEN_A_B32="$(address_b32 "$BSC_TOKEN_A")"
BSC_TOKEN_B_B32="$(address_b32 "$BSC_TOKEN_B")"
BSC_TDEC_B32="$(address_b32 "$BSC_TDEC")"
OPBNB_TOKEN_A_B32="$(address_b32 "$OPBNB_TOKEN_A")"
OPBNB_TOKEN_B_B32="$(address_b32 "$OPBNB_TOKEN_B")"
OPBNB_TDEC_B32="$(address_b32 "$OPBNB_TDEC")"
MEGAETH_TOKEN_A_B32="$(address_b32 "$MEGAETH_TOKEN_A")"
MEGAETH_TOKEN_B_B32="$(address_b32 "$MEGAETH_TOKEN_B")"
MEGAETH_TOKEN_C_B32="$(address_b32 "$MEGAETH_TOKEN_C")"

if [[ "$INCLUDE_EVM" == "1" ]]; then
  echo "=== EVM TokenRegistry mappings ==="
  ensure_chain_registered "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "evm_56" "$BSC_CHAIN_ID"
  ensure_chain_registered "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "evm_204" "$OPBNB_CHAIN_ID"
  if [[ "$INCLUDE_TERRA" == "1" ]]; then
    ensure_chain_registered "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "terraclassic_columbus-5" "$TERRA_CHAIN_ID"
  fi
  if [[ "$INCLUDE_SOLANA" == "1" ]]; then
    ensure_chain_registered "MegaETH" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "solana_mainnet-beta" "$SOLANA_CHAIN_ID"
  fi
  ensure_chain_registered "BSC" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "evm_4326" "$MEGAETH_CHAIN_ID"
  ensure_chain_registered "opBNB" "$OPBNB_RPC" "$OPBNB_TOKEN_REGISTRY" "evm_4326" "$MEGAETH_CHAIN_ID"

  # MegaETH outgoing and incoming with BSC/opBNB.
  set_dest "MegaETH tokena -> BSC tokena" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_A" "$BSC_CHAIN_ID" "$BSC_TOKEN_A_B32" 18
  set_dest "MegaETH tokenb -> BSC tokenb" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_B" "$BSC_CHAIN_ID" "$BSC_TOKEN_B_B32" 18
  set_dest "MegaETH tokenc -> BSC tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_C" "$BSC_CHAIN_ID" "$BSC_TDEC_B32" 18
  set_dest "MegaETH tokena -> opBNB tokena" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_A" "$OPBNB_CHAIN_ID" "$OPBNB_TOKEN_A_B32" 18
  set_dest "MegaETH tokenb -> opBNB tokenb" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_B" "$OPBNB_CHAIN_ID" "$OPBNB_TOKEN_B_B32" 18
  set_dest "MegaETH tokenc -> opBNB tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_C" "$OPBNB_CHAIN_ID" "$OPBNB_TDEC_B32" 12
  set_incoming "MegaETH tokena <- BSC" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$BSC_CHAIN_ID" "$MEGAETH_TOKEN_A" 18
  set_incoming "MegaETH tokenb <- BSC" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$BSC_CHAIN_ID" "$MEGAETH_TOKEN_B" 18
  set_incoming "MegaETH tokenc <- BSC tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$BSC_CHAIN_ID" "$MEGAETH_TOKEN_C" 18
  set_incoming "MegaETH tokena <- opBNB" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$OPBNB_CHAIN_ID" "$MEGAETH_TOKEN_A" 18
  set_incoming "MegaETH tokenb <- opBNB" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$OPBNB_CHAIN_ID" "$MEGAETH_TOKEN_B" 18
  set_incoming "MegaETH tokenc <- opBNB tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$OPBNB_CHAIN_ID" "$MEGAETH_TOKEN_C" 12

  # Existing EVM chains outgoing to MegaETH, and incoming from MegaETH.
  set_dest "BSC tokena -> MegaETH tokena" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$BSC_TOKEN_A" "$MEGAETH_CHAIN_ID" "$MEGAETH_TOKEN_A_B32" 18
  set_dest "BSC tokenb -> MegaETH tokenb" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$BSC_TOKEN_B" "$MEGAETH_CHAIN_ID" "$MEGAETH_TOKEN_B_B32" 18
  set_dest "BSC tdec -> MegaETH tokenc" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$BSC_TDEC" "$MEGAETH_CHAIN_ID" "$MEGAETH_TOKEN_C_B32" 18
  set_incoming "BSC tokena <- MegaETH" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$MEGAETH_CHAIN_ID" "$BSC_TOKEN_A" 18
  set_incoming "BSC tokenb <- MegaETH" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$MEGAETH_CHAIN_ID" "$BSC_TOKEN_B" 18
  set_incoming "BSC tdec <- MegaETH tokenc" "$BSC_RPC" "$BSC_TOKEN_REGISTRY" "$MEGAETH_CHAIN_ID" "$BSC_TDEC" 18

  set_dest "opBNB tokena -> MegaETH tokena" "$OPBNB_RPC" "$OPBNB_TOKEN_REGISTRY" "$OPBNB_TOKEN_A" "$MEGAETH_CHAIN_ID" "$MEGAETH_TOKEN_A_B32" 18
  set_dest "opBNB tokenb -> MegaETH tokenb" "$OPBNB_RPC" "$OPBNB_TOKEN_REGISTRY" "$OPBNB_TOKEN_B" "$MEGAETH_CHAIN_ID" "$MEGAETH_TOKEN_B_B32" 18
  set_dest "opBNB tdec -> MegaETH tokenc" "$OPBNB_RPC" "$OPBNB_TOKEN_REGISTRY" "$OPBNB_TDEC" "$MEGAETH_CHAIN_ID" "$MEGAETH_TOKEN_C_B32" 18
  set_incoming "opBNB tokena <- MegaETH" "$OPBNB_RPC" "$OPBNB_TOKEN_REGISTRY" "$MEGAETH_CHAIN_ID" "$OPBNB_TOKEN_A" 18
  set_incoming "opBNB tokenb <- MegaETH" "$OPBNB_RPC" "$OPBNB_TOKEN_REGISTRY" "$MEGAETH_CHAIN_ID" "$OPBNB_TOKEN_B" 18
  set_incoming "opBNB tdec <- MegaETH tokenc" "$OPBNB_RPC" "$OPBNB_TOKEN_REGISTRY" "$MEGAETH_CHAIN_ID" "$OPBNB_TDEC" 18

  if [[ "$INCLUDE_TERRA" == "1" ]]; then
    TERRA_TOKEN_A_B32="$(terra_b32 "$TERRA_TOKEN_A")"
    TERRA_TOKEN_B_B32="$(terra_b32 "$TERRA_TOKEN_B")"
    TERRA_TDEC_B32="$(terra_b32 "$TERRA_TDEC")"
    set_dest "MegaETH tokena -> Terra tokena" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_A" "$TERRA_CHAIN_ID" "$TERRA_TOKEN_A_B32" 18
    set_dest "MegaETH tokenb -> Terra tokenb" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_B" "$TERRA_CHAIN_ID" "$TERRA_TOKEN_B_B32" 18
    set_dest "MegaETH tokenc -> Terra tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_C" "$TERRA_CHAIN_ID" "$TERRA_TDEC_B32" 6
    set_incoming "MegaETH tokena <- Terra" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$TERRA_CHAIN_ID" "$MEGAETH_TOKEN_A" 18
    set_incoming "MegaETH tokenb <- Terra" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$TERRA_CHAIN_ID" "$MEGAETH_TOKEN_B" 18
    set_incoming "MegaETH tokenc <- Terra tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$TERRA_CHAIN_ID" "$MEGAETH_TOKEN_C" 6
  fi

  if [[ "$INCLUDE_SOLANA" == "1" ]]; then
    SOLANA_TOKEN_A_B32="$(base58_b32 "$SOLANA_TOKEN_A")"
    SOLANA_TOKEN_B_B32="$(base58_b32 "$SOLANA_TOKEN_B")"
    SOLANA_TDEC_B32="$(base58_b32 "$SOLANA_TDEC")"
    set_dest "MegaETH tokena -> Solana tokena" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_A" "$SOLANA_CHAIN_ID" "$SOLANA_TOKEN_A_B32" 9
    set_dest "MegaETH tokenb -> Solana tokenb" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_B" "$SOLANA_CHAIN_ID" "$SOLANA_TOKEN_B_B32" 9
    set_dest "MegaETH tokenc -> Solana tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$MEGAETH_TOKEN_C" "$SOLANA_CHAIN_ID" "$SOLANA_TDEC_B32" 6
    set_incoming "MegaETH tokena <- Solana" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$SOLANA_CHAIN_ID" "$MEGAETH_TOKEN_A" 9
    set_incoming "MegaETH tokenb <- Solana" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$SOLANA_CHAIN_ID" "$MEGAETH_TOKEN_B" 9
    set_incoming "MegaETH tokenc <- Solana tdec" "$MEGAETH_RPC" "$MEGAETH_TOKEN_REGISTRY" "$SOLANA_CHAIN_ID" "$MEGAETH_TOKEN_C" 6
  fi
fi

if [[ "$INCLUDE_TERRA" == "1" ]]; then
  echo "=== Terra Classic token mappings ==="
  MEGAETH_CHAIN_B64="$(hex_b64 "$MEGAETH_CHAIN_ID")"
  TERRA_TOKEN_A_SRC_B64="$(terra_b32_b64 "$TERRA_TOKEN_A")"
  TERRA_TOKEN_B_SRC_B64="$(terra_b32_b64 "$TERRA_TOKEN_B")"
  TERRA_TDEC_SRC_B64="$(terra_b32_b64 "$TERRA_TDEC")"

  terra_tx "tokena -> MegaETH tokena" "$(printf '{"set_token_destination":{"token":"%s","dest_chain":"%s","dest_token":"%s","dest_decimals":18}}' "$TERRA_TOKEN_A" "$MEGAETH_CHAIN_B64" "$MEGAETH_TOKEN_A_B32")"
  terra_tx "tokenb -> MegaETH tokenb" "$(printf '{"set_token_destination":{"token":"%s","dest_chain":"%s","dest_token":"%s","dest_decimals":18}}' "$TERRA_TOKEN_B" "$MEGAETH_CHAIN_B64" "$MEGAETH_TOKEN_B_B32")"
  terra_tx "tdec -> MegaETH tokenc" "$(printf '{"set_token_destination":{"token":"%s","dest_chain":"%s","dest_token":"%s","dest_decimals":18}}' "$TERRA_TDEC" "$MEGAETH_CHAIN_B64" "$MEGAETH_TOKEN_C_B32")"

  # Terra incoming mapping keys use the local Terra token bytes32, matching WithdrawSubmit validation.
  terra_tx "tokena <- MegaETH" "$(printf '{"set_incoming_token_mapping":{"src_chain":"%s","src_token":"%s","local_token":"%s","src_decimals":18}}' "$MEGAETH_CHAIN_B64" "$TERRA_TOKEN_A_SRC_B64" "$TERRA_TOKEN_A")"
  terra_tx "tokenb <- MegaETH" "$(printf '{"set_incoming_token_mapping":{"src_chain":"%s","src_token":"%s","local_token":"%s","src_decimals":18}}' "$MEGAETH_CHAIN_B64" "$TERRA_TOKEN_B_SRC_B64" "$TERRA_TOKEN_B")"
  terra_tx "tdec <- MegaETH tokenc" "$(printf '{"set_incoming_token_mapping":{"src_chain":"%s","src_token":"%s","local_token":"%s","src_decimals":18}}' "$MEGAETH_CHAIN_B64" "$TERRA_TDEC_SRC_B64" "$TERRA_TDEC")"
fi

if [[ "$INCLUDE_SOLANA" == "1" ]]; then
  run_solana_mappings
fi

echo "MegaETH token mappings complete."
