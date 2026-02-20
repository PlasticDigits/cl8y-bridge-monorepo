#!/bin/bash
# Full interactive Terra Classic bridge deployment
#
# Deploys in order:
#   1. Bridge contract (store + instantiate)
#   2. CW20 test tokens (testa 18dec, testb 18dec, tdec 6dec) with bridge as minter
#   3. Faucet contract (store + instantiate) with minter permissions on CW20s
#   4. Register EVM chains (BSC + opBNB)
#   5. Add tokens (uluna native + 3 CW20 tokens)
#   6. Set token destinations (outgoing to BSC + opBNB)
#   7. Set incoming token mappings (from BSC + opBNB)
#   8. Register operator + cancelers
#
# Prerequisites:
#   - terrad CLI installed
#   - Key in OS keyring with LUNA for gas
#   - WASM artifacts built (make build-terra-optimized)
#   - python3 with bech32 + base64 (pip install bech32)
#   - EVM deployment addresses (from deploy-evm-full.sh)
#   - cw20-mintable already stored on-chain (Code ID 10184)
#
# Usage:
#   ./scripts/deploy-terra-full.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="$PROJECT_ROOT/packages/contracts-terraclassic/artifacts"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1" >&2; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1" >&2; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }
log_step()  { echo -e "${BLUE}[STEP]${NC} $1" >&2; }
log_header() {
    echo "" >&2
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}" >&2
    echo -e "${CYAN}║  $1" >&2
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}" >&2
    echo "" >&2
}

prompt_continue() {
    echo "" >&2
    read -p "$(echo -e "${YELLOW}Press Enter to continue (or Ctrl+C to abort)...${NC}")" _ </dev/tty
    echo "" >&2
}

prompt_value() {
    local varname="$1"
    local description="$2"
    read -p "$(echo -e "${YELLOW}Enter $description: ${NC}")" value </dev/tty
    eval "export $varname=\"$value\""
}

# ─── Configuration ────────────────────────────────────────────────────────────
NODE="https://terra-classic-rpc.publicnode.com:443"
CHAIN_ID="columbus-5"
KEYRING="os"
GAS_FLAGS="--gas auto --gas-adjustment 1.5"
TX_FLAGS="--chain-id $CHAIN_ID --node $NODE --keyring-backend $KEYRING"

# Bridge chain IDs
BRIDGE_BSC_CHAIN_ID_B64="AAAAOA=="    # 0x00000038
BRIDGE_OPBNB_CHAIN_ID_B64="AAAAzA=="  # 0x000000cc
BRIDGE_TERRA_CHAIN_ID_B64="AAAAAQ=="  # 0x00000001

# Bridge configuration
WITHDRAW_DELAY=300
MIN_SIGNATURES=1
MIN_BRIDGE_AMOUNT="1000000"
MAX_BRIDGE_AMOUNT="1000000000000"
FEE_BPS=30

# Submit a terrad tx and set TX_HASH. Retries on failure.
# All user-facing output goes to stderr so $() capture isn't polluted.
submit_tx() {
    local label="$1"
    shift
    local tx_output

    while true; do
        log_step "Submitting: $label"
        tx_output=$("$@" -y -o json) || true

        TX_HASH=$(echo "$tx_output" | jq -r '.txhash // empty' 2>/dev/null)
        local CODE=$(echo "$tx_output" | jq -r '.code // 0' 2>/dev/null)

        if [ -z "$TX_HASH" ]; then
            log_error "No txhash returned for $label"
            echo "$tx_output" >&2
            echo "" >&2
            read -p "$(echo -e "${YELLOW}Retry? [Y/n]: ${NC}")" retry </dev/tty
            [[ "$retry" == "n" || "$retry" == "N" ]] && exit 1
            continue
        fi

        if [ "$CODE" != "0" ] && [ "$CODE" != "null" ] && [ -n "$CODE" ]; then
            local RAW_LOG=$(echo "$tx_output" | jq -r '.raw_log // empty' 2>/dev/null)
            log_error "$label rejected (code $CODE): $RAW_LOG"
            echo "" >&2
            read -p "$(echo -e "${YELLOW}Retry? [Y/n]: ${NC}")" retry </dev/tty
            [[ "$retry" == "n" || "$retry" == "N" ]] && exit 1
            continue
        fi

        log_info "$label TX: $TX_HASH"
        break
    done

    log_info "Waiting for confirmation..."
    sleep 15
}

# Store a WASM binary and extract the code ID
store_wasm() {
    local wasm_file="$1"
    local label="$2"
    local fees="${3:-1500000000uluna}"

    submit_tx "$label store" \
        terrad tx wasm store "$wasm_file" \
        --from "$TERRA_KEY_NAME" \
        $TX_FLAGS \
        $GAS_FLAGS \
        --fees "$fees"

    local CODE_ID
    CODE_ID=$(terrad query tx "$TX_HASH" --node "$NODE" -o json 2>/dev/null | \
        jq -r '.events[] | select(.type=="store_code") | .attributes[] | select(.key=="code_id") | .value // empty')

    if [ -z "$CODE_ID" ]; then
        log_warn "Could not auto-extract code_id. Check TX manually:"
        log_warn "  terrad query tx $TX_HASH --node $NODE"
        prompt_value "CODE_ID" "code_id from transaction"
    fi

    log_info "$label code_id: $CODE_ID"
    echo "$CODE_ID"
}

# Instantiate a contract and return the contract address
instantiate_contract() {
    local code_id="$1"
    local init_msg="$2"
    local label="$3"
    local admin="$4"
    local fees="${5:-500000000uluna}"

    submit_tx "$label instantiate" \
        terrad tx wasm instantiate "$code_id" "$init_msg" \
        --from "$TERRA_KEY_NAME" \
        --label "$label" \
        --admin "$admin" \
        $TX_FLAGS \
        $GAS_FLAGS \
        --fees "$fees"

    local CONTRACT
    CONTRACT=$(terrad query tx "$TX_HASH" --node "$NODE" -o json 2>/dev/null | \
        jq -r '.events[] | select(.type=="instantiate") | .attributes[] | select(.key=="_contract_address") | .value // empty')

    if [ -z "$CONTRACT" ]; then
        log_warn "Could not auto-extract contract address. Check TX manually:"
        log_warn "  terrad query tx $TX_HASH --node $NODE"
        prompt_value "CONTRACT" "contract address from transaction"
    fi

    log_info "$label address: $CONTRACT"
    echo "$CONTRACT"
}

# Execute a contract message (as deployer)
terra_execute() {
    local contract="$1"
    local msg="$2"
    local label="$3"
    local fees="${4:-100000000uluna}"

    submit_tx "$label" \
        terrad tx wasm execute "$contract" "$msg" \
        --from "$TERRA_KEY_NAME" \
        $TX_FLAGS \
        $GAS_FLAGS \
        --fees "$fees"
}

# Execute a contract message (as admin)
terra_admin_execute() {
    local contract="$1"
    local msg="$2"
    local label="$3"
    local fees="${4:-100000000uluna}"

    submit_tx "$label" \
        terrad tx wasm execute "$contract" "$msg" \
        --from "$TERRA_ADMIN_KEY" \
        $TX_FLAGS \
        $GAS_FLAGS \
        --fees "$fees"
}

# ─── Prereq checks ───────────────────────────────────────────────────────────
check_prereqs() {
    log_step "Checking prerequisites..."

    for cmd in terrad python3 jq; do
        if ! command -v $cmd &>/dev/null; then
            log_error "$cmd is required but not installed"
            exit 1
        fi
    done

    python3 -c "import bech32" 2>/dev/null || {
        log_error "python3 bech32 package required: pip install bech32"
        exit 1
    }

    # Prompt for any missing config interactively
    if [ -z "$TERRA_KEY_NAME" ]; then
        prompt_value "TERRA_KEY_NAME" "terrad keyring key name (e.g. deployer)"
    fi
    if [ -z "$TERRA_ADMIN" ]; then
        prompt_value "TERRA_ADMIN" "admin address (terra1..., multi-sig recommended)"
    fi
    if [ -z "$TERRA_FEE_COLLECTOR" ]; then
        prompt_value "TERRA_FEE_COLLECTOR" "fee collector address (terra1...)"
    fi
    if [ -z "$TERRA_ADMIN_KEY" ]; then
        prompt_value "TERRA_ADMIN_KEY" "admin keyring key name (signs admin txs, e.g. cl8y_admin)"
    fi

    if ! terrad keys show "$TERRA_KEY_NAME" --keyring-backend "$KEYRING" &>/dev/null; then
        log_error "Key '$TERRA_KEY_NAME' not found in $KEYRING keyring"
        exit 1
    fi
    if ! terrad keys show "$TERRA_ADMIN_KEY" --keyring-backend "$KEYRING" &>/dev/null; then
        log_error "Key '$TERRA_ADMIN_KEY' not found in $KEYRING keyring"
        exit 1
    fi

    if [ ! -f "$ARTIFACTS_DIR/bridge.wasm" ]; then
        log_error "bridge.wasm not found at $ARTIFACTS_DIR/bridge.wasm"
        log_error "Run: make build-terra-optimized"
        exit 1
    fi

    log_info "TERRA_ADMIN:         $TERRA_ADMIN"
    log_info "TERRA_ADMIN_KEY:     $TERRA_ADMIN_KEY"
    log_info "TERRA_KEY_NAME:      $TERRA_KEY_NAME"
    log_info "TERRA_FEE_COLLECTOR: $TERRA_FEE_COLLECTOR"
    log_info "Prerequisites OK"
}

# ─── Phase 1: Deploy bridge ──────────────────────────────────────────────────
deploy_bridge() {
    log_header "Phase 1: Deploy Bridge Contract"

    if [ -n "$BRIDGE_CODE_ID" ]; then
        log_info "Using existing bridge code_id: $BRIDGE_CODE_ID (skipping store)"
    else
        BRIDGE_CODE_ID=$(store_wasm "$ARTIFACTS_DIR/bridge.wasm" "bridge")
    fi

    if [ -z "$TERRA_OPERATORS" ]; then
        prompt_value "TERRA_OPERATORS" "operator address(es), comma-separated (terra1...)"
    fi

    IFS=',' read -ra OPS <<< "$TERRA_OPERATORS"
    local OPS_JSON=$(printf '%s\n' "${OPS[@]}" | jq -R . | jq -s -c .)

    local INIT_MSG=$(cat <<EOFMSG
{
    "admin": "$TERRA_ADMIN",
    "operators": $OPS_JSON,
    "min_signatures": $MIN_SIGNATURES,
    "min_bridge_amount": "$MIN_BRIDGE_AMOUNT",
    "max_bridge_amount": "$MAX_BRIDGE_AMOUNT",
    "fee_bps": $FEE_BPS,
    "fee_collector": "$TERRA_FEE_COLLECTOR",
    "this_chain_id": "$BRIDGE_TERRA_CHAIN_ID_B64"
}
EOFMSG
)

    if [ -n "$TERRA_BRIDGE" ]; then
        log_info "Using existing bridge: $TERRA_BRIDGE (skipping instantiate)"
    else
        log_info "Instantiation message:"
        echo "$INIT_MSG" | jq . >&2

        TERRA_BRIDGE=$(instantiate_contract "$BRIDGE_CODE_ID" "$INIT_MSG" "cl8y-bridge-v2" "$TERRA_ADMIN" "100000000uluna")
        export TERRA_BRIDGE

        log_info "Bridge deployed: $TERRA_BRIDGE"
    fi
}

# ─── Phase 2: Deploy CW20 tokens ─────────────────────────────────────────────
deploy_cw20_tokens() {
    log_header "Phase 2: Deploy CW20 Test Tokens (cw20-mintable)"

    # cw20-mintable is already stored on Terra Classic mainnet
    CW20_CODE_ID=10184
    log_info "Using existing cw20-mintable Code ID: $CW20_CODE_ID"

    # Deployer is the primary minter so we can add_minter for bridge + faucet later
    local DEPLOYER_ADDR
    DEPLOYER_ADDR=$(terrad keys show "$TERRA_KEY_NAME" -a --keyring-backend "$KEYRING")
    log_info "Primary minter (deployer): $DEPLOYER_ADDR"

    # testa (18 decimals)
    if [ -n "$TERRA_TESTA" ]; then
        log_info "Using existing testa: $TERRA_TESTA"
    else
        log_step "Instantiating testa (18 decimals)..."
        TERRA_TESTA=$(instantiate_contract "$CW20_CODE_ID" \
            '{"name":"Test A","symbol":"testa","decimals":18,"initial_balances":[],"mint":{"minter":"'"$DEPLOYER_ADDR"'"}}' \
            "testa-cw20" "$TERRA_ADMIN")
        export TERRA_TESTA
    fi

    # testb (18 decimals)
    if [ -n "$TERRA_TESTB" ]; then
        log_info "Using existing testb: $TERRA_TESTB"
    else
        log_step "Instantiating testb (18 decimals)..."
        TERRA_TESTB=$(instantiate_contract "$CW20_CODE_ID" \
            '{"name":"Test B","symbol":"testb","decimals":18,"initial_balances":[],"mint":{"minter":"'"$DEPLOYER_ADDR"'"}}' \
            "testb-cw20" "$TERRA_ADMIN")
        export TERRA_TESTB
    fi

    # tdec (6 decimals on Terra)
    if [ -n "$TERRA_TDEC" ]; then
        log_info "Using existing tdec: $TERRA_TDEC"
    else
        log_step "Instantiating tdec (6 decimals)..."
        TERRA_TDEC=$(instantiate_contract "$CW20_CODE_ID" \
            '{"name":"Test Dec","symbol":"tdec","decimals":6,"initial_balances":[],"mint":{"minter":"'"$DEPLOYER_ADDR"'"}}' \
            "tdec-cw20" "$TERRA_ADMIN")
        export TERRA_TDEC
    fi

    # Add bridge as minter on each CW20 token
    log_step "Adding bridge as minter on each CW20 token..."
    for addr_label in "$TERRA_TESTA:testa" "$TERRA_TESTB:testb" "$TERRA_TDEC:tdec"; do
        IFS=: read -r addr lbl <<< "$addr_label"
        terra_execute "$addr" \
            '{"add_minter":{"minter":"'"$TERRA_BRIDGE"'"}}' \
            "$lbl AddMinter(bridge)"
    done
    log_info "Bridge added as minter on all CW20 tokens"

    echo "" >&2
    echo "═══════════════════════════════════════" >&2
    echo "  CW20 Tokens Deployed" >&2
    echo "═══════════════════════════════════════" >&2
    echo "  testa (18 dec): $TERRA_TESTA" >&2
    echo "  testb (18 dec): $TERRA_TESTB" >&2
    echo "  tdec  (6 dec):  $TERRA_TDEC" >&2
    echo "═══════════════════════════════════════" >&2

    # Verify
    log_step "Verifying token contracts..."
    for addr_label in "$TERRA_TESTA:testa" "$TERRA_TESTB:testb" "$TERRA_TDEC:tdec"; do
        IFS=: read -r addr lbl <<< "$addr_label"
        local INFO=$(terrad query wasm contract-state smart "$addr" '{"token_info":{}}' --node "$NODE" -o json 2>/dev/null)
        local NAME=$(echo "$INFO" | jq -r '.data.name // "?"')
        local DEC=$(echo "$INFO" | jq -r '.data.decimals // "?"')
        log_info "$lbl → name=$NAME decimals=$DEC"
    done
}

# ─── Phase 3: Deploy faucet ──────────────────────────────────────────────────
deploy_faucet() {
    log_header "Phase 3: Deploy Faucet Contract"

    if [ -n "$FAUCET_CODE_ID" ]; then
        log_info "Using existing faucet code_id: $FAUCET_CODE_ID (skipping store)"
    else
        local FAUCET_WASM="$ARTIFACTS_DIR/faucet.wasm"
        if [ ! -f "$FAUCET_WASM" ]; then
            log_warn "faucet.wasm not found at $FAUCET_WASM"
            log_warn "Skipping faucet deployment. Deploy manually later."
            return
        fi
        FAUCET_CODE_ID=$(store_wasm "$FAUCET_WASM" "faucet")
    fi

    local DEPLOYER_ADDR=$(terrad keys show "$TERRA_KEY_NAME" -a --keyring-backend "$KEYRING")

    local FAUCET_INIT=$(cat <<EOFMSG
{
    "admin": "$TERRA_ADMIN",
    "tokens": [
        {"address": "$TERRA_TESTA", "decimals": 18},
        {"address": "$TERRA_TESTB", "decimals": 18},
        {"address": "$TERRA_TDEC", "decimals": 6}
    ]
}
EOFMSG
)

    TERRA_FAUCET=$(instantiate_contract "$FAUCET_CODE_ID" "$FAUCET_INIT" "cl8y-faucet" "$TERRA_ADMIN")
    export TERRA_FAUCET
    log_info "Faucet deployed: $TERRA_FAUCET"

    # Deployer is the primary minter — add faucet as an additional minter.
    log_step "Adding faucet as minter on each CW20 token..."
    for addr_label in "$TERRA_TESTA:testa" "$TERRA_TESTB:testb" "$TERRA_TDEC:tdec"; do
        IFS=: read -r addr lbl <<< "$addr_label"
        terra_execute "$addr" \
            '{"add_minter":{"minter":"'"$TERRA_FAUCET"'"}}' \
            "$lbl AddMinter(faucet)"
    done
    log_info "Faucet added as minter on all CW20 tokens"
}

# ─── Phase 4: Register EVM chains ────────────────────────────────────────────
register_chains() {
    log_header "Phase 4: Register EVM Chains"

    terra_admin_execute "$TERRA_BRIDGE" \
        '{"register_chain":{"identifier":"evm_56","chain_id":"'"$BRIDGE_BSC_CHAIN_ID_B64"'"}}' \
        "Register BSC (0x00000038)"

    terra_admin_execute "$TERRA_BRIDGE" \
        '{"register_chain":{"identifier":"evm_204","chain_id":"'"$BRIDGE_OPBNB_CHAIN_ID_B64"'"}}' \
        "Register opBNB (0x000000cc)"
}

# ─── Phase 5: Add tokens ─────────────────────────────────────────────────────
add_tokens() {
    log_header "Phase 5: Add Tokens to Bridge"

    # uluna (native)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"add_token":{"token":"uluna","is_native":true,"token_type":null,"terra_decimals":6}}' \
        "Add uluna (native, 6 dec)"

    # testa (CW20, 18 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"add_token":{"token":"'"$TERRA_TESTA"'","is_native":false,"token_type":"mint_burn","terra_decimals":18}}' \
        "Add testa (CW20, 18 dec)"

    # testb (CW20, 18 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"add_token":{"token":"'"$TERRA_TESTB"'","is_native":false,"token_type":"mint_burn","terra_decimals":18}}' \
        "Add testb (CW20, 18 dec)"

    # tdec (CW20, 6 dec on Terra)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"add_token":{"token":"'"$TERRA_TDEC"'","is_native":false,"token_type":"mint_burn","terra_decimals":6}}' \
        "Add tdec (CW20, 6 dec)"
}

# ─── Phase 6: Set token destinations (outgoing) ──────────────────────────────
set_token_destinations() {
    log_header "Phase 6: Token Destinations (Outgoing)"

    if [ -z "$BSC_TESTA" ]; then
        prompt_value "BSC_TESTA" "BSC tokena address (0x...)"
    fi
    if [ -z "$BSC_TESTB" ]; then
        prompt_value "BSC_TESTB" "BSC tokenb address (0x...)"
    fi
    if [ -z "$BSC_TDEC" ]; then
        prompt_value "BSC_TDEC" "BSC tdec address (0x...)"
    fi
    if [ -z "$OPBNB_TESTA" ]; then
        prompt_value "OPBNB_TESTA" "opBNB tokena address (0x...)"
    fi
    if [ -z "$OPBNB_TESTB" ]; then
        prompt_value "OPBNB_TESTB" "opBNB tokenb address (0x...)"
    fi
    if [ -z "$OPBNB_TDEC" ]; then
        prompt_value "OPBNB_TDEC" "opBNB tdec address (0x...)"
    fi

    # Compute EVM bytes32 representations (left-padded address, then hex-encoded without 0x)
    local BSC_TESTA_B32=$(python3 -c "print('$BSC_TESTA'.lower().replace('0x','').zfill(64))")
    local BSC_TESTB_B32=$(python3 -c "print('$BSC_TESTB'.lower().replace('0x','').zfill(64))")
    local BSC_TDEC_B32=$(python3 -c "print('$BSC_TDEC'.lower().replace('0x','').zfill(64))")
    local OPBNB_TESTA_B32=$(python3 -c "print('$OPBNB_TESTA'.lower().replace('0x','').zfill(64))")
    local OPBNB_TESTB_B32=$(python3 -c "print('$OPBNB_TESTB'.lower().replace('0x','').zfill(64))")
    local OPBNB_TDEC_B32=$(python3 -c "print('$OPBNB_TDEC'.lower().replace('0x','').zfill(64))")

    # testa → BSC (18 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_token_destination":{"token":"'"$TERRA_TESTA"'","dest_chain":"'"$BRIDGE_BSC_CHAIN_ID_B64"'","dest_token":"'"$BSC_TESTA_B32"'","dest_decimals":18}}' \
        "testa → BSC (dest dec: 18)"

    # testa → opBNB (18 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_token_destination":{"token":"'"$TERRA_TESTA"'","dest_chain":"'"$BRIDGE_OPBNB_CHAIN_ID_B64"'","dest_token":"'"$OPBNB_TESTA_B32"'","dest_decimals":18}}' \
        "testa → opBNB (dest dec: 18)"

    # testb → BSC (18 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_token_destination":{"token":"'"$TERRA_TESTB"'","dest_chain":"'"$BRIDGE_BSC_CHAIN_ID_B64"'","dest_token":"'"$BSC_TESTB_B32"'","dest_decimals":18}}' \
        "testb → BSC (dest dec: 18)"

    # testb → opBNB (18 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_token_destination":{"token":"'"$TERRA_TESTB"'","dest_chain":"'"$BRIDGE_OPBNB_CHAIN_ID_B64"'","dest_token":"'"$OPBNB_TESTB_B32"'","dest_decimals":18}}' \
        "testb → opBNB (dest dec: 18)"

    # tdec → BSC (18 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_token_destination":{"token":"'"$TERRA_TDEC"'","dest_chain":"'"$BRIDGE_BSC_CHAIN_ID_B64"'","dest_token":"'"$BSC_TDEC_B32"'","dest_decimals":18}}' \
        "tdec → BSC (dest dec: 18)"

    # tdec → opBNB (12 dec)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_token_destination":{"token":"'"$TERRA_TDEC"'","dest_chain":"'"$BRIDGE_OPBNB_CHAIN_ID_B64"'","dest_token":"'"$OPBNB_TDEC_B32"'","dest_decimals":12}}' \
        "tdec → opBNB (dest dec: 12)"
}

# ─── Phase 7: Set incoming token mappings ─────────────────────────────────────
set_incoming_mappings() {
    log_header "Phase 7: Incoming Token Mappings"

    # Compute base64-encoded bytes32 for each Terra CW20 address
    log_step "Computing base64 token identifiers..."

    local TESTA_B64=$(python3 -c "
import bech32, base64
_, data = bech32.bech32_decode('$TERRA_TESTA')
raw = bytes(bech32.convertbits(data, 5, 8, False))
print(base64.b64encode(b'\x00' * (32 - len(raw)) + raw).decode())
")
    local TESTB_B64=$(python3 -c "
import bech32, base64
_, data = bech32.bech32_decode('$TERRA_TESTB')
raw = bytes(bech32.convertbits(data, 5, 8, False))
print(base64.b64encode(b'\x00' * (32 - len(raw)) + raw).decode())
")
    local TDEC_B64=$(python3 -c "
import bech32, base64
_, data = bech32.bech32_decode('$TERRA_TDEC')
raw = bytes(bech32.convertbits(data, 5, 8, False))
print(base64.b64encode(b'\x00' * (32 - len(raw)) + raw).decode())
")

    log_info "testa src_token (b64): $TESTA_B64"
    log_info "testb src_token (b64): $TESTB_B64"
    log_info "tdec  src_token (b64): $TDEC_B64"

    # ─── Incoming from BSC ───

    # testa ← BSC (src dec: 18)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_incoming_token_mapping":{"src_chain":"'"$BRIDGE_BSC_CHAIN_ID_B64"'","src_token":"'"$TESTA_B64"'","local_token":"'"$TERRA_TESTA"'","src_decimals":18}}' \
        "testa ← BSC (src dec: 18)"

    # testb ← BSC (src dec: 18)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_incoming_token_mapping":{"src_chain":"'"$BRIDGE_BSC_CHAIN_ID_B64"'","src_token":"'"$TESTB_B64"'","local_token":"'"$TERRA_TESTB"'","src_decimals":18}}' \
        "testb ← BSC (src dec: 18)"

    # tdec ← BSC (src dec: 18)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_incoming_token_mapping":{"src_chain":"'"$BRIDGE_BSC_CHAIN_ID_B64"'","src_token":"'"$TDEC_B64"'","local_token":"'"$TERRA_TDEC"'","src_decimals":18}}' \
        "tdec ← BSC (src dec: 18)"

    # ─── Incoming from opBNB ───

    # testa ← opBNB (src dec: 18)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_incoming_token_mapping":{"src_chain":"'"$BRIDGE_OPBNB_CHAIN_ID_B64"'","src_token":"'"$TESTA_B64"'","local_token":"'"$TERRA_TESTA"'","src_decimals":18}}' \
        "testa ← opBNB (src dec: 18)"

    # testb ← opBNB (src dec: 18)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_incoming_token_mapping":{"src_chain":"'"$BRIDGE_OPBNB_CHAIN_ID_B64"'","src_token":"'"$TESTB_B64"'","local_token":"'"$TERRA_TESTB"'","src_decimals":18}}' \
        "testb ← opBNB (src dec: 18)"

    # tdec ← opBNB (src dec: 12)
    terra_admin_execute "$TERRA_BRIDGE" \
        '{"set_incoming_token_mapping":{"src_chain":"'"$BRIDGE_OPBNB_CHAIN_ID_B64"'","src_token":"'"$TDEC_B64"'","local_token":"'"$TERRA_TDEC"'","src_decimals":12}}' \
        "tdec ← opBNB (src dec: 12)"
}

# ─── Phase 8: Operator & canceler ────────────────────────────────────────────
register_operator_canceler() {
    log_header "Phase 8: Operator & Canceler Registration"

    echo -e "${YELLOW}Operators were added during bridge instantiation.${NC}" >&2
    echo -e "${YELLOW}Add cancelers now.${NC}" >&2
    echo "" >&2

    prompt_value "TERRA_CANCELER_1" "canceler address (terra1...) or press Enter to skip"

    if [ -n "$TERRA_CANCELER_1" ]; then
        terra_admin_execute "$TERRA_BRIDGE" \
            '{"add_canceler":{"address":"'"$TERRA_CANCELER_1"'"}}' \
            "Add canceler: $TERRA_CANCELER_1"

        prompt_value "TERRA_CANCELER_2" "second canceler address (or press Enter to skip)"
        if [ -n "$TERRA_CANCELER_2" ]; then
            terra_admin_execute "$TERRA_BRIDGE" \
                '{"add_canceler":{"address":"'"$TERRA_CANCELER_2"'"}}' \
                "Add canceler: $TERRA_CANCELER_2"
        fi
    fi
}

# ─── Save addresses ──────────────────────────────────────────────────────────
save_addresses() {
    local ENV_FILE="$PROJECT_ROOT/.env.terra-mainnet"

    cat > "$ENV_FILE" << EOF
# Terra Classic Full Deployment
# Generated: $(date -Iseconds)

# Bridge
TERRA_BRIDGE=$TERRA_BRIDGE
TERRA_BRIDGE_CODE_ID=$BRIDGE_CODE_ID

# CW20 tokens
TERRA_TESTA=$TERRA_TESTA
TERRA_TESTB=$TERRA_TESTB
TERRA_TDEC=$TERRA_TDEC
CW20_CODE_ID=$CW20_CODE_ID

# Faucet
TERRA_FAUCET=${TERRA_FAUCET:-not_deployed}

# Config
TERRA_ADMIN=$TERRA_ADMIN
TERRA_FEE_COLLECTOR=$TERRA_FEE_COLLECTOR
TERRA_KEY_NAME=$TERRA_KEY_NAME
EOF

    log_info "Addresses saved to $ENV_FILE"
}

# ─── Main ─────────────────────────────────────────────────────────────────────
main() {
    echo ""
    echo -e "${RED}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║   CL8Y BRIDGE — FULL TERRA CLASSIC MAINNET DEPLOYMENT      ║${NC}"
    echo -e "${RED}║                                                              ║${NC}"
    echo -e "${RED}║   This will deploy to columbus-5 MAINNET with REAL FUNDS!   ║${NC}"
    echo -e "${RED}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    read -p "Type 'DEPLOY_MAINNET' to confirm: " confirmation
    if [ "$confirmation" != "DEPLOY_MAINNET" ]; then
        log_error "Deployment cancelled"
        exit 1
    fi

    check_prereqs

    deploy_bridge
    deploy_cw20_tokens
    deploy_faucet
    register_chains
    add_tokens
    set_token_destinations
    set_incoming_mappings
    register_operator_canceler
    save_addresses

    log_header "TERRA CLASSIC DEPLOYMENT COMPLETE"

    echo "═══════════════════════════════════════════════════"
    echo "  Bridge:         $TERRA_BRIDGE"
    echo "  testa (18 dec): $TERRA_TESTA"
    echo "  testb (18 dec): $TERRA_TESTB"
    echo "  tdec  (6 dec):  $TERRA_TDEC"
    echo "  Faucet:         ${TERRA_FAUCET:-not deployed}"
    echo "═══════════════════════════════════════════════════"
    echo ""
    echo "Addresses saved to: .env.terra-mainnet"
    echo ""
    echo "Next steps:"
    echo "  1. Set Terra ↔ EVM mappings on EVM side:"
    echo "     ./scripts/deploy-evm-full.sh --terra-mappings"
    echo "  2. Update operator config with new Terra bridge address"
    echo "  3. Update canceler config with new Terra bridge address"
    echo "  4. Update frontend .env with new addresses"
    echo ""
}

main "$@"
