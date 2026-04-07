# SPL bridge security audit (implementation record)

This document implements the structured audit for SPL-related logic in `packages/contracts-solana/programs/cl8y-bridge/`: business logic walkthrough, invariant mapping, a 20-class attack matrix with **exact** test references, fuzz/property scope, governance assumptions, and command evidence from this workspace.

**Companion docs:** [SOLANA_BRIDGE_INVARIANTS.md](SOLANA_BRIDGE_INVARIANTS.md), [SOLANA_BRIDGE_DEPOSITS.md](SOLANA_BRIDGE_DEPOSITS.md), [SOLANA_FUZZING.md](SOLANA_FUZZING.md).

---

## 1. Code walkthrough (SPL paths vs invariants)

### 1.1 `deposit_spl` ([`deposit_spl.rs`](../packages/contracts-solana/programs/cl8y-bridge/src/instructions/deposit_spl.rs))

- **Paused / zero amount:** `BridgePaused`, `ZeroAmount` (INV-D2 partial).
- **Fee split:** `deposit_fee_and_net(amount, fee_bps)` in [`fee.rs`](../packages/contracts-solana/programs/cl8y-bridge/src/fee.rs) — `fee = floor(amount * fee_bps / 10000)`, `net = amount - fee`; on-chain `fee_bps` capped at **100** (1%) in `initialize` / `set_config`.
- **Lock/unlock:** `transfer_checked` moves **full gross** `amount` to `bridge_token_account`; `DepositRecord` and emitted hash use **net**; `token_mapping.accrued_fees += fee` (INV-D1, fee accounting).
- **Mint/burn:** If `fee > 0`, `transfer_checked` fee to bridge ATA; `burn` **net** from depositor; execute path mints normalized net later ([`withdraw_execute.rs`](../packages/contracts-solana/programs/cl8y-bridge/src/instructions/withdraw_execute.rs)).
- **Token-2022 / `token_interface`:** Depositor and bridge ATAs use `associated_token::token_program = token_program` so Anchor derives the correct ATA for Token-2022 vs classic SPL (INV-D3 supported subset: plain mint; rebasing / transfer-fee extensions out of scope — operational).

### 1.2 `withdraw_execute` (SPL) ([`withdraw_execute.rs`](../packages/contracts-solana/programs/cl8y-bridge/src/instructions/withdraw_execute.rs))

- **Gate:** paused, cancelled, approved, not executed, `dest_account == recipient`, `pw.token == mint.key()` (binds mint to pending record).
- **Hash:** Recomputes `compute_transfer_hash` from stored fields; must equal `pending_withdraw.transfer_hash` (INV-W2).
- **Delay:** `Clock::unix_timestamp > approved_at + withdraw_delay` (INV-W3).
- **Decimals:** `normalize_decimals` (Rust proptest vs EVM reference in `decimal.rs`).
- **Rate limit:** `rate_limit::check_and_update_withdraw_rate_limit` using mint supply and PDA config.
- **CPI:** `LockUnlock` → `transfer_checked` from bridge ATA; `MintBurn` → `mint_to` to recipient.

### 1.3 `withdraw_fees` (SPL branch) ([`withdraw_fees.rs`](../packages/contracts-solana/programs/cl8y-bridge/src/instructions/withdraw_fees.rs))

- **Admin-only** (`admin == bridge.admin`).
- SPL path: optional accounts validated; `token_mapping` PDA seeds checked; `bridge_token_account.owner == bridge`, mint alignment; `accrued_fees >= amount`; `transfer_checked` then decrement `accrued_fees`.

---

## 2. Twenty attack / abuse classes — evidence and fuzz column

“**Fuzz**” = libFuzzer (`cargo-fuzz`) or in-crate **proptest** on the host. **Integration** = Anchor TypeScript tests (including randomized loops in `full_security_audit.test.ts`).

| # | Class | Primary evidence | Fuzz (libFuzzer / proptest) |
|---|--------|------------------|-----------------------------|
| 1 | Wrong V2 transfer hash / parity | `hash_parity.test.ts` (incl. golden vectors); `full_security_audit.test.ts` “FUZZ: transfer hash collision resistance”; `e2e` `test_solana_flows.rs`; `hash.rs` unit vectors + `proptest_matches_tiny_keccak_reference` | Partial — hash proptest + `cargo-fuzz transfer_hash` |
| 2 | Execute with tampered / wrong recomputed hash | `full_security_audit.test.ts` “hash re-verification at execution time prevents tampered PW data”; `withdraw_execute` constraints | No |
| 3 | Double execution / replay same hash | `security_audit.test.ts` §“4. replay…”; `full_security_audit.test.ts` “ATTACK: replay and double-spend prevention”; `hardening.test.ts` “double execute after success” | No |
| 4 | SPL execute wrong mint / mapping | `spl_security.test.ts` “rejects … wrong-mint …”; `security_audit.test.ts` “rejects SPL deposit with mismatched mint vs token_mapping.local_mint” | No |
| 5 | Execute to wrong recipient | `spl_security.test.ts` bad-path block (`WrongRecipient`) | No |
| 6 | Execute before delay | `spl_security.test.ts` (`DelayNotElapsed`); `full_security_audit.test.ts` state machine | No |
| 7 | Execute without approve | `spl_security.test.ts` (`NotApproved`); `full_security_audit.test.ts` “execute before approval…” | No |
| 8 | Execute after cancel | `spl_security.test.ts` (`WithdrawalCancelled`) | No |
| 9 | SPL deposit mint ≠ `local_mint` | `security_audit.test.ts` “rejects SPL deposit with mismatched mint…” | No |
| 10 | `withdraw_fees` > accrued (SPL/native) | `spl_security.test.ts` (`InsufficientAccruedFees`); `full_security_audit.test.ts` “ATTACK: fee draining…” | No |
| 11 | Non-admin `withdraw_fees` | `spl_security.test.ts` “rejects withdraw_fees from non-admin” (`UnauthorizedAdmin`) | No |
| 12 | Fee vs principal (lock escrow) | `spl_security.test.ts` “keeps lock/unlock SPL fees separate from escrow and executes full SPL flow” | No |
| 13 | Mint/burn supply consistency | `spl_security.test.ts` “handles mint/burn SPL deposits…”; `full_security_audit.test.ts` “FUZZ: SPL balance accounting…”, “18. balance accounting invariants for SPL” | No |
| 14 | Decimal normalize vs EVM | `decimal.rs` `prop_matches_evm_reference`; integration paths with mixed decimals | Partial — proptest `normalize_decimals` |
| 15 | Rate limit on SPL execute | `rate_limit_integration.test.ts` (admin PDA layout); Rust `rate_limit.rs` unit tests; execute embeds `check_and_update_withdraw_rate_limit` | No (Rust unit, not libFuzzer) |
| 16 | Paused bridge (deposit SPL / submit / execute SPL) | `security_audit.test.ts` “blocks deposit_spl when paused”, “blocks withdraw_execute (SPL) when paused”; `spl_security.test.ts` “rejects withdraw_submit when bridge is paused” | No |
| 17 | Token-2022 program + ATA correctness | `token_2022_flow.test.ts` “depositSpl and withdrawExecute use Token-2022 program id…” (after `associated_token::token_program` fix) | No |
| 18 | Redirect withdrawal / theft | `full_security_audit.test.ts` “attacker cannot redirect SPL withdrawal…”, “cross-user withdrawal interception” | No |
| 19 | Rebasing / transfer-fee tokens | Documented **unsupported** — INV-D3; governance must not register | N/A |
| 20 | Fee + net arithmetic | `fee.rs` proptest: `fee_plus_net_equals_gross` (bps 0..=10000), `onchain_bps_cap_invariants` (bps 0..=100); `deposit_native` / `deposit_spl` call shared `deposit_fee_and_net` | **Yes (proptest)** |

**Summary:** Most rows are **integration-test** (or dedicated TS “FUZZ” loops), not libFuzzer on instruction dispatch. Hash and fee formulas have **host proptest**; `cargo-fuzz` covers **hash packing only**.

---

## 3. Command evidence (this repo, 2026-04-01)

| Command | Result |
|---------|--------|
| `cd packages/contracts-solana/programs/cl8y-bridge && cargo test` | **21 passed** (includes `fee::tests`, `hash` proptest, `decimal` proptest, `rate_limit` tests) |
| `cd packages/contracts-solana && anchor test` | **185 passing** (full TS suites + programs) |
| `cd …/cl8y-bridge/fuzz && cargo +nightly fuzz run transfer_hash -- -runs=5000` | **Done 5000 runs** (no crash; nightly required for sanitizer) |

`cargo fuzz` on **stable** fails without nightly (`-Z sanitizer`); use `cargo +nightly fuzz run …`.

---

## 4. Governance appendix (trust model)

Aligned with [SOLANA_BRIDGE_INVARIANTS.md](SOLANA_BRIDGE_INVARIANTS.md):

- **Admin:** May withdraw **accrued** fees only (SPL and native checks); cannot pull user principal beyond fee accounting in tested paths. Compromised admin is a **governance** risk, not an unauthenticated on-chain theft vector.
- **Operator:** May approve withdrawals; incorrect approval is mitigated by **canceler** workflow (see `cancel_flow`, `cancel_blocks_theft` tests). Collusion operator + no active canceler is **outside** program-enforced safety.
- **Canceler:** Permissioned list; unauthorized cancel rejected in tests.
- **Token policy (INV-D3):** Rebasing and transfer-fee / tax tokens are **unsupported**; registering them is an operational error.

---

## 5. Fixes applied during this audit

1. **`fee` module + proptest** — Shared `deposit_fee_and_net` for `deposit_native` / `deposit_spl`; property tests for fee/net identities.
2. **Token-2022 Anchor constraints** — `associated_token::token_program = token_program` on `deposit_spl` and `withdraw_execute` token accounts so ATAs match `TOKEN_2022_PROGRAM_ID`.
3. **`token_2022_flow.test.ts`** — Admin airdrop before setup; idempotent ATA creation (clearer errors); **correct post-execute bridge balance** = remaining **fee** (not zero) for lock/unlock.
4. **`Anchor.toml`** — Clone `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` for local validator parity with mainnet Token-2022.

---

## 6. Residual gaps (honest)

- **libFuzzer** does not drive Anchor instruction dispatch, CPI, or Sysvar surfaces ([SOLANA_FUZZING.md](SOLANA_FUZZING.md) “Future work”).
- **TS “FUZZ”** blocks in `full_security_audit.test.ts` are **randomized integration** checks, not LLVM coverage-guided fuzzing.
- **Instruction-level** structured fuzzing remains future work if stakeholders require it for SPL CPI edges.
