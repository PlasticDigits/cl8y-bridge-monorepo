# Operator EVM writer invariants (GL-138)

Cross-links: [operator.md](./operator.md), [architecture.md](./architecture.md), [testing.md](./testing.md), [deployment-guide.md](./deployment-guide.md), [`skills/agent-operator-evm-writer-rpc.md`](../skills/agent-operator-evm-writer-rpc.md), Forgejo issues **138** and **170**.

These rules apply to the operator **EVM writer** event poll, RPC fallback, and pending-withdrawal enumeration. They do **not** relax source-chain verification. Companion Terra history work is GitLab **139**.

## INV-OP-W1 — Fail closed on source verification

Never call `withdrawApprove` unless source-chain verification succeeds against the **configured** chain and bridge (`verify_deposit_on_source`). Unknown source chain IDs return `false` (no approval). Method-level RPC fallback must not skip chain-ID checks performed at startup (`verify_evm_rpc_chain_ids`).

## INV-OP-W2 — No cursor skip

`EventPollCursor` advances only through the last **contiguous successful** `eth_getLogs` chunk. A failed range is retried before later ranges. Partial success (chunk 1 ok, chunk 2 fail) leaves the cursor at chunk 1.

The first-poll lookback start is **sticky**. Recomputing `head - EVM_POLL_LOOKBACK_BLOCKS` after an initial-chunk failure would skip blocks and livelock at `last_polled_block == 0`.

## INV-OP-W3 — Method-level RPC fallback

Selecting an endpoint with `eth_blockNumber` is not proof that `eth_getLogs` will work. Each log chunk is attempted against remaining validated URLs (`rpc_fallback::with_endpoint_fallback` / `get_logs_with_endpoint_fallback`). The watcher and writer share this helper.

A successful log query — **including empty logs** — is not treated as an observed range until `eth_chainId` on that endpoint matches the configured native chain ID (`confirm_rpc_chain_id`). Empty fallback logs from a wrong-chain or unverified endpoint must not advance `EventPollCursor`.

## INV-OP-W4 — Bounded negative verification retry

Unapproved destination hashes without a visible source deposit use a size- and TTL-capped retry schedule (`NegativeVerifySchedule`). A later-valid deposit is retried after TTL/backoff expiry, or immediately when a new `WithdrawSubmit` is observed. Approved, cancelled, and executed hashes are evicted. Per-cycle verify count is capped (`WRITER_MAX_VERIFY_PER_CYCLE`) and **shared** by contract enumeration and the event-poll path (`CycleVerifyBudget`). Under size pressure the cache evicts by earliest `inserted_at` (FIFO), not earliest `next_retry`.

## INV-OP-W5 — Chain isolation

Each EVM writer and the Terra writer run on their **own** interval and backoff. A degraded chain must not `sleep` the shared manager. Shutdown sets a watch flag and waits at most 5s for loops to exit.

## INV-OP-W6 — Enumeration remains the safety net

Contract `getPendingWithdrawHashes` still runs each cycle (subject to per-hash backoff). Event polling is secondary. Do not remove enumeration without an equivalent durable discovery path.

## INV-OP-W7 — Jittered capped backoff

RPC and negative-retry delays use capped exponential backoff with jitter (`WRITER_BACKOFF_JITTER_BPS`). Call-site seeds (chain id, chunk start, hash) are mixed with **process-start entropy** (`process_jitter_entropy`) so co-scheduled operators do not retry in lockstep. Do not seed jitter from `Instant::now().elapsed()` on a just-created `Instant` (that value is ~0).

## INV-OP-W8 — Validated configuration

Lookback, chunk, interval, backoff, cache size, and TTL are parsed once at startup. Zero, overflow, and out-of-range values are **rejected** (not silently clamped). Bounds:

| Variable | Default | Min | Max |
|----------|---------|-----|-----|
| `WRITER_POLL_INTERVAL_MS` | 5000 | 200 | 120000 |
| `EVM_POLL_LOOKBACK_BLOCKS` | 5000 | 1 | 100000 |
| `EVM_POLL_CHUNK_SIZE` | 5000 | 1 | 50000 |
| `WRITER_RPC_BACKOFF_INITIAL_MS` | 2000 | 100 | 60000 |
| `WRITER_RPC_BACKOFF_MAX_MS` | 60000 | 1000 | 600000 |
| `WRITER_BACKOFF_JITTER_BPS` | 1500 | 0 | 5000 |
| `WRITER_NEGATIVE_RETRY_INITIAL_MS` | 5000 | 100 | 60000 |
| `WRITER_NEGATIVE_RETRY_MAX_MS` | 300000 | 1000 | 3600000 |
| `WRITER_NEGATIVE_RETRY_TTL_SECS` | 86400 | 60 | 604800 |
| `WRITER_NEGATIVE_RETRY_CACHE_SIZE` | 10000 | 16 | 100000 |
| `WRITER_MAX_VERIFY_PER_CYCLE` | 64 | 1 | 10000 |
| `APPROVED_HASH_CACHE_SIZE` | 100000 | 16 | 2000000 |
| `PENDING_EXECUTION_CACHE_SIZE` | 50000 | 16 | 2000000 |
| `HASH_CACHE_TTL_SECS` | 86400 | 60 | 604800 |

## INV-OP-W9 — No secret leakage

Metrics and logs must not include RPC credentials, URL query tokens, path API keys, private keys, database URLs, or signed payloads. RPC endpoints in logs use `sanitize_rpc_endpoint` (`scheme://host[:port]` only — userinfo, query, and **path** are dropped because Alchemy `/v2/<key>` and Infura `/v3/<project>` put credentials in the path). Error `Display` strings (hyper/reqwest) are passed through `sanitize_rpc_error` so embedded URLs cannot leak. Metric labels are `evm-<nativeChainId>` plus method name.

## INV-OP-W10 — Production log default is `info`

When `RUST_LOG` is unset, the operator defaults to `info` (not `cl8y_operator=debug`). Repeated no-progress verification is `debug`. Approvals and first-poll / fallback **state changes** stay at `info`.

## INV-OP-W11 — Execute approved pending withdrawals

Hashes that are **approved, not cancelled, not executed** must be queued for `withdrawExecuteUnlock` / `withdrawExecuteMint` (Terra: unlock/mint) after the cancel window. Enumeration (`getPendingWithdrawHashes`, Terra `active_withdrawals`) is the durable recovery path: do **not** skip these hashes solely because they are already approved or present in `approved_hashes`.

`pending_executions` is process-local and TTL-bounded. After restart or cache eviction the writer must re-queue from on-chain state. Do not reset an existing execute timer on each poll (that livelocks execute). Source verification is **not** required on this path (approval already verified). Do not consume `WRITER_MAX_VERIFY_PER_CYCLE` for execute re-queue.

Executed, cancelled, missing (`submittedAt == 0`), and `BelowMinPerTransaction` / Terra `BelowMinimumAmount` are **terminal**. Drop them from `pending_executions` and record them in `terminal_executions` so enumeration does not re-queue. RPC / `CancelWindowActive` / period `RateLimitExceededPerPeriod` failures stay retryable — **do not** add period-full to the terminal list (GL-170). Retryable failures increment `attempts` and apply backoff from *now* (`execute_queue`); do **not** reset an in-flight timer on enumeration re-queue.

## INV-OP-W12 — Execute send/receipt method-level RPC fallback (GL-170)

`submit_execute_withdraw` must not bind send or receipt to the single `rpc_url` that answered `eth_chainId`. Reads (`getPendingWithdraw`) use [`with_endpoint_fallback`]. Send uses [`with_retryable_rpc_fallback`] so **contract reverts** (`CancelWindowActive`, period-limit, BelowMin) are not re-broadcast on every URL. After a successful send, receipt is fetched **by hash** on remaining URLs; do not wrap send+receipt in one fallback (that would re-send). Mined `status=false` receipts are re-simulated (`eth_call`) so BelowMin selectors stay terminal.

On-chain `TokenRateLimit` / min-per-tx stay unchanged. No operator execute bypass.

## Code map

| Concern | Location |
|---------|----------|
| Shared log fallback + chain-id confirm | `packages/operator/src/rpc_fallback.rs` |
| Execute send/receipt fallback | `packages/operator/src/rpc_fallback.rs` (`with_retryable_rpc_fallback`) |
| Execute queue (enqueue, retry backoff) | `packages/operator/src/writers/execute_queue.rs` |
| URL / error sanitization | `packages/multichain-rs/src/evm/rpc_fallback.rs` (`sanitize_rpc_endpoint`, `sanitize_rpc_error`) |
| Retryable RPC classification | `packages/multichain-rs/src/evm/rpc_fallback.rs` |
| Cursor | `packages/operator/src/writers/poll_cursor.rs` |
| Negative retry | `packages/operator/src/writers/negative_retry.rs` |
| Config bounds | `packages/operator/src/poll_config.rs` |
| Isolated loops | `packages/operator/src/writers/mod.rs` |
| EVM poll + enumerate + execute re-queue | `packages/operator/src/writers/evm.rs` |
| Watcher `eth_getLogs` | `packages/operator/src/watchers/evm.rs` |
