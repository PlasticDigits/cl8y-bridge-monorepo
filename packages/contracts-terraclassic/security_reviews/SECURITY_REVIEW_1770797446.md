# Security Review: contracts-terraclassic Bridge Contract

**Date:** 2026-02-11 (UTC)
**Epoch:** 1770797446
**Reviewer:** Automated Security Review
**Package:** `packages/contracts-terraclassic` (CL8Y Bridge CosmWasm contract v2.0.0)

---

## Executive Summary

This review covers the full Terra Classic bridge contract implementation — all execute handlers, query handlers, storage definitions, hash computation, fee management, address codec, and the complete test suite. The review focuses on the V2 watchtower withdrawal flow, rate limiting, RBAC enforcement, decimal normalization, fee accounting, and cross-chain hash parity.

Three medium-severity findings and five low/informational findings were identified. No critical or high-severity vulnerabilities were found. The most significant findings relate to missing `enabled` checks in the withdrawal submission path and accounting inconsistencies in the emergency recovery flow.

### Findings Summary

| Severity | Count | Summary |
|----------|-------|---------|
| Critical | 0 | — |
| High | 0 | — |
| Medium | 3 | Missing chain/token enabled checks; recovery accounting; simulation fee mismatch |
| Low/Info | 5 | Silent fund loss; misleading errors; no expiry; missing token type guard; doc alignment |

---

## 1. Scope and Method

### In-Scope Code

| File | Lines | Purpose |
|------|-------|---------|
| `bridge/src/contract.rs` | 480 | Entry points: instantiate, execute, query, migrate |
| `bridge/src/execute/withdraw.rs` | 691 | V2 withdrawal flow (submit, approve, cancel, uncancel, execute) |
| `bridge/src/execute/outgoing.rs` | 591 | Outgoing deposits (native lock, CW20 lock, CW20 burn) |
| `bridge/src/execute/config.rs` | 858 | Configuration management (chains, tokens, operators, fees) |
| `bridge/src/execute/admin.rs` | 168 | Admin operations (pause, admin transfer, recovery) |
| `bridge/src/state.rs` | 365 | Storage structures and constants |
| `bridge/src/msg.rs` | 923 | Message types and response structs |
| `bridge/src/error.rs` | 208 | Error enum |
| `bridge/src/query.rs` | 862 | Query handlers |
| `bridge/src/hash.rs` | 1062 | Hash computation and cross-chain parity |
| `bridge/src/fee_manager.rs` | 388 | Fee calculation with CL8Y discounts |
| `bridge/src/address_codec.rs` | 500 | Universal address encoding/decoding |

### Test Files Reviewed

| File | Tests | Purpose |
|------|-------|---------|
| `bridge/tests/integration.rs` | — | Core integration tests |
| `bridge/tests/test_withdraw_flow.rs` | — | V2 withdrawal lifecycle |
| `bridge/tests/test_chain_registry.rs` | — | Chain registration/validation |
| `bridge/tests/test_fee_system.rs` | — | Fee system (standard, discount, custom) |
| `bridge/tests/test_hash_parity.rs` | — | Cross-chain hash verification |
| `bridge/tests/test_incoming_token_registry.rs` | — | Incoming token mapping validation |
| `bridge/tests/test_address_codec.rs` | — | Address encoding/decoding |

### Validation Performed

- Full static review of all source modules
- `cargo test` in `packages/contracts-terraclassic`: **192 tests passed, 0 failed**
- Cross-referencing RBAC enforcement across all execute paths
- Comparison of outgoing vs incoming path validation consistency
- Fee flow analysis (V1 legacy vs V2 FEE_CONFIG)
- Rate limit enforcement scope verification
- Hash parity review against EVM contract expectations

---

## 2. Security Posture

### 2.1 Withdrawal Flow Controls (Strong)

The V2 watchtower pattern is well-implemented:

- `WithdrawSubmit` enforces amount > 0, source chain existence, token support, incoming mapping presence with enabled check, and duplicate hash prevention.
- `WithdrawApprove` requires operator or admin role and prevents double approval. Marks nonce used per source chain.
- `WithdrawCancel` is restricted to canceler-only (correct RBAC separation from operators/admin).
- `WithdrawUncancel` requires operator/admin and correctly resets the cancel window timer.
- `WithdrawExecuteUnlock`/`WithdrawExecuteMint` require approved + not cancelled + cancel window passed + not paused + correct token type.
- Cancel window uses strict `>` comparison (`env.block.time.seconds() > window_end`), preventing off-by-one execution.

### 2.2 Rate Limiting (Adequate)

- Applied at withdrawal execution (both unlock and mint paths).
- Per-transaction and per-period (24h) limits enforced.
- Default rate limit is 0.1% of supply when no explicit config, or 100 ether for zero supply.
- Window accounting updated atomically within execution.
- Rate limits are **not** applied to outgoing deposits (by design — outgoing has min/max bounds).

### 2.3 Decimal Normalization (Strong)

- Uses `Uint256`/`Uint512` intermediate math for scale-up operations.
- Errors on overflow (`AmountOverflow`) rather than silently clamping.
- Scale-down uses integer division (truncation toward zero) — acceptable for bridge accounting.

### 2.4 Hash Computation (Strong)

- 7-field unified hash matches EVM `HashLib.computeXchainHashId` layout exactly.
- Chain IDs left-aligned in bytes32, amounts/nonces left-padded as uint256 big-endian.
- Extensive cross-chain parity tests with known vectors.
- Native token encoding uses `keccak256(denom)`, CW20 uses canonical address left-padded.

### 2.5 Admin and Recovery Controls (Adequate, see Finding 3.2)

- Admin transfer uses 7-day timelock with pending-admin acceptance.
- Emergency recovery requires admin + paused state.
- Pause gates on both outgoing deposits and withdrawal execution/submission.

### 2.6 Fee System (Adequate, see Finding 3.3)

- V2 fee config with CL8Y discount and custom per-account fees.
- Fee priority: custom > CL8Y discount > standard (correct).
- Max fee capped at 100 bps (1%).
- Fee deducted before locked balance tracking (net amount locked).

### 2.7 RBAC Enforcement (Strong)

- Admin: pause, unpause, propose admin, config changes, recovery, add/remove operators/cancelers.
- Operator: approve, uncancel withdrawals.
- Canceler: cancel withdrawals only (correct separation).
- Anyone: submit withdrawals, execute after window.

---

## 3. Findings

### 3.1 [MEDIUM] WithdrawSubmit Missing Source Chain and Token `enabled` Checks

**Location:** `bridge/src/execute/withdraw.rs` lines 91–117

**Description:**

`execute_withdraw_submit` checks that the source chain **exists** in `CHAINS` but does **not** check the `chain.enabled` flag. It also checks that the token exists in `TOKENS` but does not check `token_config.enabled`.

In contrast, all outgoing deposit paths (`execute_deposit_native`, `execute_deposit_cw20_lock`, `execute_deposit_cw20_burn`) consistently check both chain existence **and** `chain.enabled`, as well as `token_config.enabled`.

```rust
// withdraw.rs line 91-95 — only checks existence, not enabled
CHAINS
    .may_load(deps.storage, &src_chain_bytes)?
    .ok_or(ContractError::ChainNotRegistered { ... })?;
// Missing: if !chain.enabled { return Err(...); }

// withdraw.rs line 83-88 — only checks existence, not enabled
let token_config = TOKENS
    .may_load(deps.storage, token.clone())?
    .ok_or(ContractError::TokenNotSupported { ... })?;
// Missing: if !token_config.enabled { return Err(...); }
```

**Impact:** An admin disabling a chain or token would prevent new outgoing deposits to/for that chain/token, but incoming withdrawal submissions for that chain/token would still be accepted. While the withdrawal still requires operator approval (providing a secondary gate), this creates an inconsistency in the disable mechanism. A disabled chain/token should block operations in both directions.

**Recommendation:**

Add enabled checks after loading the chain config and token config in `execute_withdraw_submit`:

```rust
let chain = CHAINS
    .may_load(deps.storage, &src_chain_bytes)?
    .ok_or(ContractError::ChainNotRegistered { ... })?;
if !chain.enabled {
    return Err(ContractError::ChainNotRegistered { ... });
}

// After loading token_config:
if !token_config.enabled {
    return Err(ContractError::TokenNotSupported { token: token.clone() });
}
```

---

### 3.2 [MEDIUM] Emergency Recovery Does Not Update LOCKED_BALANCES

**Location:** `bridge/src/execute/admin.rs` lines 125–167

**Description:**

`execute_recover_asset` allows the admin to recover stuck assets when the bridge is paused. However, it sends tokens out of the contract without updating the `LOCKED_BALANCES` tracker. When the bridge is subsequently unpaused, the tracked locked balance will be higher than the contract's actual balance.

```rust
// admin.rs: sends tokens but never touches LOCKED_BALANCES
let messages: Vec<CosmosMsg> = match asset {
    AssetInfo::Native { denom } => {
        vec![CosmosMsg::Bank(BankMsg::Send { ... })]
    }
    AssetInfo::Cw20 { contract_addr } => { ... }
};
// No LOCKED_BALANCES update here
```

**Impact:** After recovery and unpausing, legitimate `WithdrawExecuteUnlock` operations will fail with `InsufficientLiquidity` even when the net tracked balance should have been reduced. This forces the admin to either:
1. Re-deposit equivalent tokens to restore the balance, or
2. Perform additional manual intervention.

The inconsistency between tracked and actual balance could also cause confusion during incident response.

**Recommendation:**

Either:
- (A) Update `LOCKED_BALANCES` in `execute_recover_asset` to subtract the recovered amount for native tokens, OR
- (B) Document that recovery is a destructive operation requiring re-initialization of locked balance tracking, OR
- (C) Add an admin function to manually reset/adjust `LOCKED_BALANCES` for a token.

---

### 3.3 [MEDIUM] `query_simulate_bridge` Uses Legacy Fee Instead of V2 FEE_CONFIG

**Location:** `bridge/src/query.rs` lines 259–282

**Description:**

`query_simulate_bridge` computes fee estimates using the legacy `config.fee_bps` field:

```rust
let fee_amount = amount.multiply_ratio(config.fee_bps as u128, 10000u128);
```

However, actual deposit operations (`execute_deposit_native`, `execute_deposit_cw20_lock`, `execute_deposit_cw20_burn`) use the V2 `FEE_CONFIG` via `calculate_fee()`, which includes CL8Y holder discounts and custom account fees. The simulation will return incorrect fee estimates for any user with a custom fee or CL8Y discount.

**Impact:** Frontend/tooling relying on `SimulateBridge` will show users incorrect fee amounts, leading to confusion about actual fees charged. This is particularly misleading for CL8Y token holders who expect discounted fees.

**Recommendation:**

Update `query_simulate_bridge` to accept an optional `depositor` parameter and use the V2 fee manager:

```rust
let fee_config = FEE_CONFIG.may_load(deps.storage)?
    .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));
let fee_amount = if let Some(depositor) = depositor {
    let addr = deps.api.addr_validate(&depositor)?;
    calculate_fee(deps, &fee_config, &addr, amount)?
} else {
    // Standard fee for unknown depositor
    calculate_fee_from_bps(amount, fee_config.standard_fee_bps)
};
```

---

### 3.4 [LOW] Non-uluna Native Tokens Sent With WithdrawSubmit Are Silently Lost

**Location:** `bridge/src/execute/withdraw.rs` lines 141–146

**Description:**

The operator gas tip extraction only looks for `"uluna"`:

```rust
let operator_gas = info
    .funds
    .iter()
    .find(|c| c.denom == "uluna")
    .map(|c| c.amount)
    .unwrap_or(Uint128::zero());
```

If a user accidentally sends other native tokens (e.g., `uusd`, IBC tokens), those funds are deposited into the contract but not tracked. They can only be recovered via `execute_recover_asset` which requires pausing the entire bridge.

**Impact:** User funds loss in edge cases. While most users will send uluna, accidental token sends result in effectively locked funds.

**Recommendation:**

Add a check that either (a) only uluna is sent, or (b) all sent tokens are summed or rejected:

```rust
// Option A: Reject non-uluna funds
for coin in &info.funds {
    if coin.denom != "uluna" {
        return Err(ContractError::InvalidAmount {
            reason: format!("Only uluna accepted as operator gas tip, got {}", coin.denom),
        });
    }
}
```

---

### 3.5 [LOW] WithdrawApprove Returns Misleading Error for Already-Approved Withdrawal

**Location:** `bridge/src/execute/withdraw.rs` lines 208–209

**Description:**

When a withdrawal is already approved (but not yet executed), the function returns `WithdrawAlreadyExecuted`:

```rust
if pending.approved {
    return Err(ContractError::WithdrawAlreadyExecuted); // Already approved
}
```

This is misleading — the withdrawal is approved, not executed. A dedicated error variant like `WithdrawAlreadyApproved` would be more accurate.

**Impact:** Operational confusion during debugging. Operators or monitoring systems may incorrectly interpret this as a double-execution rather than a duplicate approval attempt.

**Recommendation:**

Add a `WithdrawAlreadyApproved` error variant in `error.rs` and use it here.

---

### 3.6 [LOW] No Expiration or Cleanup for Unapproved Pending Withdrawals

**Location:** `bridge/src/execute/withdraw.rs`, `bridge/src/state.rs`

**Description:**

Once created via `WithdrawSubmit`, a `PendingWithdraw` record exists in `PENDING_WITHDRAWS` indefinitely if never approved. There is no expiration mechanism, time-to-live, or garbage collection. Additionally, the operator gas tip (uluna) remains locked in the contract.

**Impact:** Over time, stale entries accumulate in contract storage. Each entry occupies storage on-chain (estimated ~200+ bytes). In adversarial scenarios, an attacker could submit many cheap withdrawals (only paying gas + optional tip) to bloat contract storage.

**Recommendation:**

Consider adding:
- An expiration field to `PendingWithdraw` (e.g., 24-48 hours after submission).
- An `ExpireWithdraw` execute message that anyone can call to clean up expired, unapproved submissions and return the operator gas tip to the submitter.

---

### 3.7 [LOW] DepositNative Does Not Verify Token Type Is LockUnlock

**Location:** `bridge/src/execute/outgoing.rs` lines 41–210

**Description:**

`execute_deposit_native` does not check `token_config.token_type`. The CW20 paths (`execute_deposit_cw20_lock` and `execute_deposit_cw20_burn`) correctly enforce the expected token type:

```rust
// CW20 lock: verifies LockUnlock
if !matches!(token_config.token_type, TokenType::LockUnlock) {
    return Err(ContractError::InvalidTokenType { ... });
}
// CW20 burn: verifies MintBurn
if !matches!(token_config.token_type, TokenType::MintBurn) {
    return Err(ContractError::InvalidTokenType { ... });
}
```

But `execute_deposit_native` skips this check entirely. If a native token were incorrectly registered as `MintBurn`, the deposit would lock funds (correct for `LockUnlock`) but the corresponding withdrawal path would try `WithdrawExecuteMint` — which would mint new tokens without corresponding burns, inflating supply.

**Impact:** Low in practice since native tokens are unlikely to be registered as `MintBurn`, and admin is trusted. However, the asymmetry with CW20 paths creates a defense-in-depth gap.

**Recommendation:**

Add token type validation in `execute_deposit_native`:

```rust
if !matches!(token_config.token_type, TokenType::LockUnlock) {
    return Err(ContractError::InvalidTokenType {
        expected: "lock_unlock".to_string(),
        actual: token_config.token_type.as_str().to_string(),
    });
}
```

---

### 3.8 [LOW/INFO] Previous Review Doc Mismatches Still Partially Present

**Location:** `bridge/src/msg.rs`

**Description:**

The previous security review (epoch 20260211_075747) noted two documentation mismatches:
1. `WithdrawCancel` comment said "Canceler, Operator, or Admin" — should be "Canceler only"
2. `SetWithdrawDelay` docs mention minimum 60 seconds — runtime allows 15 seconds

The `WithdrawCancel` comment has been corrected. The `SetWithdrawDelay` documentation alignment should be verified to ensure consistency.

**Recommendation:** Confirm `SetWithdrawDelay` msg.rs docs match the runtime validation range of `15..=86400`.

---

## 4. Positive Security Notes

1. **Strong RBAC separation**: Canceler-only cancel, operator-only approve/uncancel, admin-only configuration. No role overlap in critical paths.
2. **Cancel window strictly enforced**: Uses `>` comparison preventing execution at exactly the window boundary.
3. **Nonce replay prevention**: `WITHDRAW_NONCE_USED` prevents double-approval per source chain nonce.
4. **Overflow-safe decimal normalization**: `Uint256`/`Uint512` math with explicit error on overflow.
5. **Comprehensive hash parity tests**: Cross-chain hash vectors verified against Solidity implementation.
6. **Admin timelock**: 7-day delay on admin transfer with pending-admin acceptance.
7. **CW20 code_id allowlisting**: Defense against malicious token contracts.
8. **Pause gates on critical paths**: Both submit and execute are blocked when paused.
9. **Rate limit enforcement on withdrawals**: Prevents rapid draining of bridge liquidity.
10. **Incoming token mapping validation**: Prevents withdrawals for unregistered source chain + token combinations.

---

## 5. Recommendations Summary

| Priority | Item | Finding | Status |
|----------|------|---------|--------|
| P1 | Add chain `enabled` check to `WithdrawSubmit` | 3.1 | Open |
| P1 | Add token `enabled` check to `WithdrawSubmit` | 3.1 | Open |
| P2 | Update `LOCKED_BALANCES` in `execute_recover_asset` or add manual adjustment | 3.2 | Open |
| P2 | Update `query_simulate_bridge` to use V2 FEE_CONFIG | 3.3 | Open |
| P3 | Reject non-uluna funds in `WithdrawSubmit` | 3.4 | Open |
| P3 | Add `WithdrawAlreadyApproved` error variant | 3.5 | Open |
| P3 | Add expiration/cleanup for stale pending withdrawals | 3.6 | Open |
| P3 | Add `LockUnlock` type check in `DepositNative` | 3.7 | Open |
| P3 | Verify `SetWithdrawDelay` doc alignment | 3.8 | Open |

---

## 6. Test Evidence Snapshot

`cargo test` in `packages/contracts-terraclassic`:

| Suite | Tests | Status |
|-------|-------|--------|
| Unit tests (hash, fee_manager, address_codec) | 38 + 8 | Pass |
| Integration tests | 33 | Pass |
| Withdraw flow tests | 26 | Pass |
| Chain registry tests | 22 | Pass |
| Fee system tests | 19 | Pass |
| Hash parity tests | 20 | Pass |
| Incoming token registry tests | 26 | Pass |
| **Total** | **192** | **All passed** |

---

*End of Security Review*
