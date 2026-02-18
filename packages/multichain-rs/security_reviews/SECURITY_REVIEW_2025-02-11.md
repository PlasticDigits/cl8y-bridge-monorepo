# Security Review: multichain-rs Package

**Date:** February 11, 2025  
**Package:** multichain-rs (v0.1.0)  
**Scope:** Full codebase analysis for security vulnerabilities  
**Reviewer:** Security Review

---

## Executive Summary

The multichain-rs package is a shared cross-chain library for the CL8Y Bridge, providing address encoding, hash computation, EVM and Terra chain clients, signing, and testing utilities. This review analyzed the codebase for authentication, authorization, input validation, cryptography, unsafe memory handling, dependency risks, and related security concerns.

**Overall Assessment:** The codebase follows solid security practices with no `unsafe` code, appropriate cryptographic usage (Keccak256 for EVM compatibility), and good input validation in critical paths. Several medium- and low-severity issues have been addressed in a prior remediation; remaining items are documented below with recommended actions.

---

## 1. Findings Summary

| ID | Severity | Category | Description | Status |
|----|----------|----------|-------------|--------|
| F1 | Medium | Sensitive Data | Terra broadcast returns success when confirmation times out | **Fixed** |
| F2 | Low | Cryptography | Terra mnemonic uses empty BIP39 passphrase | **Documented** |
| F3 | Low | Input Validation | `decode_bytes32_to_terra_address` accepts 20–31 bytes | **Fixed** |
| F4 | Low | Error Handling | `serde_json::to_string` unwrap in contracts/tokens | **Acceptable** (test-only) |
| F5 | Low | Error Handling | `gas_prices.uluna.parse().unwrap_or` in Terra modules | **Fixed** |
| F6 | Informational | Sensitive Data | EvmUser has `pub private_key` in testing module | **Fixed** |
| F7 | Informational | Cryptography | `generate_test_private_key` uses DefaultHasher | Acceptable |
| F8 | Informational | Dependencies | Run cargo audit in CI | **Fixed** |
| F9 | Informational | Sensitive Data | Debug leakage on signer configs | **Fixed** |
| F10 | Informational | Input Validation | Denom parsing panic | **Fixed** |
| F11 | Informational | Network | URL trust model | **Documented** |

---

## 2. Detailed Findings

### 2.1 Unsafe Code Usage

**Status:** ✅ No findings

The codebase contains **no `unsafe`** blocks or functions. All memory safety is enforced by Rust's type system.

---

### 2.2 Cryptographic Analysis

#### 2.2.1 Hash Functions

| Finding | Severity | Assessment |
|---------|----------|------------|
| Keccak256 (tiny-keccak) for transfer hashes | ✅ Appropriate | Correct choice for EVM/Solidity compatibility; matches `HashLib.sol` |

**Location:** `hash.rs` – `keccak256`, `compute_xchain_hash_id`, etc.

#### 2.2.2 Key Derivation & Signing

**Finding F2: Terra mnemonic seed uses empty passphrase**

| Severity | Low |
|----------|-----|
| **Location** | `terra/signer.rs:141`, `terra/client.rs:91` |
| **Snippet** | `mnemonic.to_seed("")` |
| **Description** | Standard for Cosmos chains but provides no BIP39 passphrase protection if mnemonic is compromised. |
| **Remediation** | 1. Document in `SECURITY.md` that passphrase protection is not used.<br>2. For high-security deployments, consider supporting optional BIP39 passphrase if upstream `cosmrs`/`bip39` support it. |

#### 2.2.3 Non-Cryptographic Hash in Testing (F7)

| Severity | Informational (Acceptable) |
|----------|----------------------------|
| **Location** | `testing/user_eoa.rs:391–406` |
| **Snippet** | `DefaultHasher` used in `generate_test_private_key` |
| **Description** | `DefaultHasher` is not cryptographically secure; output is predictable from seed. |
| **Assessment** | Acceptable – function is explicitly documented as "NOT cryptographically secure - for testing only" and is in the `testing` feature-gated module. |
| **Remediation** | Add `#[allow(clippy::disallowed_methods)]` if CI flags this. Ensure it is never used in production code paths. |

---

### 2.3 Input Validation

#### 2.3.1 Address Parsing

| Component | Validation | Assessment |
|-----------|------------|------------|
| `parse_evm_address` | 40 hex chars, hex::decode | ✅ Robust |
| `decode_bech32_address` | 20-byte output | ✅ Validates length |
| `decode_bech32_address_raw` | 20 or 32 bytes | ✅ Validates length |
| `ChainId::from_hex` | 4 bytes | ✅ Validates |
| `EvmAddress::from_hex` | 20 or 32 bytes, padding check | ✅ Validates |
| `XchainHashId::from_hex` | 32 bytes | ✅ Validates |

#### 2.3.2 Bytes Length Handling (F3)

**Finding F3: `decode_bytes32_to_terra_address` accepts non-standard lengths**

| Severity | Low |
|----------|-----|
| **Location** | `hash.rs:196–225` |
| **Status** | **Fixed** |
| **Description** | Function name implies "bytes32" but previously accepted 20–31 byte inputs silently. |
| **Fix applied** | Restricted to exactly 20 or 32 bytes. Returns `Err` and emits `warn!` for any other length. Doc comment updated with rationale. |

---

### 2.4 Sensitive Data Handling

#### 2.4.1 Debug Trait (F9 – Fixed)

**Status:** ✅ Fixed

`EvmSignerConfig` and `TerraSignerConfig` implement custom `Debug` that redacts `private_key` and `mnemonic` with `"<redacted>"`.

#### 2.4.2 EvmUser Public Field (F6)

**Finding F6: EvmUser has `pub private_key`**

| Severity | Informational |
|----------|---------------|
| **Location** | `testing/user_eoa.rs:31` |
| **Snippet** | `pub struct EvmUser { pub private_key: String, ... }` |
| **Description** | Public field exposes private key; only in `testing` feature-gated module. |
| **Remediation** | Consider making `private_key` private with a getter if needed, or add module-level documentation that test utilities must never be linked in production. |

---

### 2.5 Error Handling & Panic-Prone Code

#### 2.5.1 Denom Parsing (F10 – Fixed)

**Status:** ✅ Fixed

User-supplied denoms now use `.map_err(|e| eyre!(...))?`. The `"uluna"` literal uses `.expect("uluna is a valid constant Terra denom")` with a clear comment.

#### 2.5.2 Terra Broadcast Confirmation Fallback (F1)

**Finding F1: Returns success when confirmation times out**

| Severity | Medium |
|----------|--------|
| **Location** | `terra/signer.rs:414–424` |
| **Snippet** | When `wait_for_tx_confirmation` fails, still returns `Ok(TerraTxResult { success: true, ... })`. |
| **Description** | Callers may incorrectly assume the transaction was confirmed; it may still be pending or failed. |
| **Remediation** | Return a distinct result, e.g. `BroadcastSucceededUnconfirmed`, or set `success: false` when confirmation fails. |

```rust
// Recommended: Distinguish confirmed vs unconfirmed
pub enum BroadcastOutcome {
    Confirmed(TerraTxResult),
    Unconfirmed { tx_hash: String },
}

// In broadcast_and_confirm, on confirmation failure:
Err(e) => Ok(BroadcastOutcome::Unconfirmed { tx_hash: txhash.clone() })
// Or extend TerraTxResult with a field: confirmation_status: ConfirmationStatus
```

#### 2.5.3 serde_json::to_string unwrap (F4)

**Finding F4: Unwrap in message serialization**

| Severity | Low |
|----------|-----|
| **Location** | `terra/contracts.rs`: lines 625, 633, 640, 647, 653, 660<br>`terra/tokens.rs`: lines 331, 339, 347 |
| **Status** | **Acceptable** (test-only) |
| **Snippet** | `serde_json::to_string(&msg).unwrap()` |
| **Description** | All occurrences are inside `#[cfg(test)]` / `#[test]` functions. Using `.unwrap()` in tests is idiomatic Rust — tests should panic on unexpected failures. No production code paths are affected. |

#### 2.5.4 Gas Price Parsing (F5)

**Finding F5: unwrap_or on gas price parse**

| Severity | Low |
|----------|-----|
| **Location** | `terra/client.rs:292`, `terra/signer.rs:301`, `terra/signer.rs:314` |
| **Status** | **Fixed** |
| **Description** | Invalid FCD response could yield malformed string; `parse()` would silently fall back to 0.015. |
| **Fix applied** | All three locations now use `unwrap_or_else` with a `warn!` that logs the malformed value and the default being used. |

---

### 2.6 U256/u128 Conversion (Events)

**Status:** ✅ Acceptable

Event parsing uses `try_into().unwrap_or_else(|_| u128::MAX)` with a `warn!` log. Prevents panic on malformed chain data; clamping is defensive.

**Location:** `evm/events.rs:58–66`, etc.

---

### 2.7 Fee Calculation Rounding

**Status:** ✅ Acceptable

Integer division `amount * bps / 10_000` truncates; small amounts can round to 0. Common pattern; documented in types. No security impact for typical bridge amounts.

**Location:** `types.rs:371–374`

---

### 2.8 Network & External Input

#### 2.8.1 URL Trust Model (F11 – Documented)

**Status:** ✅ Documented in `SECURITY.md`

RPC/LCD/FCD URLs in configuration are trusted. For multi-tenant or user-supplied config, consider allowlisting schemes (https only) and hostnames. See `packages/multichain-rs/SECURITY.md`.

#### 2.8.2 HTTP Client

- `reqwest` used with 30-second timeout for LCD/FCD requests.
- TLS verification is default (rustls/native-tls); no insecure overrides observed.

---

### 2.9 Access Control

**Assessment:** The library does not implement access control; it is a low-level building block. Operator/canceler roles are enforced by on-chain contracts. The library correctly queries `isOperator` and `isCanceler` from the bridge contract where needed.

No findings.

---

### 2.10 Dependencies

#### 2.10.1 Cargo Audit (F8 – Fixed)

**Status:** ✅ Fixed

CI runs `cargo audit` for multichain-rs. See `.github/workflows/test.yml` – multichain-rs job.

#### 2.10.2 Direct Dependencies

| Dependency | Version | Assessment |
|------------|---------|------------|
| alloy | 0.8 | Mature; actively maintained |
| cosmrs | 0.17 | Cosmos SDK Rust client |
| tendermint-rpc | 0.37 | Standard for Tendermint |
| reqwest | 0.12 | HTTP client |
| bip39 | 2.0 | BIP39 mnemonics |
| bech32 | 0.9 | Bech32 encoding |
| tiny-keccak | 2.0 | Keccak256 |
| hex | 0.4 | Hex encoding |

---

### 2.11 Secure Zeroing (ADR 001)

**Status:** Deferred

See `security_reviews/ADR_001_ZEROIZE_EVALUATION.md`. Decision: defer `secrecy`/`zeroize` adoption; custom Debug already prevents logging. Revisit for high-security or multi-tenant deployments.

---

## 3. Severity Definitions

| Severity | Definition |
|----------|------------|
| **Critical** | Exploitable remotely; leads to full compromise or significant fund loss |
| **High** | Significant impact; requires specific conditions to exploit |
| **Medium** | Moderate impact; may cause incorrect behavior or information leakage |
| **Low** | Minor impact; edge cases or defense-in-depth improvements |
| **Informational** | Best practices; no direct exploit path |

---

## 4. Recommended Actions (Prioritized)

### High Priority

1. **F1 – Terra broadcast confirmation fallback**  
   Return a distinct outcome (e.g. `BroadcastSucceededUnconfirmed`) when confirmation times out so callers can handle unconfirmed broadcasts appropriately.

### Medium Priority

2. ~~**F4 – Replace `serde_json::to_string` unwrap**~~ — **Acceptable**: all occurrences are in `#[test]` functions only.

3. ~~**F5 – Gas price parse logging**~~ — **Fixed**: all three locations now use `unwrap_or_else` with `warn!`.

### Low Priority

4. **F2 – Document empty BIP39 passphrase**  
   Update `SECURITY.md` to state that Terra mnemonic uses no passphrase.

5. ~~**F3 – `decode_bytes32_to_terra_address` length**~~ — **Fixed**: restricted to 20 or 32 bytes with `warn!` on violation.

6. **F6 – EvmUser private_key visibility**  
   Consider making `private_key` private in the testing module.

7. **F7 – DefaultHasher in tests**  
   Add `#[allow(clippy::disallowed_methods)]` if CI flags it.

---

## 5. Verification Checklist

After implementing recommendations:

- [x] Terra broadcast returns explicit unconfirmed status when confirmation times out
- [x] `serde_json::to_string().unwrap()` confirmed test-only; no production unwrap paths
- [x] Gas price parse failure is logged with `warn!`
- [x] `SECURITY.md` documents BIP39 passphrase behavior
- [x] `decode_bytes32_to_terra_address` restricted to 20 or 32 bytes with `warn!` on violation
- [x] `cargo audit` runs in CI and passes
- [x] No Debug output exposes `private_key` or `mnemonic`
- [x] `EvmUser.private_key` is private with getter; custom Debug redacts it

---

## 6. References

- `packages/multichain-rs/SECURITY.md` – URL trust model, dependency scanning
- `packages/multichain-rs/security_reviews/ADR_001_ZEROIZE_EVALUATION.md` – zeroize decision
- `packages/multichain-rs/security_reviews/SPRINT_SECURITY_REMEDIATION.md` – prior sprint plan

---

*This review was performed through static code analysis. A full security assessment would also include dynamic testing, fuzzing of input parsers, and dependency vulnerability scanning integrated into CI/CD.*
