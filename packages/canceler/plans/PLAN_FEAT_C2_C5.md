# Feature Plan: Fix Security Findings C2-C5

**Source:** `security_reviews/SECURITY_REVIEW_2026-02-12.md`
**Scope:** C2 (Terra pagination), C3 (bounded dedupe), C4 (EVM pre-check), C5 (URL/endpoint hardening)
**Excluded:** C1 (fail-open verification) -- deferred; the operator service shares the same RPC/LCD dependency, so a policy change here requires coordinated design across both packages.

---

## Task 1 -- C2: Terra Approval Polling Pagination

### Problem

`poll_terra_approvals` in `src/watcher.rs:547-552` sends a single query with a hardcoded `limit: 50` and no `start_after` cursor. If the Terra bridge contract holds more than 50 pending withdrawals, the excess approvals are silently dropped each poll cycle. An attacker can flood the queue with 50+ low-value approvals to push a fraudulent high-value approval past the cancellation window.

### Current Code

```rust
// watcher.rs:547-552
let query = serde_json::json!({
    "pending_withdrawals": {
        "limit": 50
    }
});
```

The response is iterated once (`watcher.rs:570-648`) with no continuation.

### Tasks

#### 1a. Add `start_after` pagination loop

Replace the single query with a loop that pages through all pending withdrawals until the contract returns fewer results than the page size.

- Build the first query with `{"pending_withdrawals": {"limit": 50}}`.
- On each response, if the result array length equals the page size, extract the last `withdraw_hash` and issue a follow-up query: `{"pending_withdrawals": {"limit": 50, "start_after": "<last_hash_b64>"}}`.
- Continue until the response contains fewer than `limit` entries.
- Cap the total number of pages per poll cycle at a configurable maximum (default 20, i.e., 1000 approvals) to bound poll duration. Log a warning if the cap is reached.

**File changes:** `src/watcher.rs` -- `poll_terra_approvals`.

#### 1b. Sort by `approved_at` before processing

After collecting all pages, sort the collected `PendingApproval` vec by `approved_at_timestamp` ascending so the oldest (closest to cancel-window expiry) approvals are processed first.

**File changes:** `src/watcher.rs` -- `poll_terra_approvals`, after the pagination loop and before the `verify_and_cancel` loop.

#### 1c. Add backlog metrics

Register two new Prometheus gauges in `Metrics`:

- `canceler_terra_pending_queue_depth` -- total number of pending Terra withdrawals seen this poll cycle (sum across all pages).
- `canceler_terra_unprocessed_approvals` -- number of approvals that were not processed because the page cap was reached.

Update the gauges at the end of `poll_terra_approvals`.

**File changes:** `src/server.rs` -- `Metrics::new` (register gauges), struct fields. `src/watcher.rs` -- set gauge values.

#### 1d. Make page size configurable

Add `TERRA_POLL_PAGE_SIZE` (default `50`) and `TERRA_POLL_MAX_PAGES` (default `20`) to `Config`.

**File changes:** `src/config.rs` -- new fields + env parsing. `src/watcher.rs` -- read from `self.config`.

### Acceptance Criteria

- [ ] With >50 pending withdrawals, all are fetched across multiple pages.
- [ ] Approvals are processed oldest-first by `approved_at_timestamp`.
- [ ] `canceler_terra_pending_queue_depth` gauge reflects total queue size.
- [ ] `canceler_terra_unprocessed_approvals` is 0 when all pages are consumed, >0 when page cap is hit.
- [ ] Page cap prevents unbounded poll duration; a warning is logged when the cap is reached.
- [ ] Existing unit and integration tests still pass.

### Estimated Effort

Medium -- ~3-4 hours. The pagination loop is straightforward; the main work is testing the multi-page path and wiring the new metrics.

---

## Task 2 -- C3: Bounded Dedupe State

### Problem

`CancelerWatcher` in `src/watcher.rs:92-94` stores every processed hash forever in two `HashSet<[u8; 32]>` fields:

```rust
// watcher.rs:92-94
verified_hashes: HashSet<[u8; 32]>,
cancelled_hashes: HashSet<[u8; 32]>,
```

Each entry is 32 bytes of key + HashSet overhead (~56 bytes/entry with the default hasher). Over weeks of operation or under adversarial event volume, these sets grow without bound. At 100k entries the overhead is ~8.5 MB (tolerable), but at 10M entries it reaches ~850 MB and risks OOM termination.

### Tasks

#### 2a. Replace `HashSet` with a TTL-aware bounded map

Introduce a new internal type `BoundedHashCache` (in a new file `src/bounded_cache.rs`) that wraps a `HashMap<[u8; 32], Instant>` with:

- **Max capacity:** configurable via `DEDUPE_CACHE_MAX_SIZE` (default `100_000`).
- **TTL:** configurable via `DEDUPE_CACHE_TTL_SECS` (default `86400`, i.e., 24 hours). Entries older than TTL are eligible for eviction.
- **Eviction policy:** on insert when at capacity, evict the oldest entry (by insertion `Instant`). This is simpler and more predictable than LRU for a dedupe cache where access pattern is insert-once-check-many.
- Expose `contains(&[u8; 32]) -> bool`, `insert([u8; 32])`, `len() -> usize`, `clear()`.

**File changes:** new `src/bounded_cache.rs`. `src/lib.rs` -- add `pub mod bounded_cache;`.

#### 2b. Replace `HashSet` fields in `CancelerWatcher`

Swap `verified_hashes: HashSet<[u8; 32]>` and `cancelled_hashes: HashSet<[u8; 32]>` for `BoundedHashCache` instances, constructed from config values in `CancelerWatcher::new`.

Update all call sites:
- `watcher.rs:167-168` (construction)
- `watcher.rs:314-315` (chain-reset clear)
- `watcher.rs:381-382`, `590-591`, `703`, `706` (contains checks)
- `watcher.rs:728`, `761` (inserts)
- `watcher.rs:877` (stats len)

**File changes:** `src/watcher.rs`.

#### 2c. Add config fields

Add to `Config`:
- `dedupe_cache_max_size: usize` -- from `DEDUPE_CACHE_MAX_SIZE`, default `100_000`.
- `dedupe_cache_ttl_secs: u64` -- from `DEDUPE_CACHE_TTL_SECS`, default `86400`.

**File changes:** `src/config.rs` -- struct fields, `Config::load`, `Debug` impl.

#### 2d. Add memory pressure telemetry

Register two new Prometheus gauges in `Metrics`:

- `canceler_dedupe_verified_size` -- current number of entries in `verified_hashes`.
- `canceler_dedupe_cancelled_size` -- current number of entries in `cancelled_hashes`.

Update gauges at the end of each `poll_approvals` cycle.

Log a warning when either cache reaches 80% of `max_size`.

**File changes:** `src/server.rs` -- gauge registration. `src/watcher.rs` -- gauge updates + warning.

#### 2e. Unit tests for `BoundedHashCache`

- Insert up to max capacity; verify oldest entry is evicted on overflow.
- Verify TTL expiry: insert, advance time (use `tokio::time::pause`), confirm entry is evicted.
- Verify `clear()` resets length to 0.
- Verify `contains` returns `false` for evicted entries.

**File changes:** `src/bounded_cache.rs` -- `#[cfg(test)] mod tests`.

### Acceptance Criteria

- [ ] `verified_hashes` and `cancelled_hashes` never exceed `dedupe_cache_max_size`.
- [ ] Entries older than `dedupe_cache_ttl_secs` are evicted.
- [ ] `canceler_dedupe_verified_size` and `canceler_dedupe_cancelled_size` gauges are populated.
- [ ] Warning logged when cache reaches 80% capacity.
- [ ] Chain-reset still clears both caches.
- [ ] All existing tests pass; new unit tests cover capacity, TTL, and clear.

### Estimated Effort

Medium -- ~3-4 hours. The cache implementation is small; the bulk is wiring config, metrics, and writing tests.

---

## Task 3 -- C4: EVM Pre-Check Safety

### Problem

In `submit_cancel` (`src/watcher.rs:793-809`), when `evm_client.can_cancel()` returns an `Err`, the code sets `can_cancel_evm = true` and proceeds to submit the `withdrawCancel` transaction:

```rust
// watcher.rs:802-809
Err(e) => {
    warn!(
        error = %e,
        hash = %bytes32_to_hex(&withdraw_hash),
        "Failed to check can_cancel on EVM, will try anyway"
    );
    true // Try anyway
}
```

During RPC instability this causes repeated blind transaction submissions, burning gas on reverts and creating noise.

### Tasks

#### 3a. Change default to `false` on pre-check error

Replace `true // Try anyway` with `false` so that a failed pre-check skips the EVM cancel attempt for this cycle. The approval will be retried on the next poll (it is not added to `cancelled_hashes` on failure).

Log at `warn` level with a clear message: `"EVM can_cancel pre-check failed; skipping cancel attempt this cycle (will retry)"`.

**File changes:** `src/watcher.rs` -- `submit_cancel`, the `Err(e)` arm of the `can_cancel` match.

#### 3b. Add retry with exponential backoff

Before returning `false`, retry the `can_cancel` call up to `EVM_PRECHECK_MAX_RETRIES` times (default `2`, configurable) with exponential backoff starting at 500ms (500ms, 1s).

Implement as a simple loop inside `submit_cancel` (no new async machinery needed):

```rust
let mut can_cancel_evm = false;
let mut last_err = None;
for attempt in 0..=self.config.evm_precheck_max_retries {
    match self.evm_client.can_cancel(withdraw_hash).await {
        Ok(can) => { can_cancel_evm = can; break; }
        Err(e) => {
            last_err = Some(e);
            if attempt < self.config.evm_precheck_max_retries {
                let delay = Duration::from_millis(500 * 2u64.pow(attempt as u32));
                tokio::time::sleep(delay).await;
            }
        }
    }
}
if let Some(e) = last_err {
    if !can_cancel_evm {
        warn!(error = %e, ...);
        self.evm_precheck_consecutive_failures += 1;
    }
} else {
    self.evm_precheck_consecutive_failures = 0;
}
```

**File changes:** `src/watcher.rs` -- `submit_cancel`. `src/config.rs` -- `evm_precheck_max_retries: u32` field.

#### 3c. Circuit breaker for repeated failures

Add a `evm_precheck_consecutive_failures: u32` counter to `CancelerWatcher`. When it exceeds `EVM_PRECHECK_CIRCUIT_BREAKER_THRESHOLD` (default `10`, configurable):

- Log at `error` level: `"EVM pre-check circuit breaker OPEN -- skipping all EVM cancel attempts until a successful pre-check"`.
- Skip the entire EVM cancel path in `submit_cancel` (fall through to Terra).
- Increment a Prometheus counter `canceler_evm_precheck_circuit_breaker_trips_total`.
- On the next successful `can_cancel` call, reset the counter to 0 and log `"EVM pre-check circuit breaker CLOSED"`.

**File changes:** `src/watcher.rs` -- `CancelerWatcher` struct (new field), `submit_cancel` logic. `src/config.rs` -- `evm_precheck_circuit_breaker_threshold: u32`. `src/server.rs` -- new counter metric.

### Acceptance Criteria

- [ ] A single `can_cancel` RPC failure no longer triggers a blind `withdrawCancel` submission.
- [ ] Up to `evm_precheck_max_retries` retries are attempted with exponential backoff.
- [ ] After `evm_precheck_circuit_breaker_threshold` consecutive failures, EVM cancel path is skipped entirely.
- [ ] Circuit breaker resets on first successful pre-check.
- [ ] `canceler_evm_precheck_circuit_breaker_trips_total` metric is incremented on each trip.
- [ ] Terra cancel path is still attempted as fallback regardless of EVM circuit breaker state.

### Estimated Effort

Small-Medium -- ~2-3 hours. Mostly control-flow changes in `submit_cancel` and config wiring.

---

## Task 4 -- C5: URL Validation and Endpoint Hardening

### Problem

`Config::load` in `src/config.rs:120-141` reads `EVM_RPC_URL`, `TERRA_LCD_URL`, and `TERRA_RPC_URL` directly from environment with no validation. A malicious or misconfigured URL (e.g., `file:///etc/passwd`, `http://169.254.169.254/...`) could cause SSRF-like behavior via `reqwest` or `alloy` HTTP providers.

Additionally, `HEALTH_BIND_ADDRESS` defaults to `127.0.0.1` but can be set to `0.0.0.0` without any warning at startup, silently exposing operational metrics to the network.

### Tasks

#### 4a. Add URL validation helper

Create a `validate_rpc_url(url: &str, name: &str) -> Result<()>` function in `src/config.rs` that:

1. Parses the string as a `url::Url` (add the `url` crate to `Cargo.toml`).
2. Rejects schemes other than `http` and `https`. Return `Err` with a clear message: `"{name} must use http:// or https:// scheme, got {scheme}"`.
3. Rejects URLs with no host component.
4. Logs a warning (not error) if the scheme is `http` (not `https`): `"{name} uses unencrypted http:// -- use https:// in production"`.

Call `validate_rpc_url` for `EVM_RPC_URL`, `TERRA_LCD_URL`, and `TERRA_RPC_URL` inside `Config::load`, after reading the env var and before storing the value.

**File changes:** `Cargo.toml` -- add `url = "2"`. `src/config.rs` -- new function + calls in `load`.

#### 4b. Warn on non-localhost health bind

In `Config::load`, after reading `HEALTH_BIND_ADDRESS`, if the value is not `127.0.0.1` and not `::1`:

- Log at `warn` level: `"HEALTH_BIND_ADDRESS is set to {addr} -- health and metrics endpoints will be accessible from the network. Use firewall rules or a reverse proxy to restrict access in production."`.

This is a warning only, not a hard error, to preserve flexibility for container deployments that legitimately need `0.0.0.0`.

**File changes:** `src/config.rs` -- after `health_bind_address` assignment.

#### 4c. Document trust assumptions in `SECURITY.md`

Append a section to the existing `SECURITY.md` that consolidates the trust model:

- RPC/LCD URLs are validated for scheme and host but are otherwise trusted. The canceler will make authenticated (signed) transactions to whatever endpoint is configured.
- `HEALTH_BIND_ADDRESS` defaults to localhost. Changing it to `0.0.0.0` exposes unauthenticated endpoints.
- In multi-tenant or orchestrated environments, URL values should come from a trusted secret store, not user-supplied input.

**File changes:** `SECURITY.md`.

### Acceptance Criteria

- [ ] `Config::load` rejects `EVM_RPC_URL`, `TERRA_LCD_URL`, `TERRA_RPC_URL` values with non-http(s) schemes (e.g., `file://`, `ftp://`, empty string).
- [ ] `Config::load` warns on `http://` (non-TLS) URLs.
- [ ] `Config::load` warns when `HEALTH_BIND_ADDRESS` is not localhost.
- [ ] `SECURITY.md` documents the URL trust model and health endpoint exposure.
- [ ] Existing tests and integration tests pass (they use `http://localhost:*` which is valid).

### Estimated Effort

Small -- ~1-2 hours. Mostly validation logic and documentation.

---

## Summary of File Changes

| File | C2 | C3 | C4 | C5 |
|---|---|---|---|---|
| `src/watcher.rs` | Pagination loop, sort, gauge updates | Swap `HashSet` for `BoundedHashCache`, gauge updates | Retry + circuit breaker in `submit_cancel` | -- |
| `src/config.rs` | `terra_poll_page_size`, `terra_poll_max_pages` | `dedupe_cache_max_size`, `dedupe_cache_ttl_secs` | `evm_precheck_max_retries`, `evm_precheck_circuit_breaker_threshold` | `validate_rpc_url`, health bind warning |
| `src/server.rs` | Queue depth + unprocessed gauges | Dedupe size gauges | Circuit breaker trip counter | -- |
| `src/bounded_cache.rs` | -- | New file | -- | -- |
| `src/lib.rs` | -- | `pub mod bounded_cache;` | -- | -- |
| `Cargo.toml` | -- | -- | -- | `url = "2"` |
| `SECURITY.md` | -- | -- | -- | Trust model docs |

## New Config Environment Variables

| Variable | Default | Finding | Description |
|---|---|---|---|
| `TERRA_POLL_PAGE_SIZE` | `50` | C2 | Number of pending withdrawals per Terra query page |
| `TERRA_POLL_MAX_PAGES` | `20` | C2 | Max pages fetched per poll cycle (bounds poll duration) |
| `DEDUPE_CACHE_MAX_SIZE` | `100000` | C3 | Max entries in each dedupe hash cache |
| `DEDUPE_CACHE_TTL_SECS` | `86400` | C3 | Seconds before a dedupe entry is eligible for eviction |
| `EVM_PRECHECK_MAX_RETRIES` | `2` | C4 | Retry count for `can_cancel` pre-check on failure |
| `EVM_PRECHECK_CIRCUIT_BREAKER_THRESHOLD` | `10` | C4 | Consecutive pre-check failures before circuit breaker opens |

## New Prometheus Metrics

| Metric | Type | Finding | Description |
|---|---|---|---|
| `canceler_terra_pending_queue_depth` | Gauge | C2 | Total pending Terra withdrawals seen this poll |
| `canceler_terra_unprocessed_approvals` | Gauge | C2 | Approvals skipped due to page cap |
| `canceler_dedupe_verified_size` | Gauge | C3 | Current entries in verified-hashes cache |
| `canceler_dedupe_cancelled_size` | Gauge | C3 | Current entries in cancelled-hashes cache |
| `canceler_evm_precheck_circuit_breaker_trips_total` | Counter | C4 | Times the EVM pre-check circuit breaker tripped |

## Implementation Order

1. **C5** (URL validation) -- smallest scope, no behavioral changes to core loop, unblocks safe testing of the rest.
2. **C4** (EVM pre-check) -- isolated to `submit_cancel`, no data-structure changes.
3. **C3** (bounded dedupe) -- new file + struct swap; do before C2 so the pagination work already uses bounded caches.
4. **C2** (Terra pagination) -- largest scope; depends on metrics infrastructure added in C3/C4.

## Review Checkpoints

After each task, the implementer should:

1. Run `cargo build` and `cargo test` (unit tests).
2. Run `cargo clippy -- -D warnings`.
3. Verify new metrics appear in `/metrics` output (manual or integration test).
4. Update this plan document with completion status.

---

## Implementation Status (2026-02-12)

| Task | Status | Notes |
|------|--------|-------|
| C5 – URL validation | ✅ Done | `validate_rpc_url`, health bind warning, SECURITY.md |
| C4 – EVM pre-check | ✅ Done | `can_cancel_evm=false` on error, retry, circuit breaker |
| C3 – Bounded dedupe | ✅ Done | `BoundedHashCache`, gauges, 80% warning |
| C2 – Terra pagination | ✅ Done | Pagination loop, sort by `approved_at`, queue metrics |

- `cargo build`, `cargo test --lib`, `cargo clippy -- -D warnings` all pass.
