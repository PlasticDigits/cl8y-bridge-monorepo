# Terra Classic bridge invariants

Cross-links: [contracts-terraclassic.md](./contracts-terraclassic.md), [deployment-terraclassic-upgrade.md](./deployment-terraclassic-upgrade.md), [`packages/contracts-terraclassic/docs/OPERATIONAL_NOTES.md`](../packages/contracts-terraclassic/docs/OPERATIONAL_NOTES.md), [`skills/agent-terraclassic-active-withdrawals.md`](../skills/agent-terraclassic-active-withdrawals.md), issue **139**.

These invariants apply to `packages/contracts-terraclassic/bridge/` and to operator/canceler Terra polling. They do **not** change EVM or Solana pending-hash sets.

## INV-TC-AW1 — Canonical record vs active index (GL-139)

`PENDING_WITHDRAWS` is the **canonical** by-hash withdrawal record. `ACTIVE_WITHDRAW_HASHES` is a **membership index** of non-terminal rows.

| Rule | Behavior |
|------|----------|
| **Membership** | Hash is in `ACTIVE_WITHDRAW_HASHES` **iff** `PENDING_WITHDRAWS[hash]` exists and `!executed && !cancelled`. |
| **Submit** | Inserts the canonical row and the index key. Duplicate hash is rejected while the canonical row exists (including after execute). |
| **Approve** | Updates the canonical row; membership stays. |
| **Cancel** | Sets `cancelled`; **removes** the index key. Canonical row remains so authorized `WithdrawUncancel` can restore it. |
| **Uncancel** | Clears `cancelled`, resets `approved_at`, **reinserts** the index key. |
| **Execute** (unlock or mint) | Sets `executed`; **removes** the index key. Canonical row remains for status and replay. |
| **Nonce replay** | `(src_chain, nonce)` stays rejected via `WITHDRAW_NONCE_USED` after approval, independent of the index. |
| **Atomicity** | Canonical write and index insert/remove happen in the same execute message (`save_pending_and_sync_index`). A later `Err` rolls both back. |
| **Fail closed** | `ActiveWithdrawals` skips index keys whose canonical row is missing or terminal (`inconsistent_skipped`). Scan work is capped at `limit + MAX_ACTIVE_QUERY_SKIPS` (64 extra keys). A short page with `next_start_after` set is not exhausted. The query does not panic and does not approve/execute from the index alone. Execute/approve still load the canonical row. |
| **No privileged delete** | There is no admin path that erases an actionable canonical row or its replay evidence. `ContinueActiveIndexMigrate` only rebuilds the **index** from canonical history. |

| Evidence | Location |
|----------|----------|
| Index + migration state | `packages/contracts-terraclassic/bridge/src/state.rs` |
| Helpers | `packages/contracts-terraclassic/bridge/src/active_withdraw.rs` |
| Lifecycle | `packages/contracts-terraclassic/bridge/src/execute/withdraw.rs` |
| Queries | `packages/contracts-terraclassic/bridge/src/query.rs` (`ActiveWithdrawals`, `PendingWithdrawals`, `PendingWithdraw`) |
| Unit + property tests | `active_withdraw` module tests, `tests/proptest_active_withdraw.rs` |
| Integration | `tests/test_withdraw_flow.rs` (`test_active_index_*`, `test_continue_active_index_migrate_*`) |
| Migrate entry | `tests/test_active_index_migrate.rs` (`contract::migrate` + cw-multi-test wiring) |
| Scale + LCD-sized migrate | `tests/test_active_index_scale.rs` (106-row production mix; 2000-terminal active query) |

## INV-TC-AW2 — Query compatibility

| Query | Semantics | Who should use it |
|-------|-----------|-------------------|
| `pending_withdraw` | Single-hash status, including executed/cancelled/missing (`exists: false`) | Frontend transfer status, scripts, replay checks |
| `pending_withdrawals` | **All-status** historical pagination over `PENDING_WITHDRAWS`. **Unchanged** row semantics in v2.1. Additive `next_start_after` cursor (INV-TC-AW5) | Hash monitor historical discovery, audits. **Not** operator/canceler steady-state polling |
| `active_withdrawals` | Pagination over `ACTIVE_WITHDRAW_HASHES` only. Errors if migrate reconstruction is incomplete. Scan-capped; continue via `next_start_after` | Operator, canceler, execution watchers |
| `active_withdraw_index` | `active_count` + migration progress | Ops, health, migrate loops |

Do **not** silently change `pending_withdrawals` to filter terminal rows. That would break clients that enumerate history.

## INV-TC-AW3 — Bounded, resumable migrate

v2.0.0 → v2.1.0 reconstructs `ACTIVE_WITHDRAW_HASHES` from `PENDING_WITHDRAWS`.

| Rule | Behavior |
|------|----------|
| **Batch** | Each `migrate` / `ContinueActiveIndexMigrate` scans at most 100 canonical rows (default 50). Repeat until attribute `active_index_complete=true`. |
| **Version reset** | If the previous cw2 version does **not** maintain the index (`< 2.1.0`, including 2.0.0 after a code rollback), reconstruction is reset (`complete=false`, `last_key=None`) even if leftover `complete=true` remains. Terminal leftovers are **removed** from the index as rows are scanned. |
| **Idempotent (same 2.1.x)** | A completed migrate is a no-op while 2.1.x stays installed. Resume continues from `last_key`. Re-insert of an already-indexed active hash does not double-count. |
| **Instantiate** | New contracts mark migration complete with an empty index. |
| **Incomplete index** | `active_withdrawals` errors so operator/canceler **fall back** to `pending_withdrawals` (watchtower must not miss approved rows). |
| **Same code_id continue** | Live columbus-5 (`terrad` 4.0.1, wasmd **v0.61.8**, LCD 2026-09-01) does **not** reject `MsgMigrateContract` when `new_code_id == current`. CosmWasm 1.5 wasm reports `ContractMigrateVersion == nil`, so the contract `migrate` entrypoint **is** invoked on a same-`code_id` call. Repeat `wasm migrate` to the stored 2.1 code_id until `active_index_complete=true`. Keep admin `ContinueActiveIndexMigrate { rebuild: false }` if a future chain upgrade reintroduces same-code rejection, or if operators prefer execute over migrate. |
| **Emergency rebuild** | Admin `ContinueActiveIndexMigrate { rebuild: true }` on the **first** call only, then `rebuild: false` until complete. Use this if canonical-active rows are missing from the index (holes) without a version downgrade. Does **not** delete canonical rows. |
| **Rollback** | Rolling the wasm back to v2.0.0 leaves the extra maps unused; `pending_withdrawals` still works. v2.0.0 `migrate` writes cw2 version `2.0.0`, so a later re-upgrade to 2.1.x **rebuilds** (see Version reset). New binaries on old code must keep the legacy fallback. |

`MigrateMsg` remains `{}`-compatible. Optional `active_index_batch_limit` overrides the per-call scan size.

**Migrate gas evidence (not a live broadcast):** columbus-5 LCD 2026-09-01 listed **106** `PENDING_WITHDRAWS` rows (95 executed, 11 approved-not-executed) on still-v2.0 code_id `10971`. Reconstruction is 3 batches of 50 or 2 of 100 (`test_active_index_scale.rs`). Live wasm **execute** txs on the same contract that day used ~158–257k gas (`gas_wanted` 500k). Per-row JSON is under 512 bytes, so a 100-row index batch is a bounded storage walk — record the actual `gas_used` of the first v2.1 `wasm migrate` at deploy time.

## INV-TC-AW4 — Operator / canceler polling

| Rule | Behavior |
|------|----------|
| **Prefer active** | Query `active_withdrawals` first. |
| **Fallback** | On unknown variant, LCD error, missing `withdrawals` array, or incomplete migrate, use `pending_withdrawals` for that cycle. Canceler **clears** accumulated candidates (`all_approvals`, skip counters) before the legacy walk so a mid-pagination fallback cannot duplicate cancel work. |
| **Pagination** | Prefer `next_start_after`. Clamp requested page size to **30** (`WITHDRAW_LIST_MAX_LIMIT`). A short or empty page with that cursor set is not exhausted (skip cap). Do **not** request 50 and treat `len < 50` as EOF — the contract returns at most 30 rows. |
| **Canceler filter** | Only **approved && !cancelled && !executed** rows are cancel candidates. Unapproved active rows are skipped. |
| **Operator filter** | Only **unapproved && !cancelled && !executed** rows are approval candidates. |
| **Logs** | Cycle summaries and metrics; no per-entry debug for terminal history. |
| **Metrics** | `relayer_terra_withdraw_query_mode`, `relayer_terra_active_withdrawals_polled`, `relayer_terra_inconsistent_skipped_total`; canceler `canceler_terra_withdraw_query_mode`, `canceler_terra_inconsistent_skipped`. No RPC credentials or addresses in metric labels. |

Unapproved spam can still grow the **active** set; that is bounded operator-retry work tracked separately in issue **138**, not by deleting canonical rows.

## INV-TC-AW5 — List page cap and cursors

Both `pending_withdrawals` and `active_withdrawals` cap `limit` at **30**. v2.1 adds `next_start_after` on **both** queries (additive JSON; row semantics of `pending_withdrawals` unchanged).

| Client | Rule |
|--------|------|
| Operator | Page size 30. Prefer `next_start_after`. Soak: 106-row production mix is 4 legacy pages / 1 active page (`writers/terra_list.rs`). |
| Canceler | Default and clamp `TERRA_POLL_PAGE_SIZE` to 30. Same cursor rule (`terra_withdraw_list.rs`). |
| Frontend hash monitor | `TERRA_PAGE_SIZE = 30`, query `pending_withdrawals` only, follow `next_start_after` so executed rows beyond the first page remain listed (**INV-FE-TC-AW1**). |

## Frontend

Transfer-status hash discovery in `hashMonitor.ts` keeps `pending_withdrawals` so executed/cancelled hashes remain listed (bounded by page cap). Per-hash status uses `pending_withdraw`. See [FRONTEND_BRIDGE_INVARIANTS.md](./FRONTEND_BRIDGE_INVARIANTS.md) **INV-FE-TC-AW1**.
