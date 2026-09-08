# Skill: Terra Classic active withdrawals (agents / automation)

When changing Terra Classic withdrawal storage, list queries, migrate, operator Terra polling, or canceler Terra polling, preserve **INV-TC-AW1–AW5** in [`docs/TERRACLASSIC_BRIDGE_INVARIANTS.md`](../docs/TERRACLASSIC_BRIDGE_INVARIANTS.md) (issue **139**).

## Do not regress

1. **Canonical history stays** — Never delete `PENDING_WITHDRAWS` rows on execute or cancel. Replay, single-hash status, and `WithdrawUncancel` depend on them.
2. **Active index is membership-only** — `ACTIVE_WITHDRAW_HASHES` contains a hash **iff** the canonical row exists and `!executed && !cancelled`. Update both in the same execute via `save_pending_and_sync_index`.
3. **Do not silently filter `pending_withdrawals`** — That query remains all-status. `next_start_after` is an additive cursor only (INV-TC-AW5). Add or use `active_withdrawals` for operator/canceler work.
4. **Do not scan `PENDING_WITHDRAWS` to implement `active_withdrawals`** — Range the index. Skip orphan/terminal index keys (`inconsistent_skipped`); do not panic. Cap total keys visited (`limit + MAX_ACTIVE_QUERY_SKIPS`). Return `next_start_after` so a short skip-capped page can continue.
5. **Migrate is batched and version-aware** — Reconstruction must be resumable (`active_index_batch_limit`, default 50, max 100). Migrating from a wasm that does **not** maintain the index (`< 2.1.0`, including rollback to 2.0.0 then re-upgrade) **must reset** leftover `complete=true` and rebuild (insert active, remove terminal leftovers). Incomplete index → `active_withdrawals` errors → clients fall back to `pending_withdrawals`.
6. **Same code_id continue / emergency rebuild** — Live columbus-5 (terrad 4.0.1 / wasmd v0.61.8) **allows** same-`code_id` `MsgMigrateContract` and CosmWasm 1.5 still invokes `migrate` (`ContractMigrateVersion` is nil). Repeat wasm migrate until `active_index_complete=true`. Keep admin `ContinueActiveIndexMigrate` if a future chain upgrade rejects same-code migrate. Emergency rebuild: `rebuild: true` on the **first** call only, then `rebuild: false` until complete. This is **not** a privileged wipe of canonical rows.
7. **No privileged wipe** — Do not add an admin “delete pending withdrawals” path.
8. **Frontend status** — Hash monitor historical listing stays on `pending_withdrawals`; per-hash status stays on `pending_withdraw`. Page size **30** (contract cap); follow `next_start_after`. Do not switch the monitor to `active_withdrawals` or completed transfers vanish (**INV-FE-TC-AW1**). Requesting 50 and treating `len < 50` as EOF drops history after the first page.
9. **Operator/canceler** — Prefer `active_withdrawals` and `next_start_after`. Clamp list page size to 30. Keep a legacy fallback. On fallback, **clear** accumulated cancel candidates before the legacy walk. Log cycle summaries not per-terminal-entry debug.

## Where it lives

- Index + invariant: `packages/contracts-terraclassic/bridge/src/state.rs`, `active_withdraw.rs`
- Lifecycle: `packages/contracts-terraclassic/bridge/src/execute/withdraw.rs`
- Queries: `packages/contracts-terraclassic/bridge/src/query.rs`, `msg.rs`
- Migrate: `packages/contracts-terraclassic/bridge/src/contract.rs` (`migrate` + `ExecuteMsg::ContinueActiveIndexMigrate`)
- Operator: `packages/operator/src/writers/terra.rs`, `terra_list.rs`
- Canceler: `packages/canceler/src/watcher.rs`, `terra_withdraw_list.rs`
- Client types: `packages/multichain-rs/src/terra/contracts.rs`
- Frontend: `packages/frontend/src/services/hashMonitor.ts` (`hashMonitor.test.ts`)
- Tests: `tests/test_withdraw_flow.rs` (`test_active_index_*`, `test_continue_active_index_migrate_*`), `tests/test_active_index_migrate.rs`, `tests/test_active_index_scale.rs`, `tests/proptest_active_withdraw.rs`

## Related docs

- [`docs/TERRACLASSIC_BRIDGE_INVARIANTS.md`](../docs/TERRACLASSIC_BRIDGE_INVARIANTS.md)
- [`docs/contracts-terraclassic.md`](../docs/contracts-terraclassic.md)
- [`docs/deployment-terraclassic-upgrade.md`](../docs/deployment-terraclassic-upgrade.md) (v2.1 migrate loop, rollback+re-upgrade, emergency rebuild)
- [`docs/FRONTEND_BRIDGE_INVARIANTS.md`](../docs/FRONTEND_BRIDGE_INVARIANTS.md) (**INV-FE-TC-AW1**)
- Companion operator RPC livelock: issue **138** (not this skill)

## Tracking issues

- issue **139** — stop pending-withdrawal polling from scaling with terminal history. LCD-sized migrate batch counts and client pagination are evidenced; keep **open** until the first on-chain v2.1 `wasm migrate` records `gas_used` and a post-deploy operator/canceler soak. Do not `Closes #139` from client/docs-only changes.
