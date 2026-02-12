# Security Review: `canceler` Package

**Date:** 2026-02-12  
**Scope:** `packages/canceler/src/*.rs` and runtime behavior implied by configuration/network interactions  
**Method:** Static code review focused on exploitability, integrity, and availability risks

## Findings Summary

| ID | Severity | Finding | Primary Impact |
|---|---|---|---|
| C1 | Medium | Verification fails open on source-chain query errors | Fraudulent approvals can remain uncanceled during RPC/LCD disruption |
| C2 | Medium | Terra approval polling is capped at 50 with no pagination | Backlog flooding can hide approvals from review/cancellation |
| C3 | Medium | Unbounded in-memory tracking sets (`verified_hashes`, `cancelled_hashes`) | Memory growth can degrade or crash the canceler (DoS) |
| C4 | Low | EVM cancellation proceeds even when pre-check fails | Repeated unnecessary transactions and gas burn risk |
| C5 | Low | Trusted URL/input model and optional public health bind | SSRF/info exposure risk in misconfigured or multi-tenant deployments |

No Critical findings were identified in this review.

## Detailed Findings

### C1 - Verification fails open on source-chain query errors

**Severity:** Medium  
**Location:** `src/verifier.rs` (`verify_evm_deposit`, `verify_terra_deposit`)  
**Observation:** On RPC/LCD errors or unsuccessful Terra query responses, verification returns `VerificationResult::Pending` instead of `Invalid`.  
**Risk:** If an attacker can degrade or intercept source-chain connectivity, fraudulent approvals may avoid cancellation until connectivity recovers. This creates a security dependency on RPC/LCD availability and trust.

**Recommended action:**
- Add a policy mode for failure handling:
  - `fail_closed` for high-security environments (treat repeated verification failures as suspicious and cancel or alert aggressively).
  - `fail_open` for low false-positive environments (current behavior).
- Track consecutive verification failures per `withdraw_hash` and escalate after threshold (alert + optional cancel attempt).
- Emit dedicated metrics for verification transport failures to trigger operational alarms quickly.

### C2 - Terra polling limit without pagination

**Severity:** Medium  
**Location:** `src/watcher.rs` (`poll_terra_approvals`)  
**Observation:** Terra smart query uses `{"pending_withdrawals": {"limit": 50}}` with no pagination or continuation token handling.  
**Risk:** If more than 50 pending approvals exist, some approvals may not be processed in time. A backlog flood can delay or evade cancellation windows.

**Recommended action:**
- Implement pagination until exhaustion each poll cycle.
- Process approvals in deterministic priority order (e.g., earliest `approved_at` first).
- Add a metric for queue depth / unprocessed approvals and alert when above threshold.

### C3 - Unbounded in-memory tracking sets

**Severity:** Medium  
**Location:** `src/watcher.rs` (`verified_hashes`, `cancelled_hashes`)  
**Observation:** Hashes are stored indefinitely in `HashSet`s with no TTL, compaction, or upper bound.  
**Risk:** Long-lived instances or adversarial event volume can force unbounded memory growth, leading to degraded performance or process termination.

**Recommended action:**
- Replace raw `HashSet` storage with bounded retention (TTL/LRU) keyed to practical replay windows.
- Persist minimal state (e.g., last processed height + compact dedupe cache) across restarts.
- Add memory pressure telemetry and self-protection behavior (throttle/log/drop oldest entries).

### C4 - Cancellation attempted after failed EVM pre-check

**Severity:** Low  
**Location:** `src/watcher.rs` (`submit_cancel`)  
**Observation:** If `evm_client.can_cancel()` errors, logic sets `can_cancel_evm = true` and proceeds with `withdrawCancel` attempt.  
**Risk:** During RPC instability, cancel attempts may be sent unnecessarily and can consume gas repeatedly, creating avoidable operational and economic loss.

**Recommended action:**
- Change policy to `can_cancel_evm = false` on pre-check failure, or retry pre-check with backoff before submitting.
- Gate transaction submission on explicit chain/approval ownership context where possible.
- Add circuit breaker behavior for repeated `can_cancel` failures.

### C5 - Trusted URL model and optional public observability endpoints

**Severity:** Low  
**Location:** `src/config.rs`, `src/server.rs`  
**Observation:** RPC/LCD URLs are accepted directly from environment; health/metrics can be exposed by setting `HEALTH_BIND_ADDRESS=0.0.0.0`.  
**Risk:** In deployments where config is not tightly controlled, this can enable SSRF-style access to internal services or external exposure of operational metadata.

**Recommended action:**
- Enforce URL validation/allowlisting in production deployments (scheme + host/IP constraints).
- Keep default localhost binding; if externally exposed, require network ACL/reverse proxy auth.
- Document strict trust assumptions for env-driven endpoint configuration.

## Severity Levels Used

- **Medium:** Realistic exploit or failure mode with meaningful integrity/availability impact and no special privileges beyond network/config influence.
- **Low:** Defense-in-depth issue or misconfiguration-dependent risk with limited direct impact.

## Recommended Actions (Prioritized)

1. Implement Terra pagination and backlog visibility metrics (**C2**).
2. Introduce bounded dedupe state with retention policy (**C3**).
3. Add verification failure escalation policy (`fail_closed` option + alerts) (**C1**).
4. Prevent blind cancel submissions when pre-check fails; add retries/circuit breaker (**C4**).
5. Harden configuration trust boundaries and endpoint exposure controls (**C5**).

## Residual Risk

This package is security-sensitive by design and depends on external chain RPC/LCD correctness and availability. Even after remediation, operational monitoring (alerts on verification failures, cancel failure rates, queue backlog, and memory growth) remains essential to maintain fraud-detection guarantees.
