#!/usr/bin/env bash
# Cross-chain V2 transfer hash parity: EVM (forge), Solana (cl8y-bridge unit tests),
# Rust multichain-rs + CosmWasm bridge, frontend vitest — no validators or RPC.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== multichain-rs (goldens + CosmWasm agreement proptest) =="
(cd packages/multichain-rs && cargo test --locked \
  --test v2_hash_golden_vectors \
  --test hash_agrees_with_cosmwasm_bridge)

echo "== CosmWasm bridge (Terra package, standalone goldens) =="
(cd packages/contracts-terraclassic && cargo test -p bridge --test v2_hash_goldens --locked)

echo "== Solana cl8y-bridge (hash module unit tests + proptest) =="
(cd packages/contracts-solana && cargo test -p cl8y-bridge hash:: --locked)

echo "== EVM HashLib (V2 vectors + fuzz) =="
(cd packages/contracts-evm && forge test --match-path test/HashLib.t.sol --summary)

echo "== Frontend (goldens + canonical audit + fuzz + hashVerification) =="
(cd packages/frontend && npx vitest run \
  src/services/v2XchainHash.goldens.test.ts \
  src/services/hashCanonical.audit.test.ts \
  src/services/crossChainHash.fuzz.test.ts \
  src/services/hashVerification.test.ts)

echo "All xchain hash parity checks passed."
