# Sprint: multichain-rs Security Review Remediation

## Overview

**Goal**: Address the 3 prioritized recommendations from the security review of multichain-rs (SECURITY_REVIEW_2025-02-11.md).

**Source**: `packages/multichain-rs/security_reviews/SECURITY_REVIEW_2025-02-11.md`

**Target State**: All high- and medium-priority issues resolved; low-priority hardening in place.

---

## Recommended Order (by Priority)

| Order | Recommendation | Story Points | Priority |
|-------|----------------|--------------|----------|
| 1 | Custom Debug for Signer Configs (sensitive data leakage) | 3 | High |
| 2 | Replace panic-prone denom parsing with proper error propagation | 2 | Medium |
| 3 | CI audit + URL trust docs + zeroize consideration | 3 | Low |

---

## Backlog Items

### Backlog Item 1: Custom Debug for EvmSignerConfig and TerraSignerConfig

**Source Recommendation**: Remediation Priority #1 (High)

**Problem**: Both `EvmSignerConfig` (contains `private_key`) and `TerraSignerConfig` (contains `mnemonic`) derive `Debug`. Using `{:?}` or logging could accidentally leak secrets in logs, stack traces, or error messages.

**Locations**:
- `packages/multichain-rs/src/evm/signer.rs:26` – `EvmSignerConfig`
- `packages/multichain-rs/src/terra/signer.rs:37` – `TerraSignerConfig`
- `packages/multichain-rs/src/testing/user_eoa.rs:31` – `EvmUser` has `pub private_key` (testing module; optional hardening)

#### Acceptance Criteria

- [ ] **AC1**: `EvmSignerConfig` no longer derives `Debug`; custom `impl Debug` implemented that omits `private_key` (e.g., `DebugStruct::field("private_key", &"<redacted>")`)
- [ ] **AC2**: `TerraSignerConfig` no longer derives `Debug`; custom `impl Debug` implemented that omits `mnemonic` (e.g., `DebugStruct::field("mnemonic", &"<redacted>")`)
- [ ] **AC3**: `Clone` is preserved for both configs
- [ ] **AC4**: All existing tests pass; no regressions in operator/canceler integrations that use these configs
- [ ] **AC5** (optional): `EvmUser` in testing module: either redact in Debug or document that it is test-only and never logged in production

#### Implementation Notes

```rust
impl std::fmt::Debug for EvmSignerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmSignerConfig")
            .field("rpc_url", &self.rpc_url)
            .field("chain_id", &self.chain_id)
            .field("private_key", &"<redacted>")
            .finish()
    }
}
```

#### Story Points: 3

#### Dependencies: None

---

### Backlog Item 2: Replace panic-prone denom parsing with proper error propagation

**Source Recommendation**: Remediation Priority #2 (Medium)

**Problem**: `denom.parse().expect("invalid coin denom")` and `denom.parse().unwrap()` cause panics if the denom string is invalid. These are in production code paths and could crash the operator/canceler.

**Locations**:
- `packages/multichain-rs/src/terra/client.rs:305` – `denom.parse().expect("invalid coin denom")`
- `packages/multichain-rs/src/terra/client.rs:335` – `"uluna".parse().unwrap()`
- `packages/multichain-rs/src/terra/signer.rs:413` – `denom.parse().unwrap()` (in funds mapping)
- `packages/multichain-rs/src/terra/signer.rs:440` – `"uluna".parse().unwrap()`

#### Acceptance Criteria

- [ ] **AC1**: All four locations use proper error propagation (e.g., `map_err` with `eyre!`)
- [ ] **AC2**: Callers receive `Result`; no panics on invalid denom
- [ ] **AC3**: Error messages include the invalid denom value for debugging
- [ ] **AC4**: For the `"uluna"` literals: either keep `.parse().expect("uluna is valid")` (since it’s a constant) or use `Denom::from_str("uluna").unwrap()` with a comment that it’s a known-valid constant; preference is to keep as-is with a clarifying comment, or wrap in a helper like `uluna_denom()` that returns `Denom`

**Recommended pattern** (from security review):

```rust
denom.parse().map_err(|e| eyre!("Invalid coin denom '{}': {}", denom, e))?
```

For `"uluna"` literal: use `"uluna".parse().expect("uluna is a valid constant denom")` or a small `const ULUNA: &str = "uluna"` helper—either is acceptable since failure is impossible.

#### Story Points: 2

#### Dependencies: None

---

### Backlog Item 3: CI cargo audit + URL trust documentation + zeroize consideration

**Source Recommendation**: Remediation Priority #3 (Low)

**Problem**: Dependency vulnerability scanning is not automated; URL trust model is undocumented; no secure zeroing for sensitive strings.

#### Sub-task 3a: Add cargo audit to CI

##### Acceptance Criteria

- [ ] **AC3a-1**: `cargo install cargo-audit` and `cargo audit` run in CI for `packages/multichain-rs`
- [ ] **AC3a-2**: CI job fails if `cargo audit` reports vulnerabilities (or uses `--deny warnings` where appropriate)
- [ ] **AC3a-3**: multichain-rs is included in test.yml workflow (add path filter if needed) or a dedicated multichain-rs CI job exists

##### Story Points: 1

#### Sub-task 3b: Document URL trust model

##### Acceptance Criteria

- [ ] **AC3b-1**: Documentation added (in `multichain-rs/README.md` or `docs/`) stating that RPC/LCD/FCD URLs in config are trusted
- [ ] **AC3b-2**: Note added that for multi-tenant or user-supplied config, consider allowlisting schemes (https only) and hostnames

##### Story Points: 0.5

#### Sub-task 3c: Evaluate secrecy/zeroize for sensitive fields

##### Acceptance Criteria

- [ ] **AC3c-1**: ADR or brief doc evaluating `secrecy`/`zeroize` for `private_key` and `mnemonic` fields
- [ ] **AC3c-2**: Decision recorded: adopt now vs. defer to future sprint; if adopt, implement `SecretString` or similar for config structs

##### Story Points: 1.5

#### Total Story Points for Item 3: 3

#### Dependencies: None

---

## Sprint Summary

| Item | Description | Story Points |
|------|-------------|--------------|
| 1 | Custom Debug for signer configs | 3 |
| 2 | Denom parsing error propagation | 2 |
| 3 | CI audit + URL docs + zeroize evaluation | 3 |
| **Total** | | **8** |

---

## Suggested Sprint Execution

1. **Day 1–2**: Backlog Item 1 (Custom Debug) – highest security impact.
2. **Day 2–3**: Backlog Item 2 (Denom parsing) – medium impact, straightforward fix.
3. **Day 3–4**: Backlog Item 3 – sub-task 3a (CI audit) first for immediate value; 3b (URL docs) and 3c (zeroize eval) in parallel or sequence.

---

## Verification Checklist

After all items are complete:

- [ ] `cargo test` passes in `packages/multichain-rs`
- [ ] `cargo clippy` passes
- [ ] Operator and canceler (if they depend on multichain-rs) still build and run
- [ ] No `Debug` output of config structs exposes `private_key` or `mnemonic`
- [ ] Invalid denom strings return `Err` instead of panicking
- [ ] `cargo audit` runs in CI and reports clean (or known-acceptable issues documented)

---

## References

- Security review: `packages/multichain-rs/security_reviews/SECURITY_REVIEW_2025-02-11.md`
- Existing CI: `.github/workflows/test.yml` (multichain-rs may need to be added to path filters or a new job)
- `secrecy` crate: https://docs.rs/secrecy/
- `zeroize` crate: https://docs.rs/zeroize/
