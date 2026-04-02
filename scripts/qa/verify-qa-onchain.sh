#!/usr/bin/env bash
# Spot-check QA deploy addresses + token matrix on Anvil, Anvil1, LocalTerra, Solana.
# Sources .deploy/local.env and scripts/qa/qa-host.env (same as start-qa).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -f "$REPO_ROOT/.deploy/local.env" ]; then
  echo "[verify-qa-onchain] Missing .deploy/local.env — run make deploy or make start-qa." >&2
  exit 1
fi

set -a
# shellcheck source=/dev/null
source "$REPO_ROOT/.deploy/local.env"
# shellcheck source=/dev/null
source "$REPO_ROOT/scripts/qa/qa-host.env"
set +a

EVM_RPC_URL="${EVM_RPC_URL:-http://127.0.0.1:8545}"
EVM1_RPC_URL="${EVM1_RPC_URL:-http://127.0.0.1:8546}"
TERRA_LCD_URL="${TERRA_LCD_URL:-http://127.0.0.1:1318}"
SOLANA_RPC_URL="${SOLANA_RPC_URL:-http://127.0.0.1:8899}"

fail=0
check_evm_code() {
  local name="$1" addr="$2" rpc="$3"
  if [ -z "$addr" ]; then
    echo "  $name: (not set)"
    return 0
  fi
  local sz
  sz=$(cast codesize "$addr" --rpc-url "$rpc" 2>/dev/null || echo "0")
  if [ "${sz:-0}" -gt 0 ] 2>/dev/null; then
    echo "  $name: $addr OK (codesize $sz)"
  else
    echo "  $name: $addr FAIL (no code at $rpc)" >&2
    fail=1
  fi
}

# Matches packages/frontend/src/test/e2e-infra/deploy-terra.ts PLACEHOLDER_PREFIX
TERRA_PLACEHOLDER_PREFIX="terra1placeholder_"

check_terra_cw() {
  local name="$1" addr="$2"
  if [ -z "$addr" ]; then
    echo "  $name: (not set)"
    return 0
  fi
  local label
  label=$(curl -sf "${TERRA_LCD_URL}/cosmwasm/wasm/v1/contract/${addr}" 2>/dev/null | jq -r '.contract_info.label // empty' 2>/dev/null || echo "")
  if [ -n "$label" ]; then
    echo "  $name: $addr OK ($label)"
  else
    echo "  $name: $addr FAIL (no contract at LCD)" >&2
    fail=1
  fi
}

# Terra CW20 rows that may be skipped when instantiate failed (placeholder addr in .deploy/local.env)
check_terra_cw_optional_matrix() {
  local name="$1" addr="$2"
  if [ -z "$addr" ]; then
    echo "  $name: (not set) SKIP"
    return 0
  fi
  if [[ "$addr" == ${TERRA_PLACEHOLDER_PREFIX}* ]]; then
    echo "  $name: $addr SKIP (placeholder — CW20 not deployed on LocalTerra)" >&2
    if [ "${VERIFY_QA_STRICT_T2022:-}" = "1" ]; then
      echo "  $name: strict mode requires real contract" >&2
      fail=1
    fi
    return 0
  fi
  check_terra_cw "$name" "$addr"
}

check_solana_prog() {
  local addr="$1"
  if [ -z "$addr" ]; then
    echo "  Solana program: (not set)"
    return 0
  fi
  local ex
  ex=$(curl -sf -X POST -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"${addr}\",{\"encoding\":\"base64\"}]}" \
    "$SOLANA_RPC_URL" | jq -r '.result.value.executable // empty' 2>/dev/null || echo "")
  if [ "$ex" = "true" ]; then
    echo "  Solana program: $addr OK"
  else
    echo "  Solana program: $addr FAIL" >&2
    fail=1
  fi
}

check_solana_mint() {
  local name="$1" addr="$2"
  if [ -z "$addr" ]; then
    echo "  $name: (not set)"
    return 0
  fi
  local val
  val=$(curl -sf -X POST -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"${addr}\",{\"encoding\":\"jsonParsed\"}]}" \
    "$SOLANA_RPC_URL" | jq -r '.result.value // empty' 2>/dev/null || echo "")
  if [ -n "$val" ] && [ "$val" != "null" ]; then
    echo "  $name: $addr OK"
  else
    echo "  $name: $addr FAIL" >&2
    fail=1
  fi
}

echo "[verify-qa-onchain] RPCs: EVM $EVM_RPC_URL | EVM1 $EVM1_RPC_URL | Terra LCD $TERRA_LCD_URL | Solana $SOLANA_RPC_URL"
echo "Bridges / registry"
check_evm_code "EVM bridge" "${EVM_BRIDGE_ADDRESS:-}" "$EVM_RPC_URL"
check_evm_code "EVM1 bridge" "${EVM1_BRIDGE_ADDRESS:-}" "$EVM1_RPC_URL"
check_terra_cw "Terra bridge" "${TERRA_BRIDGE_ADDRESS:-}"
check_solana_prog "${SOLANA_PROGRAM_ID:-}"

echo "Token matrix (sample)"
check_evm_code "ANVIL_TOKEN_A" "${ANVIL_TOKEN_A:-}" "$EVM_RPC_URL"
check_evm_code "ANVIL_T2022" "${ANVIL_T2022:-}" "$EVM_RPC_URL"
check_evm_code "ANVIL1_TOKEN_A" "${ANVIL1_TOKEN_A:-}" "$EVM1_RPC_URL"
check_evm_code "ANVIL1_T2022" "${ANVIL1_T2022:-}" "$EVM1_RPC_URL"
check_terra_cw "TERRA_TOKEN_A" "${TERRA_TOKEN_A:-}"
check_terra_cw_optional_matrix "TERRA_T2022" "${TERRA_T2022:-}"
check_solana_mint "SOLANA_LUNC" "${SOLANA_LUNC:-}"
check_solana_mint "SOLANA_T2022" "${SOLANA_T2022:-}"

if [ "$fail" -ne 0 ]; then
  echo "[verify-qa-onchain] One or more checks failed." >&2
  exit 1
fi
echo "[verify-qa-onchain] All checks passed."
