#!/usr/bin/env bash
# GL-122: Single orchestrated entrypoint — gas preflight, dry-check, **one** forge `runBroadcastFull`
#          (head + Nick step 18 + faucet + tail), then optional ChainRegistry peer registration on the **new chain**.
#
# Usage (from repo root) — see `scripts/evm/megaeth-parity-quickstart.sh` for a one-line MegaETH default:
#   ./scripts/evm/megaeth-parity-quickstart.sh
# Manual:
#   export RPC_URL=https://your-chain.example
#   export PARITY_LEGACY_WETH_ADDRESS=...
#   export PARITY_LEGACY_CHAIN_IDENTIFIER=...
#   export PARITY_LEGACY_THIS_CHAIN_ID=...
#   export WETH_ADDRESS=...
#   export CHAIN_IDENTIFIER=...
#   export THIS_CHAIN_ID=...
#   FEE_RECIPIENT_ADDRESS optional — defaults to OPERATOR_ADDRESS when unset
#   export GUARD_STACK_ACCESS_MANAGER_ADMIN=...
#   ./scripts/evm/deploy-bsc-parity-orchestrate.sh --rpc-url "$RPC_URL" -vvv -i 1 --sender 0xYourDeployer
#
# Script arguments are forwarded to each `parity-replay.sh broadcast-*` forge invocation (typically --rpc-url, -vvv, signing).
# `dry-check` does not receive them — it is local (golden JSON + `vm.computeCreateAddress` only).
#
# Optional env:
#   FORGE — path to forge binary (default `forge`). Use e.g. ~/.local/bin/forge-parity after
#           ./scripts/evm/install-foundry-parity-fix.sh if stock forge fails decoding nonce-10 CREATE metadata.
#   FEE_RECIPIENT_ADDRESS — defaults to OPERATOR_ADDRESS when unset (same receiver as operator).
#   CHAIN_REGISTRY_ADDRESS — if set before Phase 6, registers peers automatically after tail.
#                            Otherwise Phase 6 prints the standalone helper command.
#   SKIP_PREFLIGHT=1 | SKIP_DRY_CHECK=1 | SKIP_PEER_REGISTER=1
#   USE_SEGMENTED_BROADCAST=1 — legacy four forge segments + manual Nick gate (resume / debugging only).
#   MIN_FULL_DEPLOY_BALANCE_WEI — forwarded to bsc-parity-preflight.sh (see docs/deployment-megaeth.md §5.0
#                                and scripts/evm/parity-sum-broadcast-gas-limits.sh to estimate from a fork run).
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export FORGE="${FORGE:-forge}"

# --- Canonical mainnet parity roles (GL-122 defaults; override via env only when intentional) ---
export DEPLOYER_ADDRESS="${DEPLOYER_ADDRESS:-0xD699EbC6930F593f0725D2a7dC58ACC65b41a08e}"
export ADMIN_ADDRESS="${ADMIN_ADDRESS:-0xCd4Eb82CFC16d5785b4f7E3bFC255E735e79F39c}"
export OPERATOR_ADDRESS="${OPERATOR_ADDRESS:-0x1d9e02e0e8c000FE4575c4Aaea96B19De00404CD}"
export FEE_RECIPIENT_ADDRESS="${FEE_RECIPIENT_ADDRESS:-$OPERATOR_ADDRESS}"
export CANCELER_ADDRESS="${CANCELER_ADDRESS:-0x732A65b80F4625658EbD2B4214E4f8Cf3A67AEEB}"

RPC_URL="${RPC_URL:?set RPC_URL}"
export RPC_URL

SKIP_PREFLIGHT="${SKIP_PREFLIGHT:-0}"
SKIP_DRY_CHECK="${SKIP_DRY_CHECK:-0}"
SKIP_PEER_REGISTER="${SKIP_PEER_REGISTER:-0}"
USE_SEGMENTED_BROADCAST="${USE_SEGMENTED_BROADCAST:-0}"

FORGE_EXTRA=("$@")

phase_preflight() {
  [[ "$SKIP_PREFLIGHT" == "1" ]] && return 0
  echo "=== Phase 0 — gas preflight (bsc-parity-preflight.sh) ==="
  "$ROOT/scripts/evm/bsc-parity-preflight.sh"
}

phase_dry_check() {
  [[ "$SKIP_DRY_CHECK" == "1" ]] && return 0
  echo "=== Phase 1 — runDryCheck (golden JSON / INV-PAR1) ==="
  DEPLOYER_ADDRESS="$DEPLOYER_ADDRESS" "$ROOT/scripts/evm/parity-replay.sh" dry-check
}

phase_broadcast_full() {
  echo "=== Phase 2 — runBroadcastFull (head + Nick step 18 + faucet + tail, one forge session) ==="
  "$ROOT/scripts/evm/parity-replay.sh" broadcast-full "${FORGE_EXTRA[@]}"
}

phase_segmented_broadcast() {
  echo "=== Phase 2a — runBroadcastHead (USE_SEGMENTED_BROADCAST=1) ==="
  "$ROOT/scripts/evm/parity-replay.sh" broadcast-head "${FORGE_EXTRA[@]}"

  echo "=== Phase 3 — Nick CREATE2 outer step 18 (manual) ==="
  local nonce
  nonce=$(cast nonce "$DEPLOYER_ADDRESS" --rpc-url "$RPC_URL")
  if [[ "$nonce" == "19" ]]; then
    echo "Deployer nonce already 19 — step 18 treated as complete."
  elif [[ "$nonce" != "18" ]]; then
    echo "Expected deployer nonce 18 after head (before step 18); got $nonce" >&2
    exit 1
  else
    cat <<'EOF'

Replay historical outer tx 18 (Nick CREATE2 factory) byte-identically before continuing:
  Tx hash (reference): 0xb55a2348487d743bad8d1e4484e31ebebab2c1ee2b75dd17fb1e3b2d20036dfb
See docs/deployment-megaeth.md §5.3 — raw calldata from BscScan / internal recording.

After broadcast, deployer nonce on RPC_URL must be 19.

EOF
    read -r -p "Press Enter when step 18 has been mined (nonce will read as 19)..."
    nonce=$(cast nonce "$DEPLOYER_ADDRESS" --rpc-url "$RPC_URL")
    if [[ "$nonce" != "19" ]]; then
      echo "After step 18, expected deployer nonce 19; got $nonce" >&2
      exit 1
    fi
  fi

  echo "=== Phase 4 — runBroadcastFaucet19 (nonce 19) ==="
  "$ROOT/scripts/evm/parity-replay.sh" broadcast-faucet19 "${FORGE_EXTRA[@]}"

  echo "=== Phase 5 — runBroadcastTail (nonces from TAIL_ENTRY_NONCE) ==="
  "$ROOT/scripts/evm/parity-replay.sh" broadcast-tail "${FORGE_EXTRA[@]}"
}

phase_register_peers() {
  [[ "$SKIP_PEER_REGISTER" == "1" ]] && return 0
  echo "=== Phase 6 — ChainRegistry peers (BSC / Terra / Solana) on **this** chain ==="
  if [[ -z "${CHAIN_REGISTRY_ADDRESS:-}" ]]; then
    cat <<EOF

CHAIN_REGISTRY_ADDRESS is unset — skipping interactive peer registration.
Export the ChainRegistry proxy from packages/contracts-evm/broadcast/EvmParityReplay.s.sol/<chainId>/runBroadcastFull-latest.json
(or script logs), then run:

  CHAIN_REGISTRY_ADDRESS=0x... RPC_URL=$RPC_URL \\
    ./scripts/evm/register-parity-peers-on-registry.sh

Reverse registrations (MegaETH on BSC / Terra / Solana) are **separate one-shot scripts** each — see docs/deployment-megaeth.md §5.5.

EOF
    return 0
  fi
  CHAIN_REGISTRY_ADDRESS="$CHAIN_REGISTRY_ADDRESS" RPC_URL="$RPC_URL" \
    "$ROOT/scripts/evm/register-parity-peers-on-registry.sh"
}

phase_preflight
phase_dry_check
if [[ "$USE_SEGMENTED_BROADCAST" == "1" ]]; then
  phase_segmented_broadcast
else
  phase_broadcast_full
fi

tail_nonce=$(cast nonce "$DEPLOYER_ADDRESS" --rpc-url "$RPC_URL")
echo ""
echo "=== Broadcast segments complete — deployer nonce on RPC_URL: $tail_nonce ==="
echo "    (Full greenfield parity: expect nonce **45** after outer txs 0–44; partial/resume differs.)"

phase_register_peers

echo ""
echo "=== GL-122 orchestrated deploy finished OK ==="
