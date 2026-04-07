#!/usr/bin/env bash
# Setup test SPL token mints for Solana bridge testing.
# Creates three test tokens (testa, testb, tdec) with varying decimals,
# creates an ATA for the deployer, and mints an initial supply (deployer = mint authority).
#
# Mainnet / production: the bridge program performs MintBurn-style SPL mint/burn; this script
# does not use cl8y_faucet. Export SOLANA_PROGRAM_ID so the summary can print VITE_SOLANA_PROGRAM_ID.
# After registration, transfer mint authority to the bridge as required by your runbook.
#
# Prerequisites:
#   - solana CLI configured for the target cluster
#   - spl-token CLI installed (supports --output json)
#   - Funded payer keypair (SOLANA_KEYPAIR)
#
# Usage:
#   ./scripts/solana/setup-test-tokens.sh
#
# Logs go to stderr; only the new mint address is printed to stdout from
# create_token() so TESTA_MINT=$(create_token ...) captures a single address.

set -euo pipefail

SOLANA_RPC="${SOLANA_RPC_URL:-http://localhost:8899}"
KEYPAIR="${SOLANA_KEYPAIR:-$HOME/.config/solana/id.json}"

log() {
  echo "$@" >&2
}

log "=== CL8Y Solana Test Token Setup ==="
log "RPC:     $SOLANA_RPC"
log "Keypair: $KEYPAIR (fee payer + mint authority for create/mint below)"
if [[ -n "${SOLANA_PROGRAM_ID:-}" ]]; then
  log "Bridge:  $SOLANA_PROGRAM_ID  (MintBurn path on-chain — not the faucet program)"
else
  log "Bridge:  (set SOLANA_PROGRAM_ID to echo VITE_SOLANA_PROGRAM_ID in the summary)"
fi
if [[ -n "${FAUCET_PROGRAM_ID:-}" ]]; then
  log "Faucet:  $FAUCET_PROGRAM_ID  (optional; QA/devnet only — omit on mainnet bridge deployment)"
fi
log ""

# Extract mint address from spl-token create-token output (JSON or plain text).
parse_create_token_mint() {
  python3 -c "
import json, re, sys

def emit_if_mint(s):
    if isinstance(s, str) and re.fullmatch(r'[1-9A-HJ-NP-Za-km-z]{32,44}', s):
        print(s)
        return True
    return False

raw = sys.stdin.read().strip()
# JSON (stdout with --output json)
try:
    d = json.loads(raw)
    # spl-token CLI wraps result in commandOutput { address, ... }
    co = d.get('commandOutput')
    if isinstance(co, dict):
        for k in ('address', 'mint', 'mintAddress'):
            if emit_if_mint(co.get(k)):
                raise SystemExit(0)
    for k in ('address', 'mint', 'mintAddress'):
        if emit_if_mint(d.get(k)):
            raise SystemExit(0)
    for v in d.values():
        if emit_if_mint(v):
            raise SystemExit(0)
except (json.JSONDecodeError, TypeError, AttributeError):
    pass
# Text fallbacks
for line in raw.splitlines():
    line = line.strip()
    if line.lower().startswith('address:'):
        parts = line.split()
        if len(parts) >= 2:
            print(parts[1])
            raise SystemExit(0)
    if 'creating token' in line.lower():
        for p in line.split():
            if re.fullmatch(r'[1-9A-HJ-NP-Za-km-z]{32,44}', p):
                print(p)
                raise SystemExit(0)
raise SystemExit(1)
" 2>/dev/null
}

create_token() {
  local name="$1"
  local decimals="$2"
  local tmp_out tmp_err mint
  tmp_out="$(mktemp)"
  tmp_err="$(mktemp)"

  log "--- Creating $name (decimals=$decimals) ---"

  # spl-token defaults --mint-authority to the Solana CLI *config* keypair (often ~/.config/solana/id.json),
  # not --fee-payer. Pin mint authority to the same wallet as KEYPAIR (e.g. id-deployer).
  local payer_pubkey
  payer_pubkey="$(solana-keygen pubkey "$KEYPAIR")"

  if ! spl-token create-token \
    --url "$SOLANA_RPC" \
    --decimals "$decimals" \
    --fee-payer "$KEYPAIR" \
    --mint-authority "$payer_pubkey" \
    --output json >"$tmp_out" 2>"$tmp_err"; then
    log "ERROR: spl-token create-token failed for $name"
    cat "$tmp_err" >&2 || true
    cat "$tmp_out" >&2 || true
    rm -f "$tmp_out" "$tmp_err"
    return 1
  fi
  [[ -s "$tmp_err" ]] && cat "$tmp_err" >&2 || true

  mint=$(parse_create_token_mint <"$tmp_out") || true
  if [[ -z "$mint" ]]; then
    log "ERROR: Could not parse mint address from spl-token output for $name:"
    cat "$tmp_out" >&2
    rm -f "$tmp_out" "$tmp_err"
    return 1
  fi

  rm -f "$tmp_out" "$tmp_err"

  log "$name mint: $mint"

  # spl-token prints "Creating account…" / signatures to stdout — must not go to stdout here or
  # TESTA_MINT=$(create_token) captures junk.
  spl-token create-account "$mint" \
    --url "$SOLANA_RPC" \
    --fee-payer "$KEYPAIR" \
    --owner "$(solana-keygen pubkey "$KEYPAIR")" \
    1>&2 || true

  local amount
  amount=$(python3 -c "print(10**($decimals + 6))" 2>/dev/null || echo "1000000000")
  if ! spl-token mint "$mint" "$amount" \
    --url "$SOLANA_RPC" \
    --fee-payer "$KEYPAIR" \
    --mint-authority "$KEYPAIR" \
    1>&2; then
    log "  (mint initial supply skipped — check balance / authority)"
  fi

  log "  Done $name: $mint"
  log ""
  # Only stdout for caller capture:
  printf '%s\n' "$mint"
}

log "Creating test tokens..."
log ""

TESTA_MINT=$(create_token "testa" 9)
TESTB_MINT=$(create_token "testb" 9)
TDEC_MINT=$(create_token "tdec" 6)

log ""
log "=== Summary ==="
log "testa mint: $TESTA_MINT"
log "testb mint: $TESTB_MINT"
log "tdec  mint: $TDEC_MINT"
log ""
log "Add these to your frontend .env:"
log "  VITE_SOLANA_TESTA_MINT=$TESTA_MINT"
log "  VITE_SOLANA_TESTB_MINT=$TESTB_MINT"
log "  VITE_SOLANA_TDEC_MINT=$TDEC_MINT"
if [[ -n "${SOLANA_PROGRAM_ID:-}" ]]; then
  log "  VITE_SOLANA_PROGRAM_ID=$SOLANA_PROGRAM_ID"
fi
if [[ -n "${FAUCET_PROGRAM_ID:-}" ]]; then
  log "  VITE_SOLANA_FAUCET_ADDRESS=$FAUCET_PROGRAM_ID  # only if you use cl8y_faucet (not mainnet bridge MintBurn)"
fi
