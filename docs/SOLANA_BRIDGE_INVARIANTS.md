# Solana bridge invariants and test evidence

This document lists security and correctness invariants for the Anchor programs under `packages/contracts-solana/programs/`, with cross-links to automated tests. Governance (admin, operator, canceler keys) is assumed honest unless noted.

**Reference branch for EVM / TerraClassic hash specs:** `main` (there is no `master` branch in this repo). As of the Solana integration work:

- `packages/contracts-evm/src/lib/HashLib.sol` — **no diff** vs `main` for V2 `computeXchainHashId`.
- `packages/multichain-rs/src/hash.rs` — V2 `compute_xchain_hash_id` layout **unchanged** vs `main` (minor test / style deltas only).
- `packages/contracts-terraclassic/bridge/src/hash.rs` — V2 path **unchanged** vs `main` (trivial `div_ceil` refactor in legacy helpers only).
- `packages/contracts-solana/programs/cl8y-bridge/src/hash.rs` — **new**; must stay byte-identical in layout to the three references above (enforced by golden vectors from `HashLib.t.sol` + proptest vs `tiny-keccak` in-crate).

---

## Hash and cross-chain identity

### INV-H1 — V2 transfer digest parity

The 32-byte transfer id is `keccak256` over 224 bytes: seven 32-byte ABI words (`bytes32(srcChain)`, `bytes32(destChain)`, `srcAccount`, `destAccount`, `token`, `uint256(amount)`, `uint256(nonce)`), with `amount` as u128 big-endian in the **low** 16 bytes of the amount word and `nonce` as u64 big-endian in the **low** 8 bytes of the nonce word.

| Evidence | Location |
|----------|----------|
| On-chain Solana | `programs/cl8y-bridge/src/hash.rs` (`compute_transfer_hash`), unit tests + proptest vs `tiny-keccak` reference |
| Solidity goldens | `packages/contracts-evm/test/HashLib.t.sol` (`test_DepositWithdraw_*` expected digests) |
| TypeScript | `packages/contracts-solana/tests/hash_parity.test.ts` |
| Frontend (canonical) | `packages/frontend/src/services/hashVerification.ts` (`computeXchainHashId` via viem `encodeAbiParameters`) |
| E2E offline | `packages/e2e/tests/test_solana_flows.rs` (`test_hash_parity_offchain`, `test_hash_goldens_match_hashlib_t_sol` via `multichain_rs::hash`) |
| Narrative | `docs/crosschain-parity.md` |

**Amount range:** Solana instructions use `u128` for amounts; EVM uses `uint256`. Hashes agree only while the amount fits in u128 and the high 16 bytes of the ABI amount word are zero (normal bridge amounts).

### INV-H2 — Destination token in hash

The `token` word is always the **destination** chain representation (mint pubkey bytes on Solana dest, ERC-20/CW20/native encoding per `crosschain-parity.md`).

| Evidence | Narrative + registry tests; `deposit_*` / `withdraw_submit` handlers bind `TokenMapping` to that convention |

---

## Withdrawal lifecycle

### INV-W1 — Pending withdraw bound to transfer hash

`PendingWithdraw` PDA seeds include the transfer hash; `withdraw_submit` rejects if `ExecutedHash` already exists for that hash.

| Evidence | `packages/contracts-solana/tests/deposit_withdraw.test.ts`, `spl_security.test.ts`, `hardening.test.ts` |

### INV-W2 — Execute recomputes hash

`withdraw_execute` / `withdraw_execute_native` recompute `compute_transfer_hash` from stored fields and require equality with `pending_withdraw.transfer_hash`.

| Evidence | `programs/cl8y-bridge/src/instructions/withdraw_execute.rs`; integration tests in `deposit_withdraw.test.ts`, `security_audit.test.ts` |

### INV-W3 — State machine

Approved, non-cancelled, non-executed invariants for approve/cancel/execute; delay enforced after approve.

| Evidence | `cancel_flow.test.ts`, `cancel_blocks_theft.test.ts`, `rate_limit_integration.test.ts`, `full_security_audit.test.ts` |

---

## Deposits and SPL custody

### INV-D1 — Mint and mapping consistency

Deposits enforce `TokenMapping` / mint alignment; SPL uses `transfer_checked` with decimals from registry flow.

| Evidence | `spl_security.test.ts`, `deposit_withdraw.test.ts`, `bridge.test.ts` |

### INV-D2 — Paused bridge

While `bridge.paused`, user-facing deposit and withdraw paths reject.

| Evidence | `security_audit.test.ts`, `hardening.test.ts` |

### INV-D3 — Token-2022 support scope (plain mint only)

SPL paths use `anchor_spl::token_interface` (`transfer_checked`, etc.), so **plain** Token-2022 mints (same 1:1 transfer semantics as classic SPL) are supported. The following are **explicitly not supported**; operators must not register such mints for bridge custody:

- **Rebasing tokens** — balances or total supply that change outside normal user-initiated transfers (e.g. interest-bearing / rebase-style mechanics). The bridge assumes instructed amounts match `transfer_checked` and bookkeeping.
- **Transfer tax / transfer-fee / hook-style tokens** — where the amount received by the recipient or debited from the sender does not equal the instructed transfer amount (Token-2022 transfer-fee extensions and similar). Bridge accounting assumes the declared amount equals tokens moved in/out of bridge ATAs.

| Evidence | `packages/contracts-solana/tests/token_2022_flow.test.ts` (plain Token-2022 lock/unlock deposit → withdraw); narrative above |

---

## Faucet (separate program)

### INV-F1 — No bridge hash

`cl8y-faucet` does not use the V2 xchain hash; claims are cooldown + SPL mint rules only.

| Evidence | `packages/contracts-solana/tests/faucet.test.ts` |

---

## Attack-class ↔ test coverage matrix

Rows are review categories; cells name primary test files that exercise them (not every assertion is listed). Empty cells indicate follow-up coverage worth adding.

| Attack class / invariant | Bridge core | Cancel / theft | SPL / Token | Rate limit | Faucet |
|--------------------------|------------|----------------|-------------|------------|--------|
| Hash parity (INV-H1) | `hash.rs` tests, `hash_parity.test.ts` | — | — | — | — |
| Double execute / replay | `deposit_withdraw.test.ts`, `spl_security.test.ts` | `cancel_flow.test.ts` | — | — | — |
| Wrong mint / token | `spl_security.test.ts`, `bridge.test.ts` | — | ✓ | — | `faucet.test.ts` |
| Unauthorized canceler | — | `cancel_blocks_theft.test.ts`, `cancel_flow.test.ts` | — | — | — |
| Operator / admin gates | `bridge.test.ts`, `security_audit.test.ts` | ✓ | — | `rate_limit_integration.test.ts` | — |
| Paused bridge | `security_audit.test.ts`, `hardening.test.ts` | ✓ | — | — | — |
| Hash / PDA confusion | `hash_parity.test.ts`, `test_solana_flows.rs` | `cancel_blocks_theft.test.ts` | — | — | — |
| **Token-2022 (plain mint; INV-D3)** | `token_2022_flow.test.ts` (`token_interface` + `TOKEN_2022_PROGRAM_ID`) | — | ✓ unsupported: rebasing, transfer-fee/tax — see INV-D3 | — | — |

---

## Frontend hash verification (INV-HFE1)

User-visible and client-built transfer ids must use the same V2 digest as on-chain code. **Canonical implementation:** `packages/frontend/src/services/hashVerification.ts` (`computeXchainHashId`, `computeXchainHashIdFromBytes`). Solana withdraw-submit must delegate to that module (see `packages/frontend/src/services/solana/transaction.ts`).

**Validation helpers:** `packages/frontend/src/utils/validation.ts` (`isValidXchainHashId`, `normalizeXchainHashId`) are for permissive URL/form parsing. `normalizeHash` in `hashVerification.ts` is strict and throws on invalid input—use it when building comparisons to on-chain hex.

---

## Fuzzing

See `docs/SOLANA_FUZZING.md` for `cargo-fuzz` layout and scope (hash packing; **Future work** section for instruction/CPI fuzzing). Property tests today: `hash.rs` (proptest vs `tiny-keccak`), `decimal.rs` (proptest), `packages/multichain-rs/src/hash.rs` (proptest).
