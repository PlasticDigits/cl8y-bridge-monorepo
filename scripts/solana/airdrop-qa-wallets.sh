#!/usr/bin/env bash
# QA (`make start-qa`): fund comma-separated pubkeys on localnet with throttled chunk airdrops.
# Env: SOLANA_QA_AIRDROP_WALLETS (required, comma-separated base58), SOLANA_RPC_URL,
#      SOLANA_QA_AIRDROP_SOL (default 100), SOLANA_QA_AIRDROP_CHUNK_SOL (default 2),
#      SOLANA_QA_AIRDROP_SLEEP_MS (default 1000), SOLANA_QA_AIRDROP_MAX_RETRIES (default 5).
set -euo pipefail

RPC_URL="${SOLANA_RPC_URL:-http://127.0.0.1:8899}"
RAW="${SOLANA_QA_AIRDROP_WALLETS:-}"
TARGET_SOL="${SOLANA_QA_AIRDROP_SOL:-100}"
CHUNK_SOL="${SOLANA_QA_AIRDROP_CHUNK_SOL:-2}"
SLEEP_MS="${SOLANA_QA_AIRDROP_SLEEP_MS:-1000}"
MAX_RETRIES="${SOLANA_QA_AIRDROP_MAX_RETRIES:-5}"

if [ -z "${RAW//[[:space:],]/}" ]; then
  exit 0
fi

if ! command -v solana >/dev/null 2>&1; then
  echo "[airdrop-qa-wallets] solana CLI not found — skip extra QA wallet airdrops." >&2
  exit 0
fi

sleep_ms() {
  local ms="${1:-1000}"
  local sec
  sec="$(awk -v ms="$ms" 'BEGIN{ s = ms/1000; if (s < 0.05) s = 0.05; printf "%.3f", s }')"
  sleep "$sec"
}

trim() {
  local s="$1"
  s="${s#"${s%%[![:space:]]*}"}"
  s="${s%"${s##*[![:space:]]}"}"
  printf '%s' "$s"
}

# Dedupe while preserving order (bash 4+)
declare -A _seen=()
uniq_addrs=()
# Use `|| [ -n "$part" ]` so the last CSV segment is kept when it has no trailing newline (common in .env).
while IFS= read -r part || [ -n "$part" ]; do
  t="$(trim "$part")"
  [ -z "$t" ] && continue
  if [[ -n "${_seen[$t]+x}" ]]; then
    continue
  fi
  _seen["$t"]=1
  uniq_addrs+=("$t")
done < <(printf '%s' "$RAW" | tr ',' '\n')

if [ "${#uniq_addrs[@]}" -eq 0 ]; then
  exit 0
fi

echo "[airdrop-qa-wallets] Funding ${#uniq_addrs[@]} wallet(s) on $RPC_URL (target ${TARGET_SOL} SOL each, chunk ${CHUNK_SOL} SOL, sleep ${SLEEP_MS}ms)..."

current_balance() {
  local pub="$1"
  local out
  out="$(solana balance "$pub" --url "$RPC_URL" 2>/dev/null | awk '{print $1; exit}')"
  if [ -z "$out" ]; then
    echo "0"
    return
  fi
  printf '%s' "$out"
}

# Returns 0 if c < t (float), else 1
float_lt() {
  awk -v c="$1" -v t="$2" 'BEGIN{ exit !(c+0 < t+0) }'
}

# Prints min(need, chunk) as a string suitable for solana airdrop, or empty if need <= 0
next_chunk_amount() {
  awk -v c="$1" -v t="$2" -v ch="$3" 'BEGIN{
    need = t - c
    if (need <= 0) { print ""; exit }
    x = (need < ch) ? need : ch
    printf "%.9f", x+0
  }'
}

fund_one() {
  local pub="$1"
  local cur need_chunk attempt

  cur="$(current_balance "$pub")"
  if ! float_lt "$cur" "$TARGET_SOL"; then
    echo "[airdrop-qa-wallets] $pub already >= ${TARGET_SOL} SOL (balance ${cur}) — skip"
    return 0
  fi

  echo "[airdrop-qa-wallets] $pub: balance ${cur} SOL → target ${TARGET_SOL} SOL"

  while float_lt "$cur" "$TARGET_SOL"; do
    need_chunk="$(next_chunk_amount "$cur" "$TARGET_SOL" "$CHUNK_SOL")"
    if [ -z "$need_chunk" ]; then
      break
    fi

    attempt=1
    while [ "$attempt" -le "$MAX_RETRIES" ]; do
      if solana airdrop "$need_chunk" "$pub" --url "$RPC_URL" 2>/dev/null; then
        sleep_ms "$SLEEP_MS"
        break
      fi
      echo "[airdrop-qa-wallets] airdrop ${need_chunk} SOL failed for $pub (try ${attempt}/${MAX_RETRIES}), backing off..." >&2
      sleep_ms "$((attempt * SLEEP_MS))"
      attempt=$((attempt + 1))
    done

    if [ "$attempt" -gt "$MAX_RETRIES" ]; then
      echo "[airdrop-qa-wallets] ERROR: could not airdrop to $pub after ${MAX_RETRIES} retries" >&2
      return 1
    fi

    cur="$(current_balance "$pub")"
  done

  echo "[airdrop-qa-wallets] $pub done — balance $(solana balance "$pub" --url "$RPC_URL" 2>/dev/null || echo '?')"
  return 0
}

first=1
for pub in "${uniq_addrs[@]}"; do
  if [ "$first" -eq 0 ]; then
    sleep_ms "$SLEEP_MS"
  fi
  first=0
  fund_one "$pub" || exit 1
done

echo "[airdrop-qa-wallets] All listed wallets funded OK."
