# Security Review: canceler Package

**Date:** February 11, 2025  
**Package:** cl8y-canceler (v0.1.0)  
**Scope:** Full codebase analysis for security vulnerabilities  
**Reviewer:** Security Review

---

## Executive Summary

The canceler package implements the watchtower security pattern for the CL8Y Bridge. It monitors withdrawal approvals on EVM and Terra chains, verifies each approval against the source chain deposit, and submits cancellation transactions for fraudulent approvals (those without corresponding deposits).

**Overall Assessment:** The canceler follows reasonable security practices. No `unsafe` code was found. Sensitive data handling, verification logic, and error propagation are generally sound. Findings center on config Debug leakage risk, HTTP client timeouts, and a few panic-prone paths.

---

## 1. Findings Summary

| ID | Severity | Category | Description | Status |
|----|----------|----------|-------------|--------|
| C1 | Medium | Sensitive Data | Config derives Debug; contains evm_private_key, terra_mnemonic | **Fixed** |
| C2 | Low | Error Handling | terra_client denom.parse().unwrap() for "uluna" | **Fixed** |
| C3 | Low | Defensive Coding | watcher uses reqwest::Client::new() without timeout for Terra queries | **Fixed** |
| C4 | Low | Error Handling | server.rs Prometheus/Response unwraps at startup/runtime | **Fixed** |
| C5 | Low | Signal Handling | main.rs signal handler .expect() panics on install failure | Acceptable |
| C6 | Informational | Cryptography | Terra mnemonic uses empty BIP39 passphrase | **Documented** |
| C7 | Informational | Network | RPC/LCD URLs from env; trusted config model | **Documented** |
| C8 | Informational | Access Control | Health/metrics endpoints unauthenticated | **Documented** |
| C9 | Informational | Dependencies | Run cargo audit in CI | **Fixed** |

---

## 2. Detailed Findings

### 2.1 Unsafe Code Usage

**Status:** ✅ No findings

The codebase contains no `unsafe` blocks or functions.

---

### 2.2 Sensitive Data Handling

#### 2.2.1 Config Debug Leakage (C1)

| Severity | Medium |
|----------|--------|
| **Location** | `config.rs:7` |
| **Snippet** | `#[derive(Debug, Clone)]` on `Config` |
| **Description** | `Config` contains `evm_private_key` and `terra_mnemonic`. If `Config` is ever logged with `{:?}` or used in `tracing::debug!(config = ?config, ...)`, secrets would leak. |
| **Remediation** | Implement custom `Debug` for `Config` that redacts `evm_private_key` and `terra_mnemonic` (e.g., `"<redacted>"`), or remove `Debug` derive and add manual `impl Debug` only for safe fields. |

```rust
// Recommended: Custom Debug impl
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("canceler_id", &self.canceler_id)
            .field("evm_rpc_url", &self.evm_rpc_url)
            .field("evm_private_key", &"<redacted>")
            .field("terra_mnemonic", &"<redacted>")
            // ... other non-sensitive fields
            .finish()
    }
}
```

---

### 2.3 Input Validation

- **Address parsing:** Uses `Address::from_str`, `FromStr` with `.wrap_err()` / `map_err()`. ✅ Robust.
- **V2 chain ID parsing:** `env::var` with `and_then`; invalid values fall back to querying bridge or defaults. ✅ Appropriate.
- **Terra withdrawal JSON:** `parse_bytes4_from_json` / `parse_bytes32_from_json` use `base64::decode().unwrap_or_default()` — invalid base64 yields zeros; could mask upstream bugs but low impact. Acceptable.

---

### 2.4 Error Handling & Panic-Prone Code

#### 2.4.1 Terra denom parse (C2)

| Severity | Low |
|----------|-----|
| **Location** | `terra_client.rs:212` |
| **Snippet** | `denom: "uluna".parse().unwrap()` |
| **Description** | Same pattern as multichain-rs. The literal `"uluna"` is a valid Cosmos denom; failure is impossible in practice. |
| **Remediation** | Replace with `.expect("uluna is a valid constant Terra denom")` for clarity, or keep as-is with a comment. |

#### 2.4.2 Prometheus/Response unwraps (C4)

| Severity | Low |
|----------|-----|
| **Location** | `server.rs:60-99`, `server.rs:195` |
| **Snippet** | `IntCounter::new(...).unwrap()`, `registry.register(...).unwrap()`, `Response::builder().body(...).unwrap()` |
| **Description** | Metric creation and response building use `unwrap()`. Metric names are compile-time constants; failure is rare. Response body build could theoretically fail. |
| **Remediation** | For metrics: document that failure indicates programmer error. For response: use `.map_err()` and return 500 with error. |

#### 2.4.3 Signal handler (C5)

| Severity | Low |
|----------|-----|
| **Location** | `main.rs:141`, `main.rs:147` |
| **Snippet** | `.expect("Failed to install Ctrl+C handler")`, `.expect("Failed to install signal handler")` |
| **Description** | Signal installation failure causes process panic at startup. Rare on supported platforms. |
| **Remediation** | Acceptable for startup-critical handlers. Alternatively log and continue without graceful shutdown. |

---

### 2.5 HTTP Client Configuration

#### 2.5.1 Watcher Terra client without timeout (C3)

| Severity | Low |
|----------|-----|
| **Location** | `watcher.rs:372`, `watcher.rs:417` |
| **Snippet** | `let client = reqwest::Client::new();` |
| **Description** | Terra status and pending-withdrawals queries use a default client with no explicit timeout. A slow or unresponsive Terra LCD could block the poll loop. |
| **Remediation** | Use `Client::builder().timeout(Duration::from_secs(30)).build()` to match verifier and terra_client. |

---

### 2.6 Cryptographic Analysis

#### 2.6.1 Terra mnemonic passphrase (C6)

| Severity | Informational |
|----------|---------------|
| **Location** | `terra_client.rs:75` |
| **Snippet** | `mnemonic.to_seed("")` |
| **Description** | Empty BIP39 passphrase; standard for Cosmos chains but no passphrase protection if mnemonic is compromised. |
| **Remediation** | Document in `SECURITY.md`. Consider optional BIP39 passphrase for high-security deployments if upstream supports it. |

---

### 2.7 Network & External Input

#### 2.7.1 URL trust model (C7)

| Severity | Informational |
|----------|---------------|
| **Description** | `EVM_RPC_URL`, `TERRA_LCD_URL`, `TERRA_RPC_URL` are read from environment. No validation. Malicious URLs could enable SSRF. |
| **Remediation** | Document that URL configuration is trusted. For multi-tenant or user-supplied config, consider scheme allowlisting (e.g., https only) and hostname validation. |

#### 2.7.2 Health and metrics endpoints (C8)

| Severity | Informational |
|----------|---------------|
| **Location** | `server.rs` — `/health`, `/healthz`, `/readyz`, `/metrics` |
| **Description** | Endpoints are unauthenticated. With default `HEALTH_BIND_ADDRESS=127.0.0.1`, only local access. If bound to `0.0.0.0`, any network peer can access stats and metrics. |
| **Remediation** | Document that health/metrics are for internal monitoring. If exposed externally, use firewall rules, auth, or a reverse proxy. |

---

### 2.8 Verification Logic

**Assessment:** ✅ Solid

- Hash recomputation in verifier matches approval parameters before source-chain lookup.
- EVM and Terra deposit verification correctly distinguish Valid / Invalid / Pending.
- Unknown source chains return `Invalid` (not `Pending`), avoiding indefinite retry.
- V2 chain ID resolution from config or bridge contract is appropriate.

---

### 2.9 Dependencies

| Dependency | Version | Assessment |
|------------|---------|------------|
| multichain-rs | path | Shared lib; inherits its security posture |
| alloy | 0.8 | EVM library; actively maintained |
| cosmrs | 0.17 | Cosmos SDK client |
| reqwest | 0.12 | HTTP client |
| axum | 0.8 | Web framework |
| bip39 | 2.0 | Mnemonics |
| hostname | 0.4 | Default canceler ID; transitive libc |
| libc | 0.2 | Transitive (hostname) |

**C9 – cargo audit:** Add `cargo audit` to CI for the canceler package (or include in a monorepo audit job).

---

## 3. Severity Definitions

| Severity | Definition |
|----------|------------|
| **Critical** | Remotely exploitable; leads to full compromise or significant fund loss |
| **High** | Significant impact; requires specific conditions |
| **Medium** | Moderate impact; information leakage or incorrect behavior |
| **Low** | Minor impact; edge cases or defense-in-depth |
| **Informational** | Best practices; no direct exploit path |

---

## 4. Recommended Actions (Prioritized)

### High Priority

1. **C1 – Config Debug**  
   Implement custom `Debug` for `Config` that redacts `evm_private_key` and `terra_mnemonic`.

### Medium Priority

2. **C3 – Watcher HTTP timeout**  
   Use `Client::builder().timeout(Duration::from_secs(30))` for Terra queries in `watcher.rs`.

3. **C2 – Terra denom**  
   Replace `"uluna".parse().unwrap()` with `.expect("uluna is a valid constant Terra denom")` for clarity.

### Low Priority

4. **C4 – Server unwraps**  
   Consider `map_err` for `Response::builder().body()` and document metric creation as programmer-error panics.

5. **C5 – Signal handler**  
   Consider logging and degrading gracefully if signal installation fails (optional).

### Informational

6. **C6 – BIP39 passphrase**  
   Document empty passphrase usage in `SECURITY.md`.

7. **C7 – URL trust**  
   Add `SECURITY.md` (or section) describing trusted config and URL handling.

8. **C8 – Health auth**  
   Document that health/metrics are unauthenticated and intended for internal use.

9. **C9 – cargo audit**  
   Add `cargo audit` to CI for the canceler package.

---

## 5. Verification Checklist

After implementing recommendations:

- [x] Config `Debug` does not expose `evm_private_key` or `terra_mnemonic`
- [x] Watcher Terra HTTP client has explicit timeout
- [x] Terra denom parse is documented or uses explicit expect
- [x] `cargo audit` runs in CI for canceler
- [x] `SECURITY.md` documents URL trust, BIP39 passphrase, and health endpoint exposure

---

## 6. References

- multichain-rs security review: `packages/multichain-rs/security_reviews/SECURITY_REVIEW_2025-02-11.md`
- Canceler docs: `docs/canceler-network.md` (if present)
- `.env.example`: Documents required configuration; test keys should not be used in production

---

*This review was performed through static code analysis. A full assessment would also include dynamic testing, dependency scanning in CI, and deployment security review.*
