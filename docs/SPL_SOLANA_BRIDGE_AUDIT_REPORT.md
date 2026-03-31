# SPL / Solana bridge audit report

**Date:** 2026-03-31  
**Scope:** Anchor programs `cl8y-bridge` and `cl8y-faucet` under `packages/contracts-solana/programs/`, SPL custody paths, hash parity vs EVM/Terra, frontend and off-chain verification, and automated tests.  
**Assumption:** Governance is trusted per [SOLANA_BRIDGE_INVARIANTS.md](./SOLANA_BRIDGE_INVARIANTS.md) (honest admin, operator, registered cancelers). Findings below note residual risks where that assumption does not hold.

---

## 1. Executive summary

- **V2 transfer hash (INV-H1):** Solana `compute_transfer_hash` matches the 224-byte ABI layout used by [multichain-rs/src/hash.rs](../packages/multichain-rs/src/hash.rs), [HashLib.sol](../packages/contracts-evm/src/lib/HashLib.sol) `computeXchainHashId`, and Terra `compute_xchain_hash_id`. `git diff origin/main` shows **no changes** to `HashLib.sol` on this branch; Terra/multichain-rs diffs vs `main` are **non-V2** (e.g. `div_ceil`, extra proptests).
- **Golden vectors:** Five `HashLib.t.sol` `test_DepositWithdraw_*` digests are asserted in Rust (`cl8y_bridge::hash` unit tests), TypeScript ([hash_parity.test.ts](../packages/contracts-solana/tests/hash_parity.test.ts)), frontend Vitest ([hashVerification.test.ts](../packages/frontend/src/services/hashVerification.test.ts)), and E2E ([test_solana_flows.rs](../packages/e2e/tests/test_solana_flows.rs) `test_hash_goldens_match_hashlib_t_sol`). All were run successfully after sync.
- **Frontend:** Production V2 hashing is centralized in [hashVerification.ts](../packages/frontend/src/services/hashVerification.ts). Terra native string hashing uses the same viem `keccak256(toBytes(...))` pattern in [terraTokenEncoding.ts](../packages/frontend/src/services/terraTokenEncoding.ts) and [useTokenVerification.ts](../packages/frontend/src/hooks/useTokenVerification.ts) (parallel, not reusing one helper). Playwright/e2e-infra uses `cast keccak` for some fixtures.
- **Security (non-governance):** Core controls (pause, fees, SPL `transfer_checked`, PDA-bound pending withdraws, canceler + delay window, executed-hash replay protection) align with documented invariants and integration tests. **Token-2022** coverage remains explicitly partial in the invariants matrix.
- **Fuzzing:** `cargo-fuzz` targets hash packing only; no instruction-level fuzzing ([SOLANA_FUZZING.md](./SOLANA_FUZZING.md)).
- **Live Solana E2E:** Rust canceler/operator Solana scenarios require `SOLANA_ENABLED` and related env; they are **not** the same as CI `anchor test` or offline `test_solana_flows.rs`.

---

## 2. Hash parity verdict

| Implementation | Path | Status |
|----------------|------|--------|
| EVM | `packages/contracts-evm/src/lib/HashLib.sol` `computeXchainHashId` | Reference; unchanged vs `origin/main` on this branch |
| Terra Classic V2 | `packages/contracts-terraclassic/bridge/src/hash.rs` | Same V2 layout; legacy helper padding refactored (`div_ceil`) only |
| Solana | `packages/contracts-solana/programs/cl8y-bridge/src/hash.rs` | Matches multichain-rs indices; `solana_program::keccak` vs `tiny-keccak` tested |
| Off-chain Rust | `packages/multichain-rs/src/hash.rs` | Same layout; proptest module added on branch |
| Frontend | `packages/frontend/src/services/hashVerification.ts` | viem `encodeAbiParameters` + `keccak256` matches Solidity ABI encoding |

**Call sites of `compute_transfer_hash` on Solana:** `deposit_native.rs`, `deposit_spl.rs`, `withdraw_submit.rs` (PDA seeds + stored hash), `withdraw_execute.rs`, `withdraw_execute_native.rs` (recompute vs stored).

**Bounds (INV-H1):** On-chain Solana uses `u128` amount and `u64` nonce in the hash helper; EVM may pass larger `uint256` amounts—parity requires amount fitting in u128 with high 16 bytes zero in the ABI word.

---

## 3. Frontend and off-chain deduplication matrix

| Location | Purpose | Canonical? |
|----------|---------|--------------|
| `frontend/src/services/hashVerification.ts` | V2 `computeXchainHashId` / `computeXchainHashIdFromBytes` | **Yes (INV-HFE1)** |
| `frontend/src/services/solana/transaction.ts` | Delegates to `hashVerification` for bytes32 digest | Yes |
| `frontend/src/services/terraTokenEncoding.ts` | `keccak256(UTF-8)` for Terra native token ids | Justified (string → bytes32); same viem primitive as `keccak256Uluna` |
| `frontend/src/hooks/useTokenVerification.ts` | Terra/Solana mapping helpers with `keccak256(toBytes(...))` | Parallel to `terraTokenEncoding`; **dedup opportunity** |
| `frontend/src/services/terraBridgeQueries.ts` | Fallback `keccak256(toBytes(token))` when decode fails | Production path; **should stay consistent** with Terra on-chain encoding rules |
| `frontend/e2e/fixtures/transfer-helpers.ts` | `cast keccak` over ABI-encoded args | Test/infra only; duplicates viem path |
| `frontend/src/test/e2e-infra/register-tokens.ts` | `cast keccak` cache for denom strings | Infra only |
| `contracts-solana/tests/*.test.ts` | Copied `js-sha3` + 224-byte buffer helpers | Test-only; many files—**consolidation optional** |
| `contracts-solana/scripts/register-qa-tokens.ts` | `cast keccak` for UTF-8 strings | Script only |

**Verify page behavior:** [useHashVerification.ts](../packages/frontend/src/hooks/useHashVerification.ts) computes `computedHash` from RPC-derived fields and sets `matches` via **per-field** equality (with zero-bytes32 skips for Terra quirks). It does **not** require `computedHash === normalizeHash(userInput)` for the match flag; `hash` exposed to UI prefers `computedHash` when data exists. This is **consistent** with treating chain records as source of truth while still using the same `computeXchainHashId` for recomputation.

---

## 4. Tests and CI

### 4.1 Layers

| Layer | Command / location | CI |
|-------|-------------------|-----|
| Rust unit (bridge) | `cargo test` in `programs/cl8y-bridge` | Via `anchor build` / local |
| Proptest | `hash.rs`, `decimal.rs`, `rate_limit.rs`; multichain-rs `proptest_xchain_hash` | Same |
| Anchor integration | `anchor test` in `packages/contracts-solana` | [.github/workflows/test.yml](../.github/workflows/test.yml) `contracts-solana` job |
| cargo-fuzz | `programs/cl8y-bridge/fuzz/` | Optional / manual |
| E2E offline | `cargo test --test test_solana_flows` in `packages/e2e` | Not isolated in `test.yml`; runs with full `packages/e2e` when executed |
| Frontend unit | `vitest run hashVerification.test.ts` | Frontend job (if present in repo) |

### 4.2 INV ↔ test mapping (spot-checked)

| Invariant | Primary tests |
|-----------|----------------|
| INV-H1 | `hash.rs` unit tests, `hash_parity.test.ts`, `hashVerification.test.ts`, `test_solana_flows.rs` |
| INV-H2 | `deposit_*`, `withdraw_submit`, registry tests (`bridge.test.ts`, docs narrative) |
| INV-W1–W3 | `deposit_withdraw.test.ts`, `cancel_flow.test.ts`, `cancel_blocks_theft.test.ts`, `rate_limit_integration.test.ts`, `full_security_audit.test.ts` |
| INV-D1–D2 | `spl_security.test.ts`, `deposit_withdraw.test.ts`, `security_audit.test.ts`, `hardening.test.ts` |
| INV-F1 | `faucet.test.ts` |

### 4.3 Gaps (residual)

- **Token-2022:** Matrix row in [SOLANA_BRIDGE_INVARIANTS.md](./SOLANA_BRIDGE_INVARIANTS.md) marks explicit coverage gap.
- **Instruction fuzzing:** Not implemented; hash-only fuzz per [SOLANA_FUZZING.md](./SOLANA_FUZZING.md).
- **Live Solana E2E:** `canceler_solana_destination.rs` and related tests gated on `SOLANA_ENABLED`; confirm release checklist runs them when Solana is in scope.
- **Rust integration tests inside the program crate:** Behavior exercised primarily via Anchor TS + validator.

---

## 5. On-chain security (governance trusted)

- **Deposits:** `deposit_spl.rs` ties mint to `TokenMapping`, uses `transfer_checked` with mint decimals, charges fees, computes hash with **net** amount and **destination** token from mapping; `depositor` is signer.
- **Withdrawals:** `withdraw_submit` seeds `PendingWithdraw` and `ExecutedHash` PDAs with `compute_transfer_hash`; `withdraw_execute*` recomputes hash and enforces delay/flags/rate limits (per tests and instruction code).
- **Cancel:** `withdraw_cancel.rs` requires active `CancelerEntry` for signer, approved pending withdraw, within `withdraw_delay` window after approval—blocks arbitrary cancelers and late cancel (see `cancel_blocks_theft.test.ts`).
- **Roles:** Admin-only registration/config; operator `withdraw_approve`; pause blocks user deposits/withdraws (tests in security suites).
- **If governance malicious:** Admin can pause, set fees, register tokens/chains/cancelers, drain fees via `withdraw_fees`; operator can approve withdrawals; canceler can cancel within window—**by design** under trusted-governance model.

---

## 6. Off-chain Solana alignment

- **Canceler:** [verifier.rs](../packages/canceler/src/verifier.rs) imports `compute_xchain_hash_id` from [canceler/src/hash.rs](../packages/canceler/src/hash.rs) (re-export of multichain-rs). Verification compares source deposits to approval parameters using the same V2 hash.
- **Operator:** [writers/solana.rs](../packages/operator/src/writers/solana.rs) submits `withdraw_approve` after verifying EVM `getDeposit` or Terra LCD paths; does not reimplement the 224-byte layout in this file.
- **Config risk:** `SOLANA_PROGRAM_ID` must match deployed program for PDA tests in `test_solana_flows.rs` (default matches Anchor workspace id).

---

## 7. Recommendations (non-blocking)

1. Optionally **merge** `useTokenVerification` Terra keccak helpers with `terraTokenEncoding.ts` to reduce drift risk.
2. Optionally **extract** shared `computeTransferHashTs` (viem or js-sha3) into `contracts-solana/tests/helpers/hash.ts` and import from multiple test files.
3. Replace **cast keccak** in Playwright/e2e helpers with viem where feasible so one toolchain owns V2 encoding in TS.
4. Add **explicit Token-2022** deposit/withdraw integration cases if production uses Token-2022 mints.
5. Consider **CI job** that runs `cargo test -p cl8y-e2e --test test_solana_flows` (fast, no RPC) if not already covered by a monolithic e2e test run.

---

## 8. Verification commands used

```bash
# Solana program hash unit tests
cd packages/contracts-solana/programs/cl8y-bridge && cargo test hash::

# Anchor TS hash parity
cd packages/contracts-solana && npx ts-mocha -t 120000 tests/hash_parity.test.ts

# E2E offline Solana + goldens
cd packages/e2e && cargo test --test test_solana_flows

# Frontend canonical hash tests
cd packages/frontend && npx vitest run src/services/hashVerification.test.ts
```

All completed successfully at audit time.
