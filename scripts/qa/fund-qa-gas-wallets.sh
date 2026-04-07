#!/usr/bin/env bash
# QA (`make start-qa`): fund comma-separated wallets with native gas on Anvil, Anvil1, and LocalTerra.
# Mirrors optional SOLANA_QA_AIRDROP_WALLETS — use when QA wallets (e.g. browser extension) need ETH/LUNC.
#
# Env:
#   EVM_RPC_URL / EVM1_RPC_URL — from qa-host.env + .env
#   EVM_QA_FUND_WALLETS — comma-separated 0x addresses for primary Anvil (empty = skip)
#   EVM1_QA_FUND_WALLETS — comma-separated for second Anvil; if empty but EVM_QA_FUND_WALLETS is set, reuses that list
#   EVM_QA_FUND_ETH — wei/ether string per recipient on Anvil (default 50ether)
#   EVM1_QA_FUND_ETH — same for Anvil1 (defaults to EVM_QA_FUND_ETH)
#   EVM_QA_FUND_PRIVATE_KEY — sender on Anvil (default EVM_PRIVATE_KEY, Anvil account #0 in QA)
#   EVM1_QA_FUND_PRIVATE_KEY — sender on Anvil1 (defaults to EVM_QA_FUND_PRIVATE_KEY)
#   TERRA_QA_FUND_WALLETS — comma-separated terra1… bech32 addresses (empty = skip)
#   TERRA_QA_FUND_COINS — bank send amount, e.g. 100000000uluna (default 100000000uluna)
#   TERRA_KEY_NAME — terrad key in container (default test1)
#   TERRA_MNEMONIC — imported into container if key missing (same as deploy-terra-local)
#   LOCALTERRA_DOCKER_CONTAINER / LOCALTERRA_CONTAINER — docker exec target; else compose ps -q localterra
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

EVM_RPC="${EVM_RPC_URL:-http://127.0.0.1:8545}"
EVM1_RPC="${EVM1_RPC_URL:-http://127.0.0.1:8546}"
RAW_EVM="${EVM_QA_FUND_WALLETS:-}"
RAW_EVM1="${EVM1_QA_FUND_WALLETS:-}"
RAW_TERRA="${TERRA_QA_FUND_WALLETS:-}"

EVM_AMT="${EVM_QA_FUND_ETH:-50ether}"
EVM1_AMT="${EVM1_QA_FUND_ETH:-$EVM_AMT}"
FUND_PK="${EVM_QA_FUND_PRIVATE_KEY:-${EVM_PRIVATE_KEY:-}}"
FUND_PK1="${EVM1_QA_FUND_PRIVATE_KEY:-$FUND_PK}"

TERRA_COINS="${TERRA_QA_FUND_COINS:-100000000uluna}"
TERRA_FROM="${TERRA_KEY_NAME:-test1}"
TEST_MNEMONIC="${TERRA_MNEMONIC:-notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius}"

trim() {
  local s="$1"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

# Dedupe CSV → bash array (bash 4+)
csv_to_uniq_addrs() {
  local raw="$1"
  local -n _out=$2
  _out=()
  declare -A _seen=()
  local part t
  while IFS= read -r part || [ -n "$part" ]; do
    t="$(trim "$part")"
    [ -z "$t" ] && continue
    if [[ -n "${_seen[$t]+x}" ]]; then
      continue
    fi
    _seen["$t"]=1
    _out+=("$t")
  done < <(printf '%s' "$raw" | tr ',' '\n')
}

resolve_localterra_container() {
  if [ -n "${LOCALTERRA_DOCKER_CONTAINER:-}" ]; then
    printf '%s' "$LOCALTERRA_DOCKER_CONTAINER"
    return
  fi
  if [ -n "${LOCALTERRA_CONTAINER:-}" ]; then
    printf '%s' "$LOCALTERRA_CONTAINER"
    return
  fi
  local id
  id="$(cd "$REPO_ROOT" && docker compose ps -q localterra 2>/dev/null | head -1 || true)"
  if [ -n "$id" ]; then
    printf '%s' "$id"
    return
  fi
  printf '%s' 'cl8y-bridge-monorepo-localterra-1'
}

fund_evm_list() {
  local rpc="$1"
  local raw="$2"
  local amount="$3"
  local pk="$4"
  local label="$5"

  if [ -z "${raw//[[:space:],]/}" ]; then
    return 0
  fi
  if ! command -v cast >/dev/null 2>&1; then
    echo "[fund-qa-gas-wallets] cast not found — skip ${label} native funding (install Foundry)." >&2
    return 0
  fi
  if [ -z "${pk//[[:space:]]/}" ]; then
    echo "[fund-qa-gas-wallets] No private key for ${label} — skip." >&2
    return 0
  fi

  local -a addrs=()
  csv_to_uniq_addrs "$raw" addrs
  if [ "${#addrs[@]}" -eq 0 ]; then
    return 0
  fi

  echo "[fund-qa-gas-wallets] ${label}: sending ${amount} each to ${#addrs[@]} address(es) via ${rpc}..."

  local addr
  for addr in "${addrs[@]}"; do
    if [[ ! "$addr" =~ ^0x[0-9a-fA-F]{40}$ ]]; then
      echo "[fund-qa-gas-wallets] WARN: skip invalid EVM address: ${addr}" >&2
      continue
    fi
    local _cast_out
    if _cast_out="$(cast send "$addr" --value "$amount" --rpc-url "$rpc" --private-key "$pk" 2>&1)"; then
      echo "[fund-qa-gas-wallets] ${label}: funded ${addr}"
    else
      echo "[fund-qa-gas-wallets] ERROR: cast send failed for ${addr} on ${label}: ${_cast_out}" >&2
      return 1
    fi
  done
}

ensure_terra_key_in_container() {
  local c="$1"
  if docker exec "$c" terrad keys show "$TERRA_FROM" --keyring-backend test >/dev/null 2>&1; then
    return 0
  fi
  echo "[fund-qa-gas-wallets] Importing ${TERRA_FROM} into LocalTerra container (terrad keys add --recover)..."
  printf '%s\n' "$TEST_MNEMONIC" | docker exec -i "$c" terrad keys add "$TERRA_FROM" --recover --keyring-backend test >/dev/null
}

fund_terra_list() {
  local raw="$1"
  if [ -z "${raw//[[:space:],]/}" ]; then
    return 0
  fi
  if ! command -v docker >/dev/null 2>&1; then
    echo "[fund-qa-gas-wallets] docker not found — skip Terra funding." >&2
    return 0
  fi

  local c
  c="$(resolve_localterra_container)"
  if ! docker inspect "$c" >/dev/null 2>&1; then
    echo "[fund-qa-gas-wallets] LocalTerra container not found (${c}) — skip Terra funding." >&2
    return 0
  fi
  local running
  running="$(docker inspect -f '{{.State.Running}}' "$c" 2>/dev/null || echo false)"
  if [ "$running" != "true" ]; then
    echo "[fund-qa-gas-wallets] LocalTerra container not running (${c}) — skip Terra funding." >&2
    return 0
  fi

  local -a addrs=()
  csv_to_uniq_addrs "$raw" addrs
  if [ "${#addrs[@]}" -eq 0 ]; then
    return 0
  fi

  ensure_terra_key_in_container "$c"

  echo "[fund-qa-gas-wallets] Terra: sending ${TERRA_COINS} each to ${#addrs[@]} address(es) (from ${TERRA_FROM})..."

  local addr
  for addr in "${addrs[@]}"; do
    if [[ ! "$addr" =~ ^terra1[a-z0-9]+$ ]]; then
      echo "[fund-qa-gas-wallets] WARN: skip suspicious Terra address: ${addr}" >&2
      continue
    fi
    local _tx_out
    if _tx_out="$(docker exec "$c" terrad tx bank send "$TERRA_FROM" "$addr" "$TERRA_COINS" \
      --chain-id localterra --node http://localhost:26657 \
      --gas auto --gas-adjustment 1.5 --keyring-backend test -y 2>&1)"; then
      echo "[fund-qa-gas-wallets] Terra: funded ${addr}"
    else
      echo "[fund-qa-gas-wallets] ERROR: terrad bank send failed for ${addr}: ${_tx_out}" >&2
      return 1
    fi
  done
}

# EVM1 list: explicit EVM1_QA_FUND_WALLETS, else mirror EVM list when set
EVM1_EFFECTIVE="$RAW_EVM1"
if [ -z "${EVM1_EFFECTIVE//[[:space:],]/}" ] && [ -n "${RAW_EVM//[[:space:],]/}" ]; then
  EVM1_EFFECTIVE="$RAW_EVM"
fi

fund_evm_list "$EVM_RPC" "$RAW_EVM" "$EVM_AMT" "$FUND_PK" "Anvil (EVM_RPC_URL)" || exit 1
fund_evm_list "$EVM1_RPC" "$EVM1_EFFECTIVE" "$EVM1_AMT" "$FUND_PK1" "Anvil1 (EVM1_RPC_URL)" || exit 1
fund_terra_list "$RAW_TERRA" || exit 1

echo "[fund-qa-gas-wallets] Done (EVM / EVM1 / Terra sections that were configured)."
