#!/usr/bin/env bash
# Fund the Anchor/Solana deploy keypair on localnet so anchor deploy can pay rent.
# Uses SOLANA_RPC_URL (default http://127.0.0.1:8899) and ANCHOR_WALLET or ~/.config/solana/id.json.
set -euo pipefail

RPC_URL="${SOLANA_RPC_URL:-http://127.0.0.1:8899}"
KEYPAIR="${ANCHOR_WALLET:-${HOME}/.config/solana/id.json}"

if [ ! -f "$KEYPAIR" ]; then
  echo "[airdrop-for-anchor-deploy] No keypair at $KEYPAIR — skip airdrop." >&2
  exit 0
fi

if ! command -v solana >/dev/null 2>&1; then
  echo "[airdrop-for-anchor-deploy] solana CLI not found — skip airdrop." >&2
  exit 0
fi

PUB="$(solana-keygen pubkey "$KEYPAIR")"
# Program deploy needs several SOL on a fresh local validator.
BIG_AIRDROP="${SOLANA_DEPLOY_AIRDROP_SOL:-15}"

echo "[airdrop-for-anchor-deploy] Funding $PUB on $RPC_URL ..."

if solana airdrop "$BIG_AIRDROP" "$PUB" --url "$RPC_URL" 2>/dev/null; then
  solana balance "$PUB" --url "$RPC_URL" || true
  exit 0
fi

echo "[airdrop-for-anchor-deploy] Large airdrop failed or capped; retrying 2 SOL chunks (localnet)..."
ok=0
for _ in {1..12}; do
  if solana airdrop 2 "$PUB" --url "$RPC_URL" 2>/dev/null; then
    ok=1
  fi
done
if [ "$ok" -eq 1 ]; then
  solana balance "$PUB" --url "$RPC_URL" || true
  exit 0
fi

echo "[airdrop-for-anchor-deploy] Could not fund deployer — add SOL manually or check SOLANA_RPC_URL." >&2
exit 1
