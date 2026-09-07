# Skill: Operator EVM writer RPC / cursor (GL-138)

Use when changing the operator EVM writer poll loop, `eth_getLogs` fallback, pending-withdrawal enumeration, writer backoff, or operator Prometheus metrics related to approvals.

## Sources of truth

1. [`docs/OPERATOR_WRITER_INVARIANTS.md`](../docs/OPERATOR_WRITER_INVARIANTS.md) — **INV-OP-W1** through **INV-OP-W11**
2. [`docs/operator.md`](../docs/operator.md) — operator architecture and env vars
3. GitLab issue **138** — RPC/cursor livelock and stale-withdrawal retry amplification

## Invariants (do not violate)

- **INV-OP-W1:** Never approve without source verification against the configured chain/bridge.
- **INV-OP-W2:** Never advance the event cursor past a failed/unobserved `eth_getLogs` range. First-poll lookback start is sticky.
- **INV-OP-W3:** Retry the **log query itself** on fallback URLs. Do not bind `eth_getLogs` to whichever endpoint first answered `eth_blockNumber`. Do not treat empty fallback logs as observed until `eth_chainId` matches the configured chain.
- **INV-OP-W4:** Negative verification must be TTL-bounded and size-capped; late deposits must become eligible again. `WRITER_MAX_VERIFY_PER_CYCLE` is shared by enumeration **and** the event-poll path. Full-cache eviction is FIFO by `inserted_at`.
- **INV-OP-W5:** Do not `sleep` a shared writer manager on one chain’s RPC failure.
- **INV-OP-W6:** Keep contract enumeration as the durable discovery path.
- **INV-OP-W7:** Capped exponential backoff with jitter mixed with **process-start entropy**; no synchronized retry storms.
- **INV-OP-W8:** Reject invalid interval/lookback/chunk/cache env values at startup.
- **INV-OP-W9:** Never log RPC query tokens, userinfo, **path API keys** (Alchemy `/v2/<key>`, Infura `/v3/<id>`), or DB URLs. Use `sanitize_rpc_endpoint` (`scheme://host[:port]` only) and `sanitize_rpc_error` on error `Display` strings.
- **INV-OP-W10:** Default `RUST_LOG` is `info`; do not re-enable process-wide `cl8y_operator=debug` as the production default.
- **INV-OP-W11:** Approved-not-executed destination hashes must be re-queued for `withdrawExecute*` from enumeration after restart or `pending_executions` TTL eviction. Do not skip them as “already approved.” Do not reset an in-flight execute timer each poll. Do not spend `WRITER_MAX_VERIFY_PER_CYCLE` on this path. Drop executed / cancelled / missing / below-min execute errors from the queue and do not re-queue them (`terminal_executions`).

## Where it lives

- Shared fallback: `packages/operator/src/rpc_fallback.rs` (`log_rpc`, `log_rpc_error`, `confirm_rpc_chain_id`)
- Writer cursor: `packages/operator/src/writers/poll_cursor.rs`
- Negative retry + per-cycle verify budget: `packages/operator/src/writers/negative_retry.rs`
- Writer poll/enumerate: `packages/operator/src/writers/evm.rs`
- Isolated schedules: `packages/operator/src/writers/mod.rs`
- Watcher logs: `packages/operator/src/watchers/evm.rs` (same `get_logs_with_endpoint_fallback`)
- Classification + sanitizers: `multichain_rs::is_retryable_evm_rpc_error`, `sanitize_rpc_endpoint`, `sanitize_rpc_error`

## Tests

```bash
cd packages/operator && cargo test --bins
cd packages/multichain-rs && cargo test rpc_fallback
```

Mock JSON-RPC servers in `rpc_fallback.rs` tests are **transport fixtures** for method-level failover (primary answers `eth_blockNumber` / rate-limits `eth_getLogs`, fallback serves logs, `eth_chainId` is checked, cursor advances once). They are not a substitute for Anvil/LocalTerra transfer tests.

Writer-level livelock coverage: `writer_livelock_primary_blocknumber_ok_logs_429_fallback_advances_cursor_once`. Wrong-chain empty fallback logs: `empty_wrong_chain_fallback_logs_do_not_succeed`.

## Related skills

- [agent-evm-bsc-parity-replay.md](./agent-evm-bsc-parity-replay.md) — unrelated deploy parity; listed for discoverability
- [agent-metamask-blockaid-evm.md](./agent-metamask-blockaid-evm.md) — wallet alerts vs on-chain correctness

## Tracking issues

- GitLab **138** — EVM writer RPC/cursor livelock and stale-withdrawal retry amplification
- GitLab **115** — earlier operator RPC hardening (consensus head); this skill covers residual method-level fallback
- GitLab **139** — Terra withdrawal history (companion; not this skill)
