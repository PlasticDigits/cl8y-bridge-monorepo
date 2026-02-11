# Security Review: multichain-rs Package

**Date:** February 11, 2025  
**Package:** multichain-rs (v0.1.0)  
**Reviewer:** Security Review  
**Scope:** Full codebase analysis for common vulnerabilities

---

## Executive Summary

The multichain-rs package is a shared cross-chain library for the CL8Y Bridge, providing address encoding, hash computation, EVM and Terra chain clients, signing, and testing utilities. The review identified **no critical vulnerabilities** but found several **medium** and **low** severity issues, primarily around sensitive data handling, input validation, and defensive coding practices.

**Overall Assessment:** The codebase follows solid security practices with no `unsafe` code, appropriate cryptographic usage (Keccak256 for EVM compatibility), and good input validation in critical paths. Areas for improvement center on sensitive data protection, panic avoidance in production paths, and dependency hygiene.

---

## 1. Unsafe Code Usage

**Status:** ✅ No findings

The codebase contains **no `unsafe`** blocks or functions. All memory safety is enforced by Rust's type system.

---

## 2. Cryptographic Analysis

### 2.1 Hash Functions

| Finding | Severity | Status |
|---------|----------|--------|
| Keccak256 (tiny-keccak) for transfer hashes | ✅ Appropriate | Correct choice for EVM/Solidity compatibility |

The `hash` module correctly uses Keccak256 for transfer hash computation, matching the EVM contract's `HashLib.sol` implementation. Cross-chain hash parity is well-tested.

### 2.2 Key Derivation & Signing

| Finding | Severity | Description |
|---------|----------|-------------|
| Terra mnemonic seed uses empty passphrase | Low | `mnemonic.to_seed("")` - Standard for Cosmos chains but provides no passphrase protection if mnemonic is compromised |

**Location:** `terra/signer.rs:128`, `terra/client.rs:91`

**Recommendation:** Document that passphrase protection is not used. For high-security deployments, consider supporting optional BIP39 passphrase if upstream cosmrs/bip39 support it.

### 2.3 Non-Cryptographic Hash in Testing

| Finding | Severity | Description |
|---------|----------|-------------|
| `generate_test_private_key` uses DefaultHasher | Low | `DefaultHasher` is not cryptographically secure; output is predictable from seed |

**Location:** `testing/user_eoa.rs:391-406`

**Status:** Acceptable – Function is explicitly documented as "NOT cryptographically secure - for testing only" and is in the `testing` feature-gated module.

**Recommendation:** Add `#[allow(clippy::disallowed_methods)]` or similar if CI flags this, and ensure it is never exposed or used in production code paths.

---

## 3. Input Validation

### 3.1 Address Parsing

| Component | Validation | Assessment |
|-----------|------------|------------|
| `parse_evm_address` | 40 hex chars, hex::decode | ✅ Robust – Invalid hex returns Err |
| `decode_bech32_address` | 20-byte output | ✅ Validates length |
| `decode_bech32_address_raw` | 20 or 32 bytes | ✅ Validates length |
| `ChainId::from_hex` | 4 bytes | ✅ Validates |
| `EvmAddress::from_hex` | 20 or 32 bytes, padding check | ✅ Validates |
| `WithdrawHash::from_hex` | 32 bytes | ✅ Validates |

### 3.2 Panic-Prone Input Handling

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| `denom.parse().expect("invalid coin denom")` | **Medium** | `terra/client.rs:305` | Panics if funds contain invalid denom string |
| `denom.parse().unwrap()` | **Medium** | `terra/signer.rs:413` | Same issue in Terra signer |

**Recommendation:** Replace with:
```rust
denom.parse().map_err(|e| eyre!("Invalid coin denom '{}': {}", denom, e))?
```

### 3.3 Bytes Length Handling

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| `decode_bytes32_to_terra_address` | Low | `hash.rs:195-210` | Accepts 20–31 byte inputs; for non-32 lengths, left-pads and extracts. Logic is correct but non-standard for "bytes32" |

**Recommendation:** Consider restricting to exactly 20 or 32 bytes for clarity, or document the padding behavior.

---

## 4. Sensitive Data Handling

### 4.1 Debug Trait Leakage

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| EvmSignerConfig derives Debug | **Medium** | `evm/signer.rs:26` | Contains `private_key: String`; `{:?}` or logging could leak key |
| TerraSignerConfig derives Debug | **Medium** | `terra/signer.rs:37` | Contains `mnemonic: String`; same risk |
| EvmUser has `pub private_key` | **Medium** | `testing/user_eoa.rs:31` | Public field; only in testing module but still sensitive |

**Recommendation:**
- Implement custom `Debug` for config structs that omits `private_key` and `mnemonic` (e.g., `DebugStruct::field("private_key", &"<redacted>")`).
- Consider using `secrecy` or `zeroize` for sensitive fields to prevent accidental logging and support secure zeroing.

### 4.2 No Secure Zeroing

| Finding | Severity | Description |
|---------|----------|-------------|
| Private keys and mnemonics not zeroed on drop | Low | Strings holding keys remain in memory until GC; no explicit zeroing |

**Recommendation:** For high-security deployments, consider `SecretString`/`zeroize` patterns. Evaluate trade-offs with allocation patterns.

---

## 5. Access Control

**Assessment:** The library does not implement access control; it is a low-level building block. Access control (operator/canceler roles, etc.) is enforced by on-chain contracts. The library correctly queries `isOperator` and `isCanceler` from the bridge contract where needed.

No findings.

---

## 6. Dependency Risks

### 6.1 Direct Dependencies (Cargo.toml)

| Dependency | Version | Assessment |
|------------|---------|------------|
| alloy | 0.8 | Mature Ethereum library; actively maintained |
| cosmrs | 0.17 | Cosmos SDK Rust client |
| tendermint-rpc | 0.37 | Standard for Tendermint chains |
| reqwest | 0.12 | HTTP client; recent |
| bip39 | 2.0 | BIP39 mnemonics |
| bech32 | 0.9 | Bech32 encoding |
| tiny-keccak | 2.0 | Keccak256 |
| hex | 0.4 | Hex encoding |
| base64 | 0.22 | Base64 encoding |

### 6.2 Audit Status

**Recommendation:** Run `cargo install cargo-audit` and `cargo audit` regularly in CI. No known CVEs were reported during this review, but dependency scanning should be automated.

---

## 7. Logical Flaws & Design Considerations

### 7.1 Fee Calculation Rounding

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| Integer division in fee calculation | Low | `types.rs:371-374` | `amount * bps / 10_000` truncates; small amounts can round to 0 |

**Status:** Common pattern; documented in types. No security impact for typical bridge amounts.

### 7.2 Terra Broadcast Confirmation Fallback

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| Returns success when confirmation times out | Low | `terra/signer.rs:414-424` | If `wait_for_tx_confirmation` fails, still returns `Ok` with `success: true` |

**Recommendation:** Consider returning a distinct result (e.g., `BroadcastSucceededUnconfirmed`) so callers can handle unconfirmed broadcasts appropriately.

### 7.3 U256/u128 Conversion Clamping

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| Event parsing clamps overflow to u128::MAX | Low | `evm/events.rs:58-66`, etc. | `try_into().unwrap_or_else(|_| u128::MAX)` – correct defensive handling |

**Status:** Acceptable – prevents panic on malformed chain data.

---

## 8. Network & External Input

### 8.1 URL Handling

| Finding | Severity | Description |
|---------|----------|-------------|
| RPC/LCD URLs not validated | Low | Malicious config could direct requests to internal services (SSRF) |

**Recommendation:** Document that URL configuration is trusted. For multi-tenant or config-from-user deployments, consider allowlisting schemes (e.g., https only) and hostnames.

### 8.2 HTTP Client

reqwest is used with a 30-second timeout for LCD/FCD requests. No credential leakage in URLs was observed.

---

## 9. Error Handling & Information Leakage

- Errors use `eyre` and generally avoid exposing internal state.
- Stack traces in development can include configuration; ensure production builds do not enable `RUST_BACKTRACE` or similar for sensitive services.

---

## 10. Testing & Test-Only Code

- `EvmUser` and `TerraUser` in the testing module handle private keys and mnemonics; correctly feature-gated.
- Test private keys (e.g., Anvil default) are well-known and allowlisted in `.gitleaks.toml`.
- Integration tests use `std::env::var` for configuration; appropriate for CI.

---

## Summary of Findings by Severity

| Severity | Count | Items |
|----------|-------|-------|
| Critical | 0 | - |
| High | 0 | - |
| Medium | 3 | Debug leakage (2), panic on invalid denom (1) |
| Low | 8 | Empty passphrase, test key generator, URL validation, fee rounding, confirmation fallback, etc. |
| Informational | 2 | Dependency audit, zeroize consideration |

---

## Remediation Priority

1. **High priority:** Implement custom `Debug` for `EvmSignerConfig` and `TerraSignerConfig` to avoid logging secrets.
2. **Medium priority:** Replace `expect`/`unwrap` for denom parsing with proper error propagation.
3. **Low priority:** Add `cargo audit` to CI; document URL trust model; consider `secrecy`/`zeroize` for long-term hardening.

---

## Conclusion

The multichain-rs package demonstrates good security practices for a cross-chain library. No critical or high-severity vulnerabilities were found. The main improvements are around preventing accidental leakage of private keys/mnemonics via Debug and ensuring production code paths avoid panics on invalid input. Implementing the high- and medium-priority recommendations will significantly strengthen the security posture.

---

*This review was performed through static code analysis. A full security assessment would also include dynamic testing, fuzzing of input parsers, and dependency vulnerability scanning integrated into CI/CD.*
