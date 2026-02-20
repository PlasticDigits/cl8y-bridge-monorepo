#!/bin/bash
# Full interactive EVM bridge deployment for BSC + opBNB
#
# Deploys in order:
#   1. Core bridge contracts (ChainRegistry, TokenRegistry, LockUnlock, MintBurn, Bridge)
#   2. AccessManagerEnumerable (via CREATE3)
#   3. Test tokens via factory (tokena, tokenb) + standalone tdec
#   4. Mint permissions (MintBurn + Faucet roles)
#   5. Faucet
#   6. Cross-chain registration
#   7. Token registration (outgoing + incoming mappings)
#   8. Operator/canceler registration
#
# Prerequisites:
#   - Foundry installed (forge, cast)
#   - DEPLOYER_ADDRESS, ADMIN_ADDRESS, OPERATOR_ADDRESS, FEE_RECIPIENT_ADDRESS set
#   - ETHERSCAN_API_KEY for verification
#   - Real BNB on both BSC and opBNB for gas
#   - python3 with bech32 package (pip install bech32) — only needed for Terra cross-chain
#
# Usage:
#   ./scripts/deploy-evm-full.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONTRACTS_DIR="$PROJECT_ROOT/packages/contracts-evm"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step()  { echo -e "${BLUE}[STEP]${NC} $1"; }
log_header() {
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║  $1"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

prompt_continue() {
    echo ""
    read -p "$(echo -e "${YELLOW}Press Enter to continue (or Ctrl+C to abort)...${NC}")" _
    echo ""
}

prompt_address() {
    local varname="$1"
    local description="$2"
    read -p "$(echo -e "${YELLOW}Enter $description: ${NC}")" value
    eval "export $varname=\"$value\""
}

# ─── Network configs ─────────────────────────────────────────────────────────
BSC_RPC="https://bsc-dataseed1.binance.org"
BSC_CHAIN_ID_NUMERIC=56
BSC_WETH="0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"

OPBNB_RPC="https://opbnb-mainnet-rpc.bnbchain.org"
OPBNB_CHAIN_ID_NUMERIC=204
OPBNB_WETH="0x4200000000000000000000000000000000000006"

# Bridge bytes4 chain IDs
BRIDGE_BSC_CHAIN_ID="0x00000038"
BRIDGE_OPBNB_CHAIN_ID="0x000000cc"
BRIDGE_TERRA_CHAIN_ID="0x00000001"

# Hardcoded factory addresses (NOT redeployed)
BSC_FACTORY="0xD9731AcFebD5B9C9b62943D1fE97EeFAFb0F150F"
OPBNB_FACTORY="0xFDF9555c8168EfEbF9d6130E248fCc7Ba0D3bA8b"

# mint(address,uint256) and burn(address,uint256) selectors
MINT_SELECTOR="0x40c10f19"
BURN_SELECTOR="0x9dc29fac"

# ─── Prereq checks ───────────────────────────────────────────────────────────
check_prereqs() {
    log_step "Checking prerequisites..."

    for cmd in forge cast jq; do
        if ! command -v $cmd &>/dev/null; then
            log_error "$cmd is required but not installed"
            exit 1
        fi
    done

    # Prompt for any missing addresses interactively
    if [ -z "$DEPLOYER_ADDRESS" ]; then
        prompt_address "DEPLOYER_ADDRESS" "deployer wallet address (0x...)"
    fi
    if [ -z "$ADMIN_ADDRESS" ]; then
        prompt_address "ADMIN_ADDRESS" "admin/owner address (0x..., multi-sig recommended)"
    fi
    if [ -z "$OPERATOR_ADDRESS" ]; then
        prompt_address "OPERATOR_ADDRESS" "bridge operator address (0x...)"
    fi
    if [ -z "$FEE_RECIPIENT_ADDRESS" ]; then
        prompt_address "FEE_RECIPIENT_ADDRESS" "fee recipient address (0x...)"
    fi
    if [ -z "$ETHERSCAN_API_KEY" ]; then
        echo ""
        read -p "$(echo -e "${YELLOW}Enter Etherscan V2 API key (or press Enter to skip verification): ${NC}")" ETHERSCAN_API_KEY
        export ETHERSCAN_API_KEY
        if [ -z "$ETHERSCAN_API_KEY" ]; then
            log_warn "No API key — contract verification will be skipped"
        fi
    fi

    log_info "Deployer: $DEPLOYER_ADDRESS"
    log_info "Admin:    $ADMIN_ADDRESS"
    log_info "Operator: $OPERATOR_ADDRESS"
    log_info "Fee:      $FEE_RECIPIENT_ADDRESS"

    for label_rpc in "BSC:$BSC_RPC" "opBNB:$OPBNB_RPC"; do
        IFS=: read -r label rpc <<< "$label_rpc"
        BAL=$(cast balance "$DEPLOYER_ADDRESS" --rpc-url "$rpc" 2>/dev/null || echo "0")
        BAL_ETH=$(cast from-wei "$BAL" 2>/dev/null || echo "0")
        log_info "$label deployer balance: $BAL_ETH BNB"
    done

    log_info "Prerequisites OK"
}

# ─── Phase 1: Deploy core bridge ─────────────────────────────────────────────
deploy_core() {
    local chain_name="$1" rpc="$2" chain_id="$3" weth="$4" chain_identifier="$5"

    log_header "Phase 1: Deploy Core Bridge — $chain_name"

    cd "$CONTRACTS_DIR"
    forge build

    export WETH_ADDRESS="$weth"
    export CHAIN_IDENTIFIER="$chain_identifier"
    export THIS_CHAIN_ID="$chain_id"
    export ADMIN_ADDRESS
    export OPERATOR_ADDRESS
    export FEE_RECIPIENT_ADDRESS

    log_info "WETH_ADDRESS=$WETH_ADDRESS"
    log_info "CHAIN_IDENTIFIER=$CHAIN_IDENTIFIER"
    log_info "THIS_CHAIN_ID=$THIS_CHAIN_ID"

    forge script script/Deploy.s.sol:Deploy \
        --rpc-url "$rpc" \
        --sender "$DEPLOYER_ADDRESS" \
        -i 1 \
        --broadcast \
        --verify \
        --etherscan-api-key "${ETHERSCAN_API_KEY:-}" \
        --slow \
        -vvv

    local BROADCAST_FILE="$CONTRACTS_DIR/broadcast/Deploy.s.sol/$chain_id/run-latest.json"
    if [ ! -f "$BROADCAST_FILE" ]; then
        log_error "Broadcast file not found: $BROADCAST_FILE"
        exit 1
    fi

    CREATES=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[].contractAddress' "$BROADCAST_FILE")

    local CR_IMPL=$(echo "$CREATES" | sed -n '1p')
    local CR_PROXY=$(echo "$CREATES" | sed -n '2p')
    local TR_IMPL=$(echo "$CREATES" | sed -n '3p')
    local TR_PROXY=$(echo "$CREATES" | sed -n '4p')
    local LU_IMPL=$(echo "$CREATES" | sed -n '5p')
    local LU_PROXY=$(echo "$CREATES" | sed -n '6p')
    local MB_IMPL=$(echo "$CREATES" | sed -n '7p')
    local MB_PROXY=$(echo "$CREATES" | sed -n '8p')
    local BR_IMPL=$(echo "$CREATES" | sed -n '9p')
    local BR_PROXY=$(echo "$CREATES" | sed -n '10p')

    echo ""
    echo "═══════════════════════════════════════"
    echo "  $chain_name Core Contracts Deployed"
    echo "═══════════════════════════════════════"
    echo "  ChainRegistry:  $CR_PROXY"
    echo "  TokenRegistry:  $TR_PROXY"
    echo "  LockUnlock:     $LU_PROXY"
    echo "  MintBurn:       $MB_PROXY"
    echo "  Bridge:         $BR_PROXY"
    echo "═══════════════════════════════════════"

    # Export for later phases
    eval "export ${chain_identifier}_CHAIN_REGISTRY=$CR_PROXY"
    eval "export ${chain_identifier}_TOKEN_REGISTRY=$TR_PROXY"
    eval "export ${chain_identifier}_LOCK_UNLOCK=$LU_PROXY"
    eval "export ${chain_identifier}_MINT_BURN=$MB_PROXY"
    eval "export ${chain_identifier}_BRIDGE=$BR_PROXY"
}

# ─── Phase 2: Deploy AccessManagerEnumerable ──────────────────────────────────
deploy_access_manager() {
    local chain_name="$1" rpc="$2" chain_id="$3" chain_identifier="$4"

    log_header "Phase 2: Deploy AccessManagerEnumerable — $chain_name"

    cd "$CONTRACTS_DIR"
    export ACCESS_MANAGER_ADMIN="$ADMIN_ADDRESS"

    forge script script/AccessManagerEnumerable.s.sol:AccessManagerScript \
        --rpc-url "$rpc" \
        --sender "$DEPLOYER_ADDRESS" \
        -i 1 \
        --broadcast \
        --verify \
        --etherscan-api-key "${ETHERSCAN_API_KEY:-}" \
        --slow \
        -vvv

    local BROADCAST_FILE="$CONTRACTS_DIR/broadcast/AccessManagerEnumerable.s.sol/$chain_id/run-latest.json"
    CREATES=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[].contractAddress' "$BROADCAST_FILE")
    local AM_ADDR=$(echo "$CREATES" | tail -1)

    log_info "$chain_name AccessManagerEnumerable: $AM_ADDR"
    eval "export ${chain_identifier}_ACCESS_MANAGER=$AM_ADDR"
}

# ─── Phase 3: Deploy test tokens ─────────────────────────────────────────────
deploy_tokens() {
    local chain_name="$1" rpc="$2" chain_id="$3" chain_identifier="$4" factory="$5" tdec_decimals="$6"

    log_header "Phase 3: Deploy Test Tokens — $chain_name"

    local am_var="${chain_identifier}_ACCESS_MANAGER"
    local access_manager="${!am_var}"

    # tokena and tokenb via factory (18 decimals)
    # Names use "V2" suffix to avoid CREATE2 collision with previous deployment
    log_step "Creating tokena via factory..."
    cast send --interactive --rpc-url "$rpc" \
        "$factory" "createToken(string,string,string)" "Token A V2" "tokena" ""

    log_step "Creating tokenb via factory..."
    cast send --interactive --rpc-url "$rpc" \
        "$factory" "createToken(string,string,string)" "Token B V2" "tokenb" ""

    # Retrieve factory-created token addresses
    log_step "Retrieving factory token addresses..."
    local FACTORY_TOKENS=$(cast call "$factory" "getAllTokens()(address[])" --rpc-url "$rpc")
    log_info "Factory tokens: $FACTORY_TOKENS"

    echo ""
    echo -e "${YELLOW}Record the token addresses from the factory output above.${NC}"
    prompt_address "${chain_identifier}_TESTA" "$chain_name tokena address"
    prompt_address "${chain_identifier}_TESTB" "$chain_name tokenb address"

    # tdec: standalone custom-decimal token
    log_step "Deploying tdec ($tdec_decimals decimals) via standalone contract..."
    export ACCESS_MANAGER_ADDRESS="$access_manager"
    export TOKEN_NAME="Test Dec cl8y.com/bridge"
    export TOKEN_SYMBOL="tdec-cb"
    export TOKEN_DECIMALS="$tdec_decimals"

    forge script script/DeployCustomDecimalToken.s.sol:DeployCustomDecimalToken \
        --rpc-url "$rpc" \
        --sender "$DEPLOYER_ADDRESS" \
        -i 1 \
        --broadcast \
        --verify \
        --etherscan-api-key "${ETHERSCAN_API_KEY:-}" \
        --slow \
        -vvv

    local BROADCAST_FILE="$CONTRACTS_DIR/broadcast/DeployCustomDecimalToken.s.sol/$chain_id/run-latest.json"
    local TDEC_ADDR=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[0].contractAddress' "$BROADCAST_FILE")
    log_info "$chain_name tdec ($tdec_decimals dec): $TDEC_ADDR"
    eval "export ${chain_identifier}_TDEC=$TDEC_ADDR"
}

# ─── Phase 4: Mint permissions ────────────────────────────────────────────────
setup_mint_permissions() {
    local chain_name="$1" rpc="$2" chain_identifier="$3"

    log_header "Phase 4: Mint Permissions — $chain_name"

    local am_var="${chain_identifier}_ACCESS_MANAGER"
    local mb_var="${chain_identifier}_MINT_BURN"
    local access_manager="${!am_var}"
    local mint_burn="${!mb_var}"

    local testa_var="${chain_identifier}_TESTA"
    local testb_var="${chain_identifier}_TESTB"
    local tdec_var="${chain_identifier}_TDEC"

    # Grant MintBurn contract role 1 (MINTER_ROLE)
    log_step "Granting MintBurn ($mint_burn) role 1 on AccessManager..."
    cast send --interactive --rpc-url "$rpc" \
        "$access_manager" "grantRole(uint64,address,uint32)" 1 "$mint_burn" 0

    # Map role 1 to mint/burn selectors on each token
    for token_var in "$testa_var" "$testb_var" "$tdec_var"; do
        local token="${!token_var}"
        log_step "Setting mint/burn role on token $token..."
        cast send --interactive --rpc-url "$rpc" \
            "$access_manager" "setTargetFunctionRole(address,bytes4[],uint64)" \
            "$token" "[$MINT_SELECTOR,$BURN_SELECTOR]" 1
    done
}

# ─── Phase 5: Deploy Faucet ──────────────────────────────────────────────────
deploy_faucet() {
    local chain_name="$1" rpc="$2" chain_id="$3" chain_identifier="$4"

    log_header "Phase 5: Deploy Faucet — $chain_name"

    cd "$CONTRACTS_DIR"

    forge script script/DeployFaucet.s.sol:DeployFaucet \
        --rpc-url "$rpc" \
        --sender "$DEPLOYER_ADDRESS" \
        -i 1 \
        --broadcast \
        --verify \
        --etherscan-api-key "${ETHERSCAN_API_KEY:-}" \
        --slow \
        -vvv

    local BROADCAST_FILE="$CONTRACTS_DIR/broadcast/DeployFaucet.s.sol/$chain_id/run-latest.json"
    local FAUCET_ADDR=$(jq -r '[.transactions[] | select(.transactionType == "CREATE")] | .[0].contractAddress' "$BROADCAST_FILE")
    log_info "$chain_name Faucet: $FAUCET_ADDR"
    eval "export ${chain_identifier}_FAUCET=$FAUCET_ADDR"

    # Grant faucet role 1 (MINTER_ROLE) so it can mint test tokens
    local am_var="${chain_identifier}_ACCESS_MANAGER"
    local access_manager="${!am_var}"
    log_step "Granting Faucet ($FAUCET_ADDR) role 1 on AccessManager..."
    cast send --interactive --rpc-url "$rpc" \
        "$access_manager" "grantRole(uint64,address,uint32)" 1 "$FAUCET_ADDR" 0
}

# ─── Phase 6: Cross-chain registration ───────────────────────────────────────
register_cross_chains() {
    log_header "Phase 6: Cross-Chain Registration"

    # BSC: register Terra + opBNB
    log_step "BSC: registering Terra Classic..."
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_CHAIN_REGISTRY" "registerChain(string,bytes4)" \
        "terraclassic_columbus-5" "$BRIDGE_TERRA_CHAIN_ID"

    log_step "BSC: registering opBNB..."
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_CHAIN_REGISTRY" "registerChain(string,bytes4)" \
        "evm_204" "$BRIDGE_OPBNB_CHAIN_ID"

    # opBNB: register Terra + BSC
    log_step "opBNB: registering Terra Classic..."
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_CHAIN_REGISTRY" "registerChain(string,bytes4)" \
        "terraclassic_columbus-5" "$BRIDGE_TERRA_CHAIN_ID"

    log_step "opBNB: registering BSC..."
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_CHAIN_REGISTRY" "registerChain(string,bytes4)" \
        "evm_56" "$BRIDGE_BSC_CHAIN_ID"
}

# ─── Phase 7: Token registration ─────────────────────────────────────────────
register_tokens_on_chain() {
    local chain_name="$1" rpc="$2" chain_identifier="$3"
    local other_evm_name="$4" other_evm_chain_id="$5"

    local tr_var="${chain_identifier}_TOKEN_REGISTRY"
    local token_registry="${!tr_var}"

    local testa_var="${chain_identifier}_TESTA"
    local testb_var="${chain_identifier}_TESTB"
    local tdec_var="${chain_identifier}_TDEC"
    local testa="${!testa_var}"
    local testb="${!testb_var}"
    local tdec="${!tdec_var}"

    # Determine tdec local decimals
    local tdec_local_dec=18
    if [ "$chain_identifier" = "opBNB" ]; then
        tdec_local_dec=12
    fi

    log_step "$chain_name: Registering tokena..."
    cast send --interactive --rpc-url "$rpc" \
        "$token_registry" "registerToken(address,uint8)" "$testa" 1

    log_step "$chain_name: Registering tokenb..."
    cast send --interactive --rpc-url "$rpc" \
        "$token_registry" "registerToken(address,uint8)" "$testb" 1

    log_step "$chain_name: Registering tdec..."
    cast send --interactive --rpc-url "$rpc" \
        "$token_registry" "registerToken(address,uint8)" "$tdec" 1
}

setup_token_destinations() {
    log_header "Phase 7: Token Destination Mappings"

    # We need Terra token addresses for EVM→Terra mappings.
    # These won't exist until Terra deployment, so we skip Terra dest mappings for now
    # and handle EVM↔EVM mappings.

    echo -e "${YELLOW}For EVM↔EVM cross-chain mappings, we need token addresses from both chains.${NC}"
    echo ""

    # BSC→opBNB and opBNB→BSC for all tokens
    local BSC_TESTA_B32=$(cast abi-encode "f(address)" "$BSC_TESTA")
    local BSC_TESTB_B32=$(cast abi-encode "f(address)" "$BSC_TESTB")
    local BSC_TDEC_B32=$(cast abi-encode "f(address)" "$BSC_TDEC")
    local OPBNB_TESTA_B32=$(cast abi-encode "f(address)" "$opBNB_TESTA")
    local OPBNB_TESTB_B32=$(cast abi-encode "f(address)" "$opBNB_TESTB")
    local OPBNB_TDEC_B32=$(cast abi-encode "f(address)" "$opBNB_TDEC")

    # ─── BSC TokenRegistry: outgoing ───
    log_step "BSC: tokena → opBNB (dest dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$BSC_TESTA" "$BRIDGE_OPBNB_CHAIN_ID" "$OPBNB_TESTA_B32" 18

    log_step "BSC: tokenb → opBNB (dest dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$BSC_TESTB" "$BRIDGE_OPBNB_CHAIN_ID" "$OPBNB_TESTB_B32" 18

    log_step "BSC: tdec → opBNB (dest dec: 12)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$BSC_TDEC" "$BRIDGE_OPBNB_CHAIN_ID" "$OPBNB_TDEC_B32" 12

    # ─── BSC TokenRegistry: incoming from opBNB ───
    log_step "BSC: tokena ← opBNB (src dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_OPBNB_CHAIN_ID" "$BSC_TESTA" 18

    log_step "BSC: tokenb ← opBNB (src dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_OPBNB_CHAIN_ID" "$BSC_TESTB" 18

    log_step "BSC: tdec ← opBNB (src dec: 12)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_OPBNB_CHAIN_ID" "$BSC_TDEC" 12

    # ─── opBNB TokenRegistry: outgoing ───
    log_step "opBNB: tokena → BSC (dest dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$opBNB_TESTA" "$BRIDGE_BSC_CHAIN_ID" "$BSC_TESTA_B32" 18

    log_step "opBNB: tokenb → BSC (dest dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$opBNB_TESTB" "$BRIDGE_BSC_CHAIN_ID" "$BSC_TESTB_B32" 18

    log_step "opBNB: tdec → BSC (dest dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$opBNB_TDEC" "$BRIDGE_BSC_CHAIN_ID" "$BSC_TDEC_B32" 18

    # ─── opBNB TokenRegistry: incoming from BSC ───
    log_step "opBNB: tokena ← BSC (src dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_BSC_CHAIN_ID" "$opBNB_TESTA" 18

    log_step "opBNB: tokenb ← BSC (src dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_BSC_CHAIN_ID" "$opBNB_TESTB" 18

    log_step "opBNB: tdec ← BSC (src dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_BSC_CHAIN_ID" "$opBNB_TDEC" 18

    # ─── Terra mappings (deferred) ───
    echo ""
    log_warn "Terra ↔ EVM token mappings are NOT configured here."
    log_warn "Run deploy-terra-full.sh first to get Terra CW20 addresses,"
    log_warn "then run this script's --terra-mappings phase."
}

# ─── Phase 7b: Terra ↔ EVM token mappings ────────────────────────────────────
setup_terra_evm_mappings() {
    log_header "Phase 7b: Terra ↔ EVM Token Mappings"

    echo -e "${YELLOW}This phase requires Terra CW20 token addresses from deploy-terra-full.sh${NC}"
    echo ""

    prompt_address "TERRA_TESTA_ADDR" "Terra testa CW20 address (terra1...)"
    prompt_address "TERRA_TESTB_ADDR" "Terra testb CW20 address (terra1...)"
    prompt_address "TERRA_TDEC_ADDR"  "Terra tdec CW20 address (terra1...)"

    # Compute bytes32 representations via bech32 decode
    log_step "Computing bytes32 for Terra CW20 addresses..."
    local TERRA_TESTA_B32="0x$(python3 -c "import bech32; _, data = bech32.bech32_decode('$TERRA_TESTA_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print('00' * (32 - len(raw)) + raw.hex())")"
    local TERRA_TESTB_B32="0x$(python3 -c "import bech32; _, data = bech32.bech32_decode('$TERRA_TESTB_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print('00' * (32 - len(raw)) + raw.hex())")"
    local TERRA_TDEC_B32="0x$(python3 -c "import bech32; _, data = bech32.bech32_decode('$TERRA_TDEC_ADDR'); raw = bytes(bech32.convertbits(data, 5, 8, False)); print('00' * (32 - len(raw)) + raw.hex())")"

    log_info "Terra testa bytes32: $TERRA_TESTA_B32"
    log_info "Terra testb bytes32: $TERRA_TESTB_B32"
    log_info "Terra tdec bytes32:  $TERRA_TDEC_B32"

    # ─── BSC → Terra destinations ───
    log_step "BSC: tokena → Terra (dest dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$BSC_TESTA" "$BRIDGE_TERRA_CHAIN_ID" "$TERRA_TESTA_B32" 18

    log_step "BSC: tokenb → Terra (dest dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$BSC_TESTB" "$BRIDGE_TERRA_CHAIN_ID" "$TERRA_TESTB_B32" 18

    log_step "BSC: tdec → Terra (dest dec: 6)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$BSC_TDEC" "$BRIDGE_TERRA_CHAIN_ID" "$TERRA_TDEC_B32" 6

    # ─── BSC ← Terra incoming ───
    log_step "BSC: tokena ← Terra (src dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_TERRA_CHAIN_ID" "$BSC_TESTA" 18

    log_step "BSC: tokenb ← Terra (src dec: 18)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_TERRA_CHAIN_ID" "$BSC_TESTB" 18

    log_step "BSC: tdec ← Terra (src dec: 6)"
    cast send --interactive --rpc-url "$BSC_RPC" \
        "$BSC_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_TERRA_CHAIN_ID" "$BSC_TDEC" 6

    # ─── opBNB → Terra destinations ───
    log_step "opBNB: tokena → Terra (dest dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$opBNB_TESTA" "$BRIDGE_TERRA_CHAIN_ID" "$TERRA_TESTA_B32" 18

    log_step "opBNB: tokenb → Terra (dest dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$opBNB_TESTB" "$BRIDGE_TERRA_CHAIN_ID" "$TERRA_TESTB_B32" 18

    log_step "opBNB: tdec → Terra (dest dec: 6)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)" \
        "$opBNB_TDEC" "$BRIDGE_TERRA_CHAIN_ID" "$TERRA_TDEC_B32" 6

    # ─── opBNB ← Terra incoming ───
    log_step "opBNB: tokena ← Terra (src dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_TERRA_CHAIN_ID" "$opBNB_TESTA" 18

    log_step "opBNB: tokenb ← Terra (src dec: 18)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_TERRA_CHAIN_ID" "$opBNB_TESTB" 18

    log_step "opBNB: tdec ← Terra (src dec: 6)"
    cast send --interactive --rpc-url "$OPBNB_RPC" \
        "$opBNB_TOKEN_REGISTRY" "setIncomingTokenMapping(bytes4,address,uint8)" \
        "$BRIDGE_TERRA_CHAIN_ID" "$opBNB_TDEC" 6

    log_info "All Terra ↔ EVM token mappings configured."
}

# ─── Phase 8: Operator/canceler registration ─────────────────────────────────
register_operators_cancelers() {
    log_header "Phase 8: Operator & Canceler Registration"

    echo -e "${YELLOW}The operator was already added during core deployment.${NC}"
    echo -e "${YELLOW}Add additional operators/cancelers now if needed.${NC}"
    echo ""

    read -p "$(echo -e "${YELLOW}Enter canceler address (or press Enter to skip): ${NC}")" CANCELER_ADDRESS

    if [ -n "$CANCELER_ADDRESS" ]; then
        log_step "BSC: Adding canceler $CANCELER_ADDRESS..."
        cast send --interactive --rpc-url "$BSC_RPC" \
            "$BSC_BRIDGE" "addCanceler(address)" "$CANCELER_ADDRESS"

        log_step "opBNB: Adding canceler $CANCELER_ADDRESS..."
        cast send --interactive --rpc-url "$OPBNB_RPC" \
            "$opBNB_BRIDGE" "addCanceler(address)" "$CANCELER_ADDRESS"

        read -p "$(echo -e "${YELLOW}Enter another canceler address (or press Enter to skip): ${NC}")" CANCELER_2
        if [ -n "$CANCELER_2" ]; then
            cast send --interactive --rpc-url "$BSC_RPC" \
                "$BSC_BRIDGE" "addCanceler(address)" "$CANCELER_2"
            cast send --interactive --rpc-url "$OPBNB_RPC" \
                "$opBNB_BRIDGE" "addCanceler(address)" "$CANCELER_2"
        fi
    fi
}

# ─── Save addresses ──────────────────────────────────────────────────────────
save_addresses() {
    local chain_name="$1" chain_identifier="$2" filename="$3"

    local cr_var="${chain_identifier}_CHAIN_REGISTRY"
    local tr_var="${chain_identifier}_TOKEN_REGISTRY"
    local lu_var="${chain_identifier}_LOCK_UNLOCK"
    local mb_var="${chain_identifier}_MINT_BURN"
    local br_var="${chain_identifier}_BRIDGE"
    local am_var="${chain_identifier}_ACCESS_MANAGER"
    local fc_var="${chain_identifier}_FAUCET"
    local ta_var="${chain_identifier}_TESTA"
    local tb_var="${chain_identifier}_TESTB"
    local td_var="${chain_identifier}_TDEC"

    cat > "$PROJECT_ROOT/$filename" << EOF
# $chain_name Full Deployment
# Generated: $(date -Iseconds)

# Core contracts (proxies)
${chain_identifier}_CHAIN_REGISTRY=${!cr_var}
${chain_identifier}_TOKEN_REGISTRY=${!tr_var}
${chain_identifier}_LOCK_UNLOCK=${!lu_var}
${chain_identifier}_MINT_BURN=${!mb_var}
${chain_identifier}_BRIDGE=${!br_var}

# Access control
${chain_identifier}_ACCESS_MANAGER=${!am_var}

# Token addresses
${chain_identifier}_TESTA=${!ta_var}
${chain_identifier}_TESTB=${!tb_var}
${chain_identifier}_TDEC=${!td_var}

# Faucet
${chain_identifier}_FAUCET=${!fc_var}
EOF

    log_info "Addresses saved to $filename"
}

# ─── Main ─────────────────────────────────────────────────────────────────────
main() {
    echo ""
    echo -e "${RED}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║      CL8Y BRIDGE — FULL EVM MAINNET DEPLOYMENT             ║${NC}"
    echo -e "${RED}║                                                              ║${NC}"
    echo -e "${RED}║  This will deploy to BSC + opBNB MAINNET with REAL FUNDS!   ║${NC}"
    echo -e "${RED}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    if [ "$1" = "--terra-mappings" ]; then
        log_info "Running Terra ↔ EVM mapping phase only"
        echo ""
        echo -e "${YELLOW}Load your existing EVM deployment addresses first:${NC}"
        echo "  source .env.bsc-mainnet"
        echo "  source .env.opbnb-mainnet"
        echo ""
        prompt_continue
        setup_terra_evm_mappings
        exit 0
    fi

    read -p "Type 'DEPLOY TO MAINNET' to confirm: " confirmation
    if [ "$confirmation" != "DEPLOY TO MAINNET" ]; then
        log_error "Deployment cancelled"
        exit 1
    fi

    check_prereqs

    # ═══════════════════════════════════════════════════════════════════════
    # LOCKSTEP DEPLOYMENT: each phase runs on BSC then opBNB before moving
    # to the next phase. This ensures the deployer nonce stays in sync so
    # contract addresses match across chains (deterministic CREATE).
    #
    # SKIP LOGIC: Set env vars to skip already-completed phases:
    #   Phase 1: BSC_BRIDGE, BSC_CHAIN_REGISTRY, BSC_TOKEN_REGISTRY,
    #            BSC_LOCK_UNLOCK, BSC_MINT_BURN (+ opBNB_ equivalents)
    #   Phase 2: BSC_ACCESS_MANAGER, opBNB_ACCESS_MANAGER
    #   Phase 3: BSC_TESTA, BSC_TESTB, BSC_TDEC (+ opBNB_ equivalents)
    #   Phase 4: (no skip — idempotent, runs if tokens exist)
    #   Phase 5: BSC_FAUCET, opBNB_FAUCET
    # ═══════════════════════════════════════════════════════════════════════

    # ─── Phase 1: Core bridge on both chains ───
    if [ -n "$BSC_BRIDGE" ] && [ -n "$BSC_CHAIN_REGISTRY" ] && [ -n "$BSC_TOKEN_REGISTRY" ] \
       && [ -n "$BSC_LOCK_UNLOCK" ] && [ -n "$BSC_MINT_BURN" ] \
       && [ -n "$opBNB_BRIDGE" ] && [ -n "$opBNB_CHAIN_REGISTRY" ] && [ -n "$opBNB_TOKEN_REGISTRY" ] \
       && [ -n "$opBNB_LOCK_UNLOCK" ] && [ -n "$opBNB_MINT_BURN" ]; then
        log_info "Phase 1: SKIPPED — core contracts already set via env vars"
        log_info "  BSC Bridge:   $BSC_BRIDGE"
        log_info "  opBNB Bridge: $opBNB_BRIDGE"
    else
        deploy_core "BSC" "$BSC_RPC" "$BSC_CHAIN_ID_NUMERIC" "$BSC_WETH" "BSC"
        log_info "Now deploying the same on opBNB to keep nonces in sync..."
        prompt_continue
        deploy_core "opBNB" "$OPBNB_RPC" "$OPBNB_CHAIN_ID_NUMERIC" "$OPBNB_WETH" "opBNB"
    fi

    # ─── Phase 2: AccessManager on both chains ───
    if [ -n "$BSC_ACCESS_MANAGER" ] && [ -n "$opBNB_ACCESS_MANAGER" ]; then
        log_info "Phase 2: SKIPPED — AccessManager already set via env vars"
        log_info "  BSC:   $BSC_ACCESS_MANAGER"
        log_info "  opBNB: $opBNB_ACCESS_MANAGER"
    else
        deploy_access_manager "BSC" "$BSC_RPC" "$BSC_CHAIN_ID_NUMERIC" "BSC"
        deploy_access_manager "opBNB" "$OPBNB_RPC" "$OPBNB_CHAIN_ID_NUMERIC" "opBNB"
    fi

    # ─── Phase 3: Tokens on both chains ───
    if [ -n "$BSC_TESTA" ] && [ -n "$BSC_TESTB" ] && [ -n "$BSC_TDEC" ] \
       && [ -n "$opBNB_TESTA" ] && [ -n "$opBNB_TESTB" ] && [ -n "$opBNB_TDEC" ]; then
        log_info "Phase 3: SKIPPED — tokens already set via env vars"
        log_info "  BSC testa:  $BSC_TESTA"
        log_info "  BSC testb:  $BSC_TESTB"
        log_info "  BSC tdec:   $BSC_TDEC"
        log_info "  opBNB testa: $opBNB_TESTA"
        log_info "  opBNB testb: $opBNB_TESTB"
        log_info "  opBNB tdec:  $opBNB_TDEC"
    else
        deploy_tokens "BSC" "$BSC_RPC" "$BSC_CHAIN_ID_NUMERIC" "BSC" "$BSC_FACTORY" 18
        deploy_tokens "opBNB" "$OPBNB_RPC" "$OPBNB_CHAIN_ID_NUMERIC" "opBNB" "$OPBNB_FACTORY" 12
    fi

    # ─── Phase 4: Mint permissions on both chains ───
    setup_mint_permissions "BSC" "$BSC_RPC" "BSC"
    setup_mint_permissions "opBNB" "$OPBNB_RPC" "opBNB"

    # ─── Phase 5: Faucet on both chains ───
    if [ -n "$BSC_FAUCET" ] && [ -n "$opBNB_FAUCET" ]; then
        log_info "Phase 5: SKIPPED — faucets already set via env vars"
        log_info "  BSC:   $BSC_FAUCET"
        log_info "  opBNB: $opBNB_FAUCET"
    else
        deploy_faucet "BSC" "$BSC_RPC" "$BSC_CHAIN_ID_NUMERIC" "BSC"
        deploy_faucet "opBNB" "$OPBNB_RPC" "$OPBNB_CHAIN_ID_NUMERIC" "opBNB"
    fi

    # Save addresses after all contracts deployed
    save_addresses "BSC Mainnet" "BSC" ".env.bsc-mainnet"
    save_addresses "opBNB Mainnet" "opBNB" ".env.opbnb-mainnet"

    log_header "ALL CONTRACTS DEPLOYED ON BOTH CHAINS"
    echo -e "${GREEN}Review addresses in .env.bsc-mainnet and .env.opbnb-mainnet${NC}"
    prompt_continue

    # ─── Phase 6-8: Cross-chain config (nonce order doesn't matter) ───
    register_tokens_on_chain "BSC" "$BSC_RPC" "BSC" "opBNB" "$BRIDGE_OPBNB_CHAIN_ID"
    register_tokens_on_chain "opBNB" "$OPBNB_RPC" "opBNB" "BSC" "$BRIDGE_BSC_CHAIN_ID"
    register_cross_chains
    setup_token_destinations
    register_operators_cancelers

    # ═══════════════════ Summary ═══════════════════
    log_header "FULL EVM DEPLOYMENT COMPLETE"

    echo "BSC addresses:  .env.bsc-mainnet"
    echo "opBNB addresses: .env.opbnb-mainnet"
    echo ""
    echo "Next steps:"
    echo "  1. Deploy Terra contracts: ./scripts/deploy-terra-full.sh"
    echo "  2. Set Terra ↔ EVM mappings: ./scripts/deploy-evm-full.sh --terra-mappings"
    echo "  3. Configure operator (.env) with new bridge addresses"
    echo "  4. Configure canceler(s) with new bridge addresses"
    echo ""
}

main "$@"
