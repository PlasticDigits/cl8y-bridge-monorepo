# Security Review: Operator Package (Comprehensive Audit)

**Date:** 2026-02-19
**Target:** `packages/operator`
**Reviewer:** AI Assistant

## Overview

This review provides a comprehensive security audit of the `operator` package, specifically focusing on the V2 Watchtower architecture, cross-chain verification logic, and resource management. This audit builds upon the findings from 2026-02-12.

The operator implements a "Watchtower" pattern:
1.  **Poll**: Monitors `WithdrawSubmit` (EVM) and `PendingWithdrawals` (Terra).
2.  **Verify**: Checks `getDeposit(hash)` on the source chain (RPC/LCD).
3.  **Approve**: Submits `withdrawApprove` on the destination chain.

## Remediation Status (Previous)

| ID | Finding | Status |
|----|---------|--------|
| 1.1 | Manual HTTP server | **Fixed** (verified) |
| 1.2 | Manual header parsing | **Fixed** (verified) |
| 2.1 | Database truncation | **Fixed** (verified) |
| 2.3 | Secret Management | **Mitigated** (Secrets redacted in Debug) |

---

## 3. New Findings (2026-02-19)

### 3.1 Terra LCD Query URL Injection (Low Risk)
**Location:** `src/watchers/terra.rs`, `src/writers/terra.rs`

The Terra watcher and writer construct LCD query URLs using `format!`:
```rust
format!("{}/cosmos/tx/v1beta1/txs?events=wasm._contract_address='{}'...", lcd_url, bridge_address)
```
While `bridge_address` is validated to be non-empty in `config.rs`, it is not strictly validated against the bech32 format before being used in the URL. If a malicious actor were to control the configuration (e.g., via env vars), they could potentially inject query parameters.

-   **Risk**: Low (Requires config compromise).
-   **Recommendation**: Validate `bridge_address` using a bech32 library or regex during configuration load to ensure it contains only valid characters.

### 3.2 Cache Eviction Performance (Informational)
**Location:** `src/bounded_cache.rs`

The `BoundedHashCache` and `BoundedPendingCache` implement eviction using an O(N) scan over the entire map (`min_by_key`) when the cache is full.
-   **Default Size**: 100,000 entries.
-   **Impact**: When the cache is full, every new approval insertion triggers a scan of 100,000 items.
-   **Mitigation**: The insertion rate is naturally limited by the cost of on-chain transactions (gas) and block times. An attacker would need to pay significant fees to flood the cache.
-   **Recommendation**: Monitor CPU usage. If performance degrades, switch to an O(1) eviction policy (e.g., `lru` crate or `VecDeque` ring buffer).

### 3.3 Token Registry Fallback (Informational)
**Location:** `src/watchers/evm.rs`

When parsing V2 Deposit events, the watcher attempts to query the `TokenRegistry` contract to get the canonical destination token address.
-   **Logic**: `token_registry.getDestToken(token, dest_chain)`
-   **Fallback**: If the registry call fails (or registry address is 0), it falls back to using the source token address (legacy behavior).
-   **Risk**: If the `TokenRegistry` is misconfigured or unreachable, the operator might compute a different hash than the destination bridge expects (if the bridge uses a mapped token address), causing approvals to fail or be for the wrong hash.
-   **Recommendation**: Ensure `TokenRegistry` is high-availability. Consider failing loudly or retrying instead of silent fallback if strict correctness is required.

### 3.4 Database Field Validation (Low Risk)
**Location:** `src/db/mod.rs`

The `insert_evm_deposit` and `insert_terra_deposit` functions accept `Vec<u8>` or `String` fields without strict length validation at the application level (unlike `insert_approval` which checks VARCHAR(42)).
-   **Observation**: `dest_chain_key`, `dest_token_address`, `dest_account` are inserted as byte vectors.
-   **Risk**: If a watcher parses a malformed event with valid topics but huge data fields, it could bloat the database.
-   **Recommendation**: Add explicit length checks (e.g., `bytes.len() <= 32`) in the `NewEvmDeposit` / `NewTerraDeposit` structs or insert functions.

## 4. Architecture Security Review

### 4.1 Cross-Chain Verification (Strong)
The operator correctly implements the V2 verification logic:
-   **Hash Matching**: It does **not** recompute hashes from raw fields (which is error-prone). It takes the `xchain_hash_id` from the destination chain and verifies existence of that *same hash* on the source chain.
-   **Source Routing**: It uses a map of `source_chain_endpoints` to verify deposits on the correct origin chain (supporting multi-EVM).
-   **Chain ID Validation**: It validates that the deposit's `destChain` matches the operator's configured chain ID.

### 4.2 Resource Management (Strong)
-   **Bounded Caches**: Prevents memory exhaustion attacks.
-   **Circuit Breakers**: Prevents cascading failures by pausing writers after consecutive errors.
-   **Rate Limiting**: API endpoints are rate-limited (`tower_governor`).

### 4.3 Secret Safety (Strong)
-   **Redaction**: `Debug` implementations for `Config` structs redact sensitive fields (`private_key`, `mnemonic`, `database.url`).
-   **Env Vars**: Secrets are loaded from environment, which is standard practice for containerized deployments.

## 5. Conclusion

The `operator` package demonstrates a mature security posture. The V2 architecture significantly reduces risk by removing the need for the operator to sign user-initiated withdrawals. The primary remaining risks are low-severity configuration validation issues.

**Action Items:**
1.  Add bech32 validation for `terra.bridge_address` in `config.rs`.
2.  Add length validation for byte fields in `db/models.rs` or `db/mod.rs`.
