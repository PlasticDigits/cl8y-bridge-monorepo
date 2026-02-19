# Security Review: `canceler` Package

**Date:** 2026-02-19
**Scope:** `packages/canceler/src/*.rs`
**Method:** Static code review and re-audit of previous findings

## Findings Summary

| ID | Severity | Finding | Status |
|---|---|---|---|
| C1 | High | Verification fails open on source-chain query errors | **Escalated (see C12)** |
| C2 | Medium | Terra approval polling limit without pagination | **Fixed** |
| C3 | Medium | Unbounded in-memory tracking sets | **Fixed** |
| C4 | Low | EVM cancellation attempted after failed pre-check | **Fixed** |
| C5 | Low | Trusted URL model and optional public health bind | **Fixed** |
| C6 | High | Misconfigured Chain IDs trigger mass cancellation | **Fixed** |
| C7 | Low | Secrets managed via Environment Variables | **Open** |
| C8 | Medium | Cross-package V2 chain ID fallback to native IDs | **Fixed** |
| C9 | Low | Inconsistent cancellation routing falls through to Terra | **New** |
| C12 | **Critical** | EVM Approvals dropped on verification retry (Fail-Forget) | **Fixed** |

## Detailed Findings

### C1 - Verification fails open on source-chain query errors

**Status:** **Escalated to Critical (C12)**
**Analysis:** The previous finding correctly identified that RPC errors return `Pending`. However, the implication that the system "retries" was based on an assumption that the watcher maintains a retry queue. As discovered in C12, for EVM chains, there is no retry queue. A `Pending` result effectively means the approval is ignored forever.

### C7 - Secrets managed via Environment Variables

**Status:** **Open**
**Location:** `src/config.rs`
**Observation:** `EVM_PRIVATE_KEY` and `TERRA_MNEMONIC` are loaded directly from environment variables.
**Recommendation:** Support loading secrets from file paths (e.g., `EVM_PRIVATE_KEY_FILE`) to integrate better with container orchestration secret management.

### C9 - Inconsistent cancellation routing falls through to Terra

**Severity:** Low
**Location:** `src/watcher.rs` (`submit_cancel`)
**Observation:** The logic for submitting cancellations attempts to route to the correct EVM chain based on `dest_chain`. However, if the EVM cancellation fails (e.g., `can_cancel` pre-check fails or transaction fails) or if the `if/else if` block completes without returning, the code falls through to `Try Terra`.
```rust
if dest_chain == self.this_chain_id { ... }
else if let Some(peer) = ... { ... }

// Falls through to here even if dest_chain was EVM but failed
if self.terra_client.can_cancel(xchain_hash_id)...
```
**Risk:** If `dest_chain` is known to be EVM (e.g. Chain ID 1), attempting to cancel on Terra (Chain ID 2) is logically incorrect and wastes resources/logs, though likely harmless as Terra should reject the hash.
**Recommendation:** Restructure the routing to be exclusive based on `dest_chain`. If `dest_chain` is EVM, only attempt EVM cancellation.

### C12 - EVM Approvals dropped on verification retry (Fail-Forget)

**Severity:** **Critical**
**Location:** `src/watcher.rs` (`poll_evm_approvals`)
**Observation:**
The EVM polling loop consumes events using a block range filter (`last_evm_block + 1` to `current_block`).
1. It fetches `WithdrawApprove` events.
2. It attempts to verify them via `verify_and_cancel`.
3. If verification returns `VerificationResult::Pending` (due to source chain RPC error, unknown chain ID, or transient failure), the function logs a message and returns `Ok`.
4. Crucially, the approval is **not stored** in any pending queue or retry list.
5. The `last_evm_block` is updated to `current_block`.
6. On the next poll cycle, the watcher reads from `current_block + 1`. The previously "Pending" approval is in the past and will **never be retried**.

**Contrast with Terra:** `poll_terra_approvals` queries the *current state* of pending withdrawals from the bridge contract, so it naturally retries every cycle. EVM relies on *events*, which are ephemeral in this consumption model.

**Risk:** If an attacker can cause the verifier to return `Pending` (e.g., by DDoS-ing the source chain RPC) for just the single poll cycle where their approval event appears, the canceler will drop the approval and never check it again. The attacker can then wait out the cancel window and execute the fraud. This converts a transient availability issue into a permanent security failure.

**Resolution (four-part fix):**
1. **`BoundedMapCache<V>`** (`src/bounded_cache.rs`): New generic bounded key→value cache with TTL and oldest-first eviction. Used to store `PendingApproval` values keyed by hash. Memory-bounded: with default cap of 10 000 entries at ~200 bytes each, worst-case is ~2.5 MB.
2. **Retry queue** (`src/watcher.rs`): `pending_retry_queue: BoundedMapCache<PendingApproval>` field added to `CancelerWatcher`. When `verify_and_cancel` returns `Pending`, the approval is inserted into the queue. When it resolves as `Valid` or `Invalid`, it is removed. On verification error, it is re-inserted.
3. **Retry loop** (`src/watcher.rs`): `retry_pending_approvals()` is called at the start of every `poll_approvals()` cycle. It drains the queue, re-verifies each approval, and lets `verify_and_cancel` re-insert any that are still `Pending`.
4. **Config & metrics**: `PENDING_RETRY_MAX_SIZE` (default 10 000) and `PENDING_RETRY_TTL_SECS` (default 7200 = 2 hours) env vars control bounds. Prometheus gauge `canceler_pending_retry_queue_size` exposes current queue depth. 80% capacity warnings are logged.

**Memory safety:** The queue cannot grow unbounded because:
- Hard cap via `max_size` (default 10 000) — oldest entries are evicted when full.
- TTL eviction (default 2 hours) — stale entries are removed on every `insert` and `take_all`.
- Chain resets clear the queue entirely.
