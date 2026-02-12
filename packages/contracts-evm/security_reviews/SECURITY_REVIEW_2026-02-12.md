# Security Review: contracts-evm Package

**Date:** 2026-02-12 (UTC)
**Epoch:** 20260212_000000
**Reviewer:** Security Review Agent
**Package:** `packages/contracts-evm`

---

## Executive Summary

This document provides a security review of the `contracts-evm` package, focusing on the `Bridge.sol` contract and its dependencies (`LockUnlock.sol`, `MintBurn.sol`, registries). The system implements a cross-chain bridge with a hub-and-spoke or point-to-point architecture, featuring user-initiated withdrawals, operator approvals, and a cancellation window.

The codebase follows standard Solidity patterns and leverages OpenZeppelin libraries for core security features (Access Control, Pausable, ReentrancyGuard, SafeERC20).

### Key Findings Summary

| Severity | Count | Summary |
|----------|-------|---------|
| High     | 0     | — |
| Medium   | 0     | — |
| Low      | 2     | Centralization risks, Operational requirements |
| Info     | 3     | Design choices, Token compatibility |

---

## 1. Project Specific Risks

### 1.1 Centralization & Role Power
**Risk:** The `owner` has extensive control, including upgrading contracts, setting fees, and managing operators/cancelers. The `operator` role is critical for approving withdrawals.
**Mitigation:**
-   **Timelock:** Consider using a TimelockController for the `owner` address in production to prevent instant malicious upgrades or parameter changes.
-   **Multi-sig:** The `owner` should be a multi-sig wallet.
-   **Operator Monitoring:** The `operator` is a hot wallet or automated service. Its compromise could lead to approval of invalid withdrawals (though they still need to be validly signed/hashed to match a deposit). However, the operator *cannot* forge a deposit record on the source chain, but they could approve a withdrawal that hasn't actually happened if the off-chain verification is compromised.

### 1.2 Bridge Guard Dependence
**Risk:** The `guardBridge` (if set) is called on every deposit and withdraw. A malicious or buggy guard could cause a Denial of Service (DoS) for all bridge operations.
**Mitigation:**
-   Ensure `guardBridge` is thoroughly tested.
-   The `owner` can set `guardBridge` to `address(0)` to disable it in an emergency.

### 1.3 Token Compatibility
**Risk:** `LockUnlock.sol` and `MintBurn.sol` explicitly do not support rebasing tokens or fee-on-transfer tokens.
-   **LockUnlock:** Checks `finalBalanceThis != initialBalanceThis + amount` (and similar for `from`). Fee-on-transfer tokens would fail this check, causing reverts. Rebasing tokens could cause accounting discrepancies over time.
-   **MintBurn:** Relies on standard `mint`/`burn` interfaces.
**Mitigation:**
-   **Strict Registration:** Only register tokens that are known to be compatible (standard ERC20).
-   **Operational Process:** Verify token mechanics before calling `tokenRegistry.addToken`.

### 1.4 Native Token Handling
**Risk:** `depositNative` accepts ETH and keeps it in the `Bridge` contract. It does *not* wrap it into WETH. The `wrappedNative` address is used primarily as an identifier.
**Implication:**
-   The Bridge contract effectively acts as a vault for native ETH.
-   If the implementation is upgraded to one that doesn't have a `receive()` function or logic to handle ETH, funds could be stuck (though `recoverAsset` exists).
-   `recoverAsset` allows the owner to drain ETH, which is a centralization risk (rug pull vector).

### 1.5 Withdrawal Window Boundary
**Risk:** The withdrawal execution logic uses an exclusive boundary: `if (block.timestamp <= windowEnd) revert CancelWindowActive(windowEnd);`.
**Analysis:** This means a withdrawal cannot be executed *exactly* at `windowEnd`. It must be at `windowEnd + 1` second or later. This is a precise design choice and must be accounted for in off-chain executors.

---

## 2. Common Solidity Pitfalls Analysis

### 2.1 Reentrancy
**Status:** **Protected**
-   `Bridge.sol`: All external state-changing functions (`deposit*`, `withdraw*`) use `nonReentrant` or `whenPaused` (which implies no reentrancy during normal operation, though `whenPaused` doesn't prevent reentrancy itself, `nonReentrant` does).
-   `LockUnlock.sol` / `MintBurn.sol`: Functions `lock`, `unlock`, `mint`, `burn` are `nonReentrant`.
-   **ETH Transfers:** `call` is used for fee payments and `operatorGas`. These are done *after* state checks or at the end of the function, and `nonReentrant` protects against callbacks.

### 2.2 Integer Overflow/Underflow
**Status:** **Protected**
-   Solidity `^0.8.30` is used, which has built-in overflow protection.
-   `AccessManagerEnumerable` uses `unchecked` for loop counters, which is safe.
-   No unsafe `unchecked` blocks found in core logic.

### 2.3 Access Control
**Status:** **Correct**
-   `onlyOwner`, `onlyOperator`, `onlyCanceler` modifiers are applied consistently.
-   `_authorizeUpgrade` is properly restricted to `onlyOwner`.

### 2.4 Unchecked Return Values
**Status:** **Checked**
-   Low-level `call` for ETH transfers checks the boolean return value: `if (!success) revert ...`.
-   `SafeERC20` is used for all token transfers, ensuring reverts on failure.

### 2.5 Denial of Service
**Status:** **Low Risk**
-   No unbounded loops in critical paths.
-   `AccessManagerEnumerable` has loops, but these are for view functions or restricted admin actions, not core bridge flow.

---

## 3. Code Quality & Best Practices

-   **Modularity:** The separation of concerns (Bridge, Registries, LockUnlock/MintBurn) is good. It allows upgrading specific components without replacing the entire system.
-   **Libraries:** Heavy use of OpenZeppelin ensures standard security patterns.
-   **Events:** Comprehensive event emission for off-chain indexing.
-   **Error Handling:** Custom errors are used (gas efficient) instead of require strings.

---

## 4. Recommendations

1.  **Fee Recipient Validation:** Ensure the `feeRecipient` address is an EOA or a contract with a `receive()` function. If it's a contract without `receive()`, ETH deposits will revert.
2.  **Operator Gas Failure:** In `withdrawApprove`, if the `operatorGas` transfer fails (e.g., operator is a contract that reverts), the approval reverts. Ensure the operator address is capable of receiving ETH.
3.  **Owner Security:** Given the power of the `owner` role (including `recoverAsset`), use a Multi-Sig wallet with a Timelock for the owner address in production.
4.  **Monitoring:** Set up monitoring for `GuardBridge` failures and `CancelWindowActive` reverts to detect potential issues or attacks.

---

*End of Security Review*
