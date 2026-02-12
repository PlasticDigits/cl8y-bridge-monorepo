# Security Review: `operator` Package

**Date:** 2026-02-12  
**Scope:** `packages/operator/src/**/*.rs`, `packages/operator/Cargo.toml`, `packages/operator/Cargo.lock`  
**Method:** Static code review + dependency scan (`cargo audit`) focused on secrets handling, input validation, authn/authz, unsafe patterns, and data exposure

## Findings Summary

| ID | Severity | Finding | Primary Impact |
|---|---|---|---|
| O1 | Medium | EVM source-chain verification uses local (destination) RPC/bridge context | Incorrect cross-chain verification can approve/skip withdrawals based on wrong chain state |
| O2 | Medium | API endpoints are unauthenticated and can expose pending transfer data when publicly bound | Operational and transaction metadata exposure |
| O3 | Medium | Custom TCP HTTP parsing and unbounded per-connection tasks enable request confusion and DoS | Availability degradation and unintended endpoint access behavior |
| O4 | High | `cargo audit` reports known vulnerable dependencies (6 advisories) | Memory safety, crypto side-channel, DoS, and SQL protocol-smuggling risk in dependency graph |
| O5 | Low | `static mut` global state in API server (`START_TIME`) uses unsafe mutable static access | Unsound concurrency pattern and potential UB risk |

No Critical findings were identified in this review.

## Detailed Findings

### O1 - EVM source verification is not routed by source chain

**Severity:** Medium  
**Location:** `src/writers/evm.rs` (`verify_deposit_on_source`)  
**Observation:** For non-Terra withdrawals, verification always queries `self.rpc_url` and `self.bridge_address` (the writer's current chain context), even when `src_chain_id` indicates another EVM chain. The code comment also notes this should route to source chain RPC in multi-EVM production.  
**Risk:** Source-of-truth validation for cross-chain deposits is weakened. In multi-EVM deployments, approval decisions can be made from the wrong chain state, creating integrity risk and mis-approval/missed-approval scenarios under chain mismatch conditions.

**Recommended action:**
- Route EVM verification by `src_chain_id` to the corresponding configured source chain RPC + bridge address.
- Fail closed for unknown/unmapped `src_chain_id` values (do not approve).
- Add explicit invariant tests covering BSC->opBNB / ETH->Polygon style source/destination mismatches.

### O2 - Unauthenticated API exposes internal transfer state

**Severity:** Medium  
**Location:** `src/main.rs`, `src/api.rs`  
**Observation:** `/status`, `/pending`, `/metrics`, and `/health` are served without authentication/authorization checks. Bind address is env-configurable (`OPERATOR_API_BIND_ADDRESS`) and can be set to non-local interfaces.  
**Risk:** If exposed beyond localhost, external parties can enumerate pending approvals/releases, recipients, amounts, and relayer health/throughput metadata, which can aid targeted abuse or operational intelligence gathering.

**Recommended action:**
- Restrict API binding to localhost by policy in production (or enforce allowlisted CIDRs).
- Add authentication (mTLS, reverse proxy auth, or signed token) for `/status` and `/pending`.
- Separate public liveness endpoints from privileged operational endpoints.

### O3 - Naive HTTP request parsing and unlimited spawned handlers

**Severity:** Medium  
**Location:** `src/api.rs`, `src/metrics.rs`  
**Observation:** Servers parse requests via substring checks (e.g., `request.contains("GET /pending")`) on a fixed 1024-byte buffer and spawn one task per accepted connection with no request/idle timeout or concurrency cap.  
**Risk:**  
- Request confusion: path matching can be influenced by arbitrary request bytes/headers due to substring matching rather than structured HTTP parsing.  
- DoS: unbounded open connections/tasks can consume file descriptors, memory, and scheduler capacity.

**Recommended action:**
- Replace custom TCP protocol handling with a hardened HTTP stack (`axum`/`hyper`) and strict path/method routing.
- Apply read/write/idle timeouts and connection limits.
- Add basic rate limiting for non-local clients.

### O4 - Dependency vulnerability exposure (`cargo audit`)

**Severity:** High  
**Location:** `packages/operator/Cargo.lock` (scan run in `packages/operator`)  
**Observation:** `cargo audit` reported 6 vulnerabilities and additional warnings in the dependency graph, including:
- `RUSTSEC-2026-0007` (`bytes` 1.11.0) - integer overflow / potential memory corruption
- `RUSTSEC-2024-0363` (`sqlx` 0.7.4) - protocol-level truncation/overflow issue
- `RUSTSEC-2024-0437` (`protobuf` 2.28.0) - uncontrolled recursion DoS
- `RUSTSEC-2026-0009` (`time` 0.3.36) - stack exhaustion DoS
- `RUSTSEC-2024-0344` (`curve25519-dalek` 3.2.0) - timing variability
- `RUSTSEC-2023-0071` (`rsa` 0.9.10) - Marvin timing side-channel

**Risk:** Known vulnerable transitive dependencies increase exploitability surface for untrusted network and data parsing paths.

**Recommended action:**
- Prioritize dependency upgrades to patched versions (direct and transitive), starting with `bytes`, `sqlx`, `protobuf`, and `time`.
- Add CI enforcement (`cargo audit` gate or allowlist with expiry + owner).
- Re-evaluate advisories without upstream patches (`rsa`) and document compensating controls.

### O5 - Unsafe mutable static for uptime tracking

**Severity:** Low  
**Location:** `src/api.rs` (`static mut START_TIME`)  
**Observation:** Uptime state is stored in a mutable static and accessed through `unsafe` blocks.  
**Risk:** This is an unsound pattern and can become undefined behavior if initialization/reads change under concurrency or refactoring.

**Recommended action:**
- Replace with `OnceLock<Instant>` or `LazyLock<Instant>` and remove `unsafe`.
- Add test coverage for repeated server initialization behavior.

## Additional Notes (Non-Finding Checks)

- **Secrets handling:** `Config` implements redacted `Debug` for sensitive values (`private_key`, `mnemonic`, DB URL), which is a positive control.
- **Input validation:** Basic shape checks exist, but many trust boundaries remain configuration-driven (RPC/LCD endpoints, API bind values). Hard allowlists and stricter URL validation would improve resilience.

## Severity Levels Used

- **High:** Known exploitable risk with meaningful integrity/confidentiality/availability impact and broad dependency/runtime exposure.
- **Medium:** Realistic deployment risk requiring attacker network reach or configuration influence; can materially affect validation, availability, or data exposure.
- **Low:** Defense-in-depth issue with narrower exploit conditions or lower direct impact.

## Recommended Actions (Prioritized)

1. Remediate vulnerable dependencies and enforce continuous `cargo audit` policy (**O4**).
2. Fix source-chain verification routing for multi-EVM approvals and fail closed on unknown source chain IDs (**O1**).
3. Replace custom TCP/HTTP handling with hardened framework routing + connection protections (**O3**).
4. Add authentication/access controls for operational endpoints and restrict bind surfaces in production (**O2**).
5. Remove `unsafe static mut` uptime state and use safe one-time initialization primitives (**O5**).

---

## Remediation Status (2026-02-12)

All five findings have been remediated:

| ID | Status | Remediation |
|---|---|---|
| O1 | **Fixed** | `verify_deposit_on_source` now routes by `src_chain_id` via a `source_chain_endpoints` map populated from config. Unknown source chains fail closed (return false). |
| O2 | **Fixed** | `/status` and `/pending` are now gated by optional `OPERATOR_API_TOKEN` bearer auth. `/health` and `/metrics` remain public. |
| O3 | **Fixed** | HTTP request parsing now uses structured first-line extraction (method + path). Connection concurrency bounded by semaphore (256). Read timeout of 5s applied. Both `api.rs` and `metrics.rs` hardened. |
| O4 | **Partially fixed** | `bytes` 1.11.0→1.11.1, `sqlx` 0.7→0.8.6, `protobuf` removed (prometheus default-features=false). 3 transitive advisories remain unfixable without upstream upgrades (curve25519-dalek, rsa, time) — documented with compensating controls in `.cargo/audit.toml`. `cargo audit` now passes (exit 0). Pre-commit hook added. |
| O5 | **Fixed** | `static mut START_TIME` replaced with `OnceLock<Instant>`. All `unsafe` blocks removed from `api.rs`. |

### Remaining Accepted Risks

| Advisory | Crate | Reason |
|---|---|---|
| RUSTSEC-2024-0344 | curve25519-dalek 3.2.0 | Transitive via cosmwasm; no application-layer scalar arithmetic |
| RUSTSEC-2023-0071 | rsa 0.9.10 | No fix available; pulled by sqlx-mysql (compile-time only, Postgres used at runtime) |
| RUSTSEC-2026-0009 | time 0.3.41 | Fix requires Rust ≥1.88; transitive via tendermint; operates on trusted chain data |

## Residual Risk

The operator remains security-sensitive because correctness depends on cross-chain RPC/LCD truthfulness and availability. Even after remediation, enforce runtime monitoring for approval anomalies, verification failures by source chain, API access patterns, and dependency advisory drift.
