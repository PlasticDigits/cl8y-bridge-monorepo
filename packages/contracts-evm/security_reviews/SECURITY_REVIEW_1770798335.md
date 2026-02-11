# Security Review: contracts-evm Package

**Date:** 2026-02-11 (UTC)  
**Epoch:** 1770798335  
**Reviewer:** Security Review  
**Package:** `packages/contracts-evm`

---

## Executive Summary

This document provides a full security review of the contracts-evm package. The bridge implements a cross-chain asset transfer system with user-initiated withdrawals, operator approvals, and a canceler role for withdrawal cancellation within a configurable time window.

### Remediation Status (since prior review 20260211_170530)

| Finding | Status |
|---------|--------|
| All prior High/Medium findings | ✅ Resolved |
| RBAC, depositNative validation, cancel window | ✅ Verified unchanged |

### Key Findings Summary (current state)

| Severity | Count | Summary |
|----------|-------|---------|
| High | 0 | — |
| Medium | 0 | — |
| Low/Info | 5 | Operational notes, edge cases |

---

## 1. Test Coverage

### 1.1 Coverage Overview

- **Total test suites:** 15
- **Total tests:** 307+ passing (includes 4 invariant tests)
- **Bridge tests:** 65
- **Bridge invariants:** 4 (depositNonce, thisChainId, cancelWindow, registries)

### 1.2 Per-Contract Test Status

| Contract | Tests | Status |
|----------|-------|--------|
| Bridge | 65 | ✅ Excellent |
| BridgeInvariantTest | 4 | ✅ Implemented |
| ChainRegistry | 18 | ✅ Good |
| TokenRegistry | 24 | ✅ Good |
| LockUnlock | 8 | ✅ Adequate |
| MintBurn | 7 | ✅ Adequate |
| GuardBridge | 9 | ✅ Good |
| BlacklistBasic | 7 | ✅ Adequate |
| TokenRateLimit | 11 | ✅ Good |
| HashLib | 38 | ✅ Excellent |
| FeeCalculatorLib | 32 | ✅ Good |
| Others | Various | ✅ Adequate |

### 1.3 Test Coverage Gaps

1. **Guard–Bridge integration** – Documented in OPERATIONAL_NOTES.md §8. GuardBridge tested in isolation.
2. **Create3Deployer** – TODO for future (OPERATIONAL_NOTES.md §10). Standard Solady CREATE3 wrapper.
3. **depositNative** – Full validation tests (wrappedNative not set, token not registered, dest mapping not set).

---

## 2. Top Common Solidity Issues

### 2.1 Reentrancy ✅

- **Bridge:** Uses `ReentrancyGuard` and `nonReentrant` on all deposit/withdraw entry points.
- **LockUnlock / MintBurn:** Use `ReentrancyGuard` on `lock`, `unlock`, `burn`, `mint`.
- **ETH transfers:** Fee and `operatorGas` transfers occur after state updates; `nonReentrant` prevents reentrancy.

**Verdict:** Reentrancy protections are in place.

### 2.2 Integer Overflow / Underflow ✅

- Solidity `^0.8.30` provides built-in checks.
- No unsafe `unchecked` usage in critical Bridge/LockUnlock/MintBurn paths.

**Verdict:** No issues identified.

### 2.3 Access Control ✅

- **Admin (onlyOwner):** `pause`, `unpause`, `setCancelWindow`, `setGuardBridge`, `recoverAsset`, `addOperator`, `removeOperator`, `addCanceler`, `removeCanceler`, `setFeeParams`, `setCustomAccountFee`, `removeCustomAccountFee`, upgrades. Chain/token registration in registries. `wrappedNative` set at deployment in `initialize()` only.
- **Operator (onlyOperator):** `withdrawApprove`, `withdrawUncancel` only.
- **Canceler (onlyCanceler):** `withdrawCancel` only.
- **Public:** `deposit*`, `withdrawSubmit`, `withdrawExecute*`.

**Verdict:** RBAC aligns with the design.

### 2.4 Unchecked Low-Level Calls ✅

- Fee and `operatorGas` transfers use `call` with revert on failure.
- External value transfers are guarded.

**Verdict:** No issues identified.

### 2.5 Front-Running ✅

- `withdrawSubmit` and `withdrawExecute*` are permissionless by design; recipient is derived from `destAccount`.
- No vulnerability from front-running identified.

### 2.6 Token Assumptions ✅ Documented

- **Rebasing / fee-on-transfer:** Not supported; LockUnlock/MintBurn balance checks document this.
- **Status:** Documented in OPERATIONAL_NOTES.md §4.

---

## 3. Flows

### 3.1 Deposit Flow

1. **depositERC20 / depositERC20Mintable**
   - Validates token and chain; calculates fee; locks or burns; stores deposit record; emits event.

2. **depositNative**
   - Reverts if `wrappedNative == address(0)`.
   - Reverts if `wrappedNative` is not registered (`TokenNotRegistered`).
   - Reverts if dest mapping for `wrappedNative` and `destChain` is not set (`DestTokenMappingNotSet`).
   - Validates amount, chain; transfers fee to feeRecipient; Bridge retains net ETH; records deposit with `token: wrappedNative`.

### 3.2 Withdraw Flow

1. **withdrawSubmit** (public) – Creates `PendingWithdraw`. `srcDecimals` provided by submitter; validated off-chain by operator.
2. **withdrawApprove** (operator) – Marks approved, pays `operatorGas`.
3. **withdrawCancel** (canceler) – Cancels within window.
4. **withdrawUncancel** (operator) – Restores and restarts window.
5. **withdrawExecuteUnlock / withdrawExecuteMint** (public) – Executes after window (exclusive boundary).

### 3.3 Hash Consistency

- `HashLib.computeTransferHash` used consistently on deposit and withdraw.
- Correct alignment for cross-chain matching (srcChain, destChain, srcAccount, destAccount, token, amount, nonce).

---

## 4. Access Control (RBAC)

### 4.1 Bridge

| Function | Access | Status |
|----------|--------|--------|
| `pause`, `unpause` | onlyOwner | ✅ |
| `setCancelWindow`, `setGuardBridge` | onlyOwner | ✅ |
| `recoverAsset` | onlyOwner, whenPaused | ✅ |
| `addOperator`, `removeOperator` | onlyOwner | ✅ |
| `addCanceler`, `removeCanceler` | onlyOwner | ✅ |
| `setFeeParams`, `setCustomAccountFee`, `removeCustomAccountFee` | onlyOwner | ✅ |
| `withdrawApprove`, `withdrawUncancel` | onlyOperator | ✅ |
| `withdrawCancel` | onlyCanceler | ✅ |
| `deposit*`, `withdrawSubmit`, `withdrawExecute*` | Public | ✅ |
| `upgradeToAndCall` | onlyOwner | ✅ |

### 4.2 ChainRegistry / TokenRegistry

- All registration and config: `onlyOwner`. No operators or cancelers.

### 4.3 Owner as Implicit Operator/Canceler

- `_onlyOperator` and `_onlyCanceler` both allow `msg.sender == owner()`.
- Documented in OPERATIONAL_NOTES.md §11.

---

## 5. Additional Findings

### 5.1 Cancel Window Boundary ✅ Verified

**Location:** `Bridge.sol` `_validateWithdrawExecution`, OPERATIONAL_NOTES.md §3

**Implementation:** `if (block.timestamp <= windowEnd) revert CancelWindowActive(windowEnd);`  
**Semantics:** Execution allowed when `block.timestamp > windowEnd` (exclusive). Documentation matches implementation.

### 5.2 Fee Recipient Must Accept ETH (Low) ✅ Documented

**Location:** `Bridge.sol` – `depositNative`, `setFeeParams`

**Status:** Documented in OPERATIONAL_NOTES.md §1. `feeRecipient` must accept plain ETH.

### 5.3 Guard Module State Mutations (Low) ✅ Documented

**Location:** `TokenRateLimit.sol` – `checkDeposit`, `checkWithdraw`

**Status:** Documented in OPERATIONAL_NOTES.md §2. Guard check functions may mutate state.

### 5.4 withdrawExecuteUnlock vs withdrawExecuteMint – Caller Responsibility (Info)

**Location:** `Bridge.sol` – `withdrawExecuteUnlock`, `withdrawExecuteMint`

**Status:** Documented in OPERATIONAL_NOTES.md §7. Caller must invoke the correct function per token type.

### 5.5 Decimal Normalization Edge Case (Info)

**Location:** `Bridge.sol` – `_normalizeDecimals`

**Observation:** When `destDecimals > srcDecimals`, the function multiplies `amount` by `10 ** (destDecimals - srcDecimals)`. For extreme decimal differences (e.g., srcDecimals=0, destDecimals=77+), multiplication could overflow `uint256`. In practice, tokens use 0–18 decimals; `srcDecimals` is user-provided in `withdrawSubmit` but validated off-chain by the operator before approval.

**Mitigation:** Operator must validate `srcDecimals` matches source-chain token (OPERATIONAL_NOTES.md §12). Extremely misaligned decimals would cause revert, not silent corruption.

**Verdict:** Informational; acceptable risk given operator validation.

### 5.6 TokenRegistry unregisterToken While Deposits Exist (Info)

**Location:** `TokenRegistry.sol` – `unregisterToken`

**Observation:** Unregistering a token clears mappings but does not affect existing `DepositRecord` entries in the Bridge. Pending withdrawals for that token could still execute if the token remains registered on the source chain. Unregistering removes the token from outgoing flows only; incoming flows rely on operator validation.

**Verdict:** By design; no vulnerability. Operators validate incoming withdrawals.

---

## 6. Recommendations Summary

### Implemented

1. RBAC – Admin-only for config; operator for withdraw approval only.
2. depositNative validation – Full checks (wrappedNative, token registration, dest mapping).
3. wrappedNative at deploy – Set in `Bridge.initialize()` only; no `setWrappedNative`.
4. Cancel window – Exclusive boundary documented and implemented.
5. OPERATIONAL_NOTES.md – Comprehensive operational and integrator guidance.

### Informational

6. **Create3Deployer** – TODO for future (OPERATIONAL_NOTES.md §10).
7. **Decimal normalization** – Edge case documented above; operator validates srcDecimals.

---

## 7. Appendix: Contract Inventory

| Contract | Purpose |
|----------|---------|
| Bridge | Main bridge: deposits, withdrawals, approvals, cancel |
| ChainRegistry | Chain ID and identifier registry |
| TokenRegistry | Token registration and per-chain mappings |
| LockUnlock | Lock/unlock ERC20 for LockUnlock tokens |
| MintBurn | Burn/mint for MintBurn tokens |
| GuardBridge | Composable guard orchestration |
| BlacklistBasic | Account blacklist guard |
| TokenRateLimit | Per-token 24h rate limits |
| AccessManagerEnumerable | Extended AccessManager with enumeration |
| DatastoreSetAddress | Generic address set storage |
| FactoryTokenCl8yBridged | Bridged token factory |
| TokenCl8yBridged | Bridged ERC20 implementation |
| HashLib | Transfer hash and encoding utilities |
| FeeCalculatorLib | Fee calculation (standard, discounted, custom) |
| AddressCodecLib | Address encoding/decoding for Cosmos/EVM |
| Create3Deployer | Deterministic deployment support |

---

## 8. Changelog vs. Prior Review (20260211_170530)

| Item | Before | After |
|------|--------|-------|
| New findings | — | Decimal normalization edge case (Info), unregisterToken impact (Info) |
| High findings | 0 | 0 |
| Medium findings | 0 | 0 |
| Low/Info findings | 4 | 5 |
| Test count | 317 | 307+ |

---

*End of Security Review*
