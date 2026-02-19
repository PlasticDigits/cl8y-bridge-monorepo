# Security Review: contracts-evm Package

**Date:** 2026-02-19 (UTC)
**Epoch:** 20260219_000000
**Reviewer:** AI Assistant
**Package:** `packages/contracts-evm`

---

## Executive Summary

This document provides a security review of the `contracts-evm` package, focusing on the `Bridge.sol` contract, `TokenRegistry.sol`, `ChainRegistry.sol`, and associated handlers (`LockUnlock.sol`, `MintBurn.sol`). The system implements a cross-chain bridge supporting both Lock/Unlock and Mint/Burn mechanisms.

The codebase is well-structured, leveraging OpenZeppelin for core security (Access Control, Pausable, ReentrancyGuard, SafeERC20) and Solady for optimized enumerable sets.

### Key Findings Summary

| Severity | Count | Summary |
|----------|-------|---------|
| High     | 0     | — |
| Medium   | 2     | Rate Limit Window, Guard/Fee DoS Vectors |
| Low      | 3     | Centralization risks, Token Compatibility, Operational requirements |
| Info     | 2     | Design choices, Gas optimization |

---

## 1. Project Specific Risks

### 1.1 Rate Limiting Strategy (Fixed Window)
**Risk:** The rate limiting in `TokenRegistry.sol` uses a fixed 24-hour window that resets relative to the first transaction in the window.
**Analysis:**
```solidity
if (block.timestamp >= win.windowStart + RATE_LIMIT_WINDOW) {
    win.windowStart = block.timestamp;
    win.used = 0;
}
```
**Implication:** A user can consume 100% of their limit at the end of a window (e.g., T=23h59m) and another 100% immediately after the reset (T=24h01m), effectively doubling the throughput in a short period.
**Mitigation:** This is a common trade-off for gas efficiency. Operational monitoring should be aware of this "burst" capability.

### 1.2 Denial of Service (DoS) Vectors
**Risk:** The system relies on external calls that can revert, potentially blocking core functionality.
**Analysis:**
1.  **Fee Recipient:** In `Bridge.depositNative`, `feeRecipient.call{value: fee}("")` is checked. If the fee recipient is a contract that reverts on receive (and `fee > 0`), all deposits fail.
2.  **Operator Gas:** In `Bridge.withdrawApprove`, the transfer of `operatorGas` to `msg.sender` (the operator) is checked. If the operator cannot receive ETH, they cannot approve withdrawals.
3.  **Guard Bridge:** `Bridge` calls `GuardBridge`, which iterates over all registered guard modules. If *any* guard module reverts, the entire transaction fails.
**Mitigation:**
-   Ensure `feeRecipient` and `operator` addresses are EOAs or capable contracts.
-   Vigilantly audit and monitor any modules added to `GuardBridge`.
-   The owner can set `guardBridge` to `address(0)` in emergencies.

### 1.3 Token Compatibility
**Risk:** `LockUnlock.sol` and `MintBurn.sol` enforce strict balance checks:
```solidity
if (finalBalanceThis != initialBalanceThis - amount) revert InvalidUnlockThis();
```
**Implication:**
-   **Fee-on-Transfer Tokens:** Will always revert.
-   **Rebasing Tokens:** May cause accounting errors or reverts.
-   **TokenRegistry:** Does *not* automatically validate these properties during registration; it only checks `totalSupply`.
**Mitigation:** Strict manual verification of token mechanics before registration.

### 1.4 Decimal Precision Trust
**Risk:** `Bridge.withdrawSubmit` uses `tokenRegistry.getSrcTokenDecimals(srcChain, token)`.
**Implication:** If `TokenRegistry` is misconfigured with incorrect source decimals, `_normalizeDecimals` will mint/unlock incorrect amounts on the destination chain.
**Mitigation:** Verify all `setIncomingTokenMapping` calls carefully.

---

## 2. Common Solidity Pitfalls Analysis

### 2.1 Reentrancy
**Status:** **Protected**
-   All external state-changing functions in `Bridge`, `LockUnlock`, and `MintBurn` use `nonReentrant`.
-   External calls (fees, operator gas, guard checks) happen either after state changes or under `nonReentrant` protection.

### 2.2 Access Control
**Status:** **Correct**
-   Critical functions (`authorizeUpgrade`, `setFeeParams`, registry management) are restricted to `onlyOwner`.
-   Operational functions (`withdrawApprove`, `withdrawCancel`) are restricted to `onlyOperator` or `onlyCanceler`.
-   `TokenRegistry` restricts rate limit updates to the `rateLimitBridge`.

### 2.3 Integer Overflow/Underflow
**Status:** **Protected**
-   Solidity `^0.8.30` provides built-in overflow protection.
-   `AccessManagerEnumerable` uses `unchecked` for loop counters (safe).

### 2.4 Data Validation
**Status:** **Robust**
-   Checks for zero addresses (`destAccount`, `feeRecipient`).
-   Checks for zero amounts.
-   Checks for registered chains and tokens.
-   `HashLib` ensures unique IDs for transfers involving `nonce`.

---

## 3. Code Quality & Best Practices

-   **Modularity:** The separation of concerns (Bridge, Registries, Asset Handlers) is excellent.
-   **Upgradability:** UUPS pattern is used consistently. Storage gaps (`__gap`) are present to allow future storage expansion without collisions.
-   **Libraries:** Usage of `Solady` for `EnumerableSetLib` is a good gas optimization choice.
-   **Events:** Events are comprehensive and include previous values for updates (e.g., `CancelWindowUpdated`).

---

## 4. Recommendations

1.  **Operational Procedures:**
    -   **Fee Recipient:** Must be an EOA or `payable` contract.
    -   **Operators:** Must be EOAs or `payable` contracts.
    -   **Token Registration:** Manually verify that tokens are standard ERC20s (no fees, no rebasing).
2.  **Monitoring:**
    -   Monitor for `GuardBridge` failures.
    -   Monitor for Rate Limit "bursts" around the 24h window reset.
3.  **Future Improvements:**
    -   Consider a sliding window for rate limiting if gas costs allow, to smooth out the burst risk.
    -   Consider `try/catch` blocks around `GuardBridge` calls if resilience to a single failing guard is desired (though failing safe is usually preferred).

---

*End of Security Review*
