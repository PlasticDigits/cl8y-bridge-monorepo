# Security Review: `canceler` Package

**Date:** 2026-02-12
**Scope:** `packages/canceler/src/*.rs` and runtime behavior implied by configuration/network interactions
**Method:** Static code review focused on exploitability, integrity, and availability risks

## Findings Summary

| ID | Severity | Finding | Status |
|---|---|---|---|
| C1 | Medium | Verification fails open on source-chain query errors | **Open** |
| C2 | Medium | Terra approval polling limit without pagination | **Fixed** |
| C3 | Medium | Unbounded in-memory tracking sets | **Fixed** |
| C4 | Low | EVM cancellation attempted after failed pre-check | **Fixed** |
| C5 | Low | Trusted URL model and optional public health bind | **Fixed** |
| C6 | High | Misconfigured Chain IDs trigger mass cancellation | **Fixed** |
| C7 | Low | Secrets managed via Environment Variables | **Open** |
| C8 | Medium | Cross-package V2 chain ID fallback to native IDs | **Fixed** |

## Detailed Findings

### C1 - Verification fails open on source-chain query errors

**Severity:** Medium
**Location:** `src/verifier.rs` (`verify_evm_deposit`, `verify_terra_deposit`)
**Observation:** On RPC/LCD errors or unsuccessful Terra query responses, verification returns `VerificationResult::Pending`. The `watcher` loop logs an error and retries, but does not escalate.
**Risk:** If an attacker can degrade or intercept source-chain connectivity (returning errors), fraudulent approvals may avoid cancellation until connectivity recovers. The system "fails open" (allows potential fraud to persist) rather than "failing closed" (pausing or alerting aggressively).
**Recommendation:**
- Implement a `fail_closed` policy for high-security modes.
- Track consecutive verification failures per hash and alert/escalate.

### C2 - Terra polling limit without pagination

**Status:** **Fixed**
**Resolution:** Pagination logic implemented in `poll_terra_approvals` using `terra_poll_page_size` and `terra_poll_max_pages`. Metrics added for queue depth and unprocessed approvals.

### C3 - Unbounded in-memory tracking sets

**Status:** **Fixed**
**Resolution:** `BoundedHashCache` implemented with configurable `dedupe_cache_max_size` and `dedupe_cache_ttl_secs`. Gauges added to monitor cache usage.

### C4 - Cancellation attempted after failed EVM pre-check

**Status:** **Fixed**
**Resolution:** Logic updated to track consecutive pre-check failures. A circuit breaker (`evm_precheck_circuit_open`) stops EVM cancel attempts if the threshold is reached. Exponential backoff implemented for retries.

### C5 - Trusted URL model and optional public observability endpoints

**Status:** **Fixed**
**Resolution:** `validate_rpc_url` implemented to enforce http/https schemes. Warnings added for non-localhost `HEALTH_BIND_ADDRESS`.

### C6 - Misconfigured Chain IDs trigger mass cancellation

**Severity:** High
**Status:** **Fixed**
**Location:** `src/verifier.rs` (`verify`), `src/watcher.rs` (`new`)
**Observation:** The verifier uses `EVM_V2_CHAIN_ID` and `TERRA_V2_CHAIN_ID` to identify the source chain of an approval. Previously, if `src_chain_id` matched neither, the code returned `VerificationResult::Invalid` with reason "Unknown source chain", triggering a cancellation transaction.
**Risk:** If the canceler was misconfigured (e.g., wrong V2 chain ID provided in env vars), **valid** approvals from the mismatched chain would be treated as having an "unknown source", marked `Invalid`, and the canceler would submit cancellation transactions for them — a catastrophic false-positive causing DoS for valid bridge users.

**Resolution (three-part fix):**
1. **Verifier (`src/verifier.rs`):** Changed unknown source chain from `VerificationResult::Invalid` to `VerificationResult::Pending`. The approval is retried but no destructive action is taken. An `error!`-level log with an `unknown_source_chain_count` counter ensures operators are alerted immediately.
2. **Startup validation (`src/watcher.rs`):** Added `validate_chain_ids_against_bridge()` which queries `getThisChainId()` from the EVM bridge contract and compares against the configured V2 chain ID. A mismatch produces an `error!`-level log at startup.
3. **Prometheus metric (`src/server.rs`):** Added `canceler_unknown_source_chain_total` gauge so monitoring systems can alert on sustained unknown-chain events (which indicate misconfiguration or a new unmonitored chain).

**Design rationale:** Hash integrity check (computed hash vs claimed hash) still returns `Invalid` — this is safe because it catches on-chain data inconsistency regardless of configuration. The unknown-chain branch is the only path where misconfiguration could cause false positives.

### C7 - Secrets managed via Environment Variables

**Severity:** Low
**Status:** **Open**
**Location:** `src/config.rs`
**Observation:** `EVM_PRIVATE_KEY` and `TERRA_MNEMONIC` are loaded directly from environment variables.
**Risk:** In shared or containerized environments, environment variables can sometimes be inspected by other processes or leak in crash dumps/logs (though `Config` debug is redacted, the raw env vars might not be).
**Recommendation:**
- Support loading secrets from file paths (e.g., Docker secrets, Kubernetes secrets) in addition to env vars.
- Ensure the process environment is locked down.

### C8 - Cross-package V2 chain ID fallback to native IDs

**Severity:** Medium
**Status:** **Fixed**
**Location:** Multiple files across `packages/operator/` and `packages/canceler/`
**Observation:** Multiple locations across both the canceler and operator previously fell back to converting native chain IDs (e.g., 31337 for Anvil) to V2 4-byte IDs when the V2 chain ID was not explicitly configured. This conversion was incorrect when ChainRegistry uses sequential IDs (e.g., 0x00000001) rather than native IDs.

**Resolution:** All native-ID fallbacks have been removed. V2 chain IDs are now resolved via config or bridge contract query, and the service **refuses to start** if neither succeeds:
- `packages/canceler/src/verifier.rs`: `new_v2()` now requires `[u8; 4]` chain IDs (not `Option`). Old `with_v2_chain_ids` removed.
- `packages/canceler/src/watcher.rs`: `resolve_v2_chain_ids_required()` returns `Result<([u8; 4], [u8; 4])>` — hard error on failure.
- `packages/operator/src/multi_evm.rs`: `{prefix}_THIS_CHAIN_ID` is now required — startup error if missing.
- `packages/operator/src/watchers/evm.rs`: Falls back to config `EVM_THIS_CHAIN_ID` only; errors if both bridge query and config fail.
- `packages/operator/src/writers/evm.rs`: Same pattern for EVM; `TERRA_THIS_CHAIN_ID` required when Terra config present; `process_evm_deposit` returns error if deposit has no stored V2 chain ID.
- `packages/operator/src/writers/terra.rs`: Falls back to contract query only; errors if both query and `TERRA_THIS_CHAIN_ID` config fail.

## Common Rust Web Service Errors Review

1.  **Panic Handling:**
    -   `unwrap()` usage was reviewed. Most are guarded or used on constant parsing (safe).
    -   `amount.try_into().unwrap_or(...)` correctly clamps values instead of panicking.
    -   **Verdict:** Safe.

2.  **Integer Arithmetic:**
    -   `u128` used for amounts. `saturating_sub` used for block lookback.
    -   **Verdict:** Safe.

3.  **Concurrency:**
    -   `tokio::select!` used for shutdown and polling.
    -   `AtomicBool`/`AtomicU32`/`AtomicU64` used for shared state signaling (circuit breaker, counters).
    -   **Verdict:** Safe.

4.  **Resource Exhaustion:**
    -   Pagination and Bounded Cache limits prevent memory exhaustion.
    -   **Verdict:** Safe (remediated by C2/C3 fixes).

## Residual Risk

The `canceler` is a critical security component. The primary remaining risks are operational:
1. **RPC Availability (C1):** The service depends entirely on the view of the chain provided by the RPC/LCD nodes.
2. **Startup Dependency:** Both the canceler and operator now require either explicit V2 chain ID config or a reachable bridge contract at startup. This is the correct trade-off: fail loudly at startup rather than silently misbehave at runtime.
