# Security Review: Terra Classic Bridge Contracts
**Date:** 2026-02-12
**Scope:** `packages/contracts-terraclassic`

## 1. Executive Summary

A security review was performed on the `bridge` contract implementation for Terra Classic. The contract implements a federated bridge with a "Watchtower" security pattern (v2.0), allowing a `Canceler` role to intervene during a withdrawal delay window.

The implementation follows standard CosmWasm patterns and includes robust access control, reentrancy protection via the Checks-Effects-Interactions (CEI) pattern, and comprehensive configuration options.

**Key Findings:**
- **Architecture:** Solid Watchtower implementation with clear role separation (Admin, Operator, Canceler).
- **Security:** No critical vulnerabilities found in the logic flow.
- **Risk:** The primary risk is the handling of Terra Classic's potential tax on transfers, which could cause accounting drift if not managed.

## 2. Detailed Findings

### 2.1 Project Specific Risks

#### 2.1.1 Terra Classic Tax Handling (Medium Risk)
Terra Classic (LUNC) and its native stablecoins have historically had tax mechanisms (burn tax).
- **Issue:** The contract tracks `LOCKED_BALANCES` based on the `amount` passed in `execute_deposit_native`. It also sends `BankMsg::Send` for withdrawals and fee collection.
- **Risk:**
    - If a tax is deducted from the *received* amount before it hits the contract (but `info.funds` reports the pre-tax amount?), or if the contract receives less than it thinks, `LOCKED_BALANCES` will inflate relative to actual balance.
    - If a tax is charged on *outgoing* `BankMsg::Send` (deducted from the sent amount), the recipient receives less than expected.
    - If a tax is charged on *outgoing* `BankMsg::Send` (charged in addition to the amount), the contract pays the tax. Over time, this drains the contract's balance faster than `LOCKED_BALANCES` decreases, potentially leading to insolvency (inability to pay out the last users).
- **Recommendation:** Verify the current tax parameters on Terra Classic. If tax is non-zero, ensure the contract logic accounts for it (e.g., deducting tax from the credited amount, or ensuring `fee_bps` is sufficient to cover outgoing tax costs).

#### 2.1.2 Decimal Normalization Truncation (Low Risk)
The `normalize_decimals` function handles conversion between source and destination chain decimals.
- **Logic:** When `src_decimals > dest_decimals`, it divides by a power of 10.
- **Impact:** This truncates the amount. Small "dust" amounts (below the precision of the destination chain) will be lost (result = 0).
- **Mitigation:** This is generally acceptable for bridges, but should be documented for users. The contract handles the overflow case (`src < dest`) correctly using `Uint256` and checking for `Uint128` limits.

#### 2.1.3 Token Mapping Integrity (Admin Risk)
The bridge relies on correct mappings in `TOKEN_SRC_MAPPINGS` (for withdrawals) and `TOKEN_DEST_MAPPINGS` (for deposits).
- **Risk:** If an Admin configures a mapping with incorrect decimals, the `normalize_decimals` function will compute incorrect amounts, potentially allowing users to withdraw significantly more or less than intended.
- **Mitigation:** Operational procedures must verify decimal precision before registering tokens.

### 2.2 Common CosmWasm Vulnerabilities

#### 2.2.1 Reentrancy
- **Analysis:** The contract consistently applies the Checks-Effects-Interactions (CEI) pattern.
    - **Withdrawals:** `pending.executed` is set to `true` and saved to storage *before* any `CosmosMsg` (Bank Send or Wasm Execute) is created/returned.
    - **Deposits (CW20):** `LOCKED_BALANCES` / `OUTGOING_NONCE` are updated *before* any fee transfer messages.
- **Conclusion:** Safe from reentrancy.

#### 2.2.2 Access Control
- **Analysis:**
    - `WithdrawApprove` is strictly limited to `OPERATORS` (or Admin).
    - `WithdrawCancel` is strictly limited to `CANCELERS`.
    - `WithdrawExecute*` enforces the time delay window.
    - Configuration functions are strictly `Admin` only.
- **Conclusion:** Access control is implemented correctly.

#### 2.2.3 Denial of Service (Spam)
- **Issue:** Users can spam `WithdrawSubmit` with valid gas but invalid cross-chain nonces.
- **Impact:** `PENDING_WITHDRAWS` storage grows.
- **Mitigation:** The `PendingWithdrawals` query supports pagination (`start_after`, `limit`), ensuring that Operators and Cancelers can continue to function even with a large state. The cost of attack is limited by gas fees.

#### 2.2.4 Replay Protection
- **Analysis:**
    - **Outgoing:** `OUTGOING_NONCE` increments monotonically.
    - **Incoming:** `WithdrawSubmit` checks `PENDING_WITHDRAWS` for existing hash. `WithdrawApprove` updates `WITHDRAW_NONCE_USED`.
- **Note:** `WITHDRAW_NONCE_USED` is set but not strictly checked within the *contract's* submit/approve flow to block execution (relies on Operator off-chain check or `WithdrawAlreadySubmitted`). However, since `xchain_hash_id` includes the nonce, a duplicate submission with the same nonce/amount is blocked by `WithdrawAlreadySubmitted`. A submission with same nonce but different amount requires Operator rejection (which is standard federated behavior).

### 2.3 Code Quality & Best Practices
- **Funds Validation:** `execute_deposit_native` strictly enforces `info.funds.len() == 1`, preventing confusion with multiple token sends.
- **Safe Arithmetic:** Uses `Uint128` and `Uint256` (via helper) to prevent overflow/underflow.
- **Versioning:** `set_contract_version` is used for migration safety.

## 3. Recommendations

1.  **Tax Verification:** Confirm Terra Classic tax behavior. If the contract is expected to pay tax on outgoing transfers, verify that the collected fees (`fee_bps`) are sufficient to cover this burn, or implement a mechanism to deduct the tax from the user's withdrawal amount.
2.  **Operational Safety:** Ensure Operators and Cancelers are monitoring `PendingWithdrawals` efficiently using the pagination features.
3.  **Monitoring:** Monitor `LOCKED_BALANCES` vs actual contract balance to detect any drift early (e.g. due to tax or rounding).

