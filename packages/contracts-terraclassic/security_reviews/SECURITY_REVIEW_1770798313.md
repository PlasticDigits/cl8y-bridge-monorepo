# Security Review - Contracts TerraClassic

**Date:** 2026-02-11
**Epoch:** 1770798313
**Reviewer:** AI Assistant

## 1. Executive Summary

This security review covers the `contracts-terraclassic` package, specifically focusing on the bridge implementation in `bridge/src`. The review analyzes the implementation of the Watchtower security pattern, rate limiting, access control, and general code safety.

The codebase implements a robust cross-chain bridge with a "Watchtower" security model, separating the roles of Operators (who approve withdrawals) and Cancelers (who can block fraudulent withdrawals during a delay period).

## 2. Scope

The following files were reviewed:
- `bridge/src/contract.rs`: Entry points and message dispatch.
- `bridge/src/state.rs`: State definitions and storage layout.
- `bridge/src/msg.rs`: Message definitions and API surface.
- `bridge/src/execute/withdraw.rs`: Core withdrawal logic (Submit, Approve, Cancel, Execute).
- `bridge/src/execute/outgoing.rs`: Outgoing transfer logic (Deposit, Receive).
- `bridge/src/execute/admin.rs`: Admin and emergency functions.
- `bridge/src/address_codec.rs`: Universal address encoding.

## 3. Findings

### 3.1. Watchtower Pattern Implementation (Positive)
The contract correctly implements the Watchtower pattern:
- **Separation of Duties**: Operators approve withdrawals, but cannot execute them immediately. Cancelers can cancel approved withdrawals but cannot approve them.
- **Withdrawal Delay**: A configurable `WITHDRAW_DELAY` (default 5 minutes) enforces a window between approval and execution, giving Cancelers time to act.
- **Checks-Effects-Interactions**: The `execute_withdraw_execute_unlock` and `execute_withdraw_execute_mint` functions correctly update state (marking as executed) *before* transferring or minting tokens, preventing reentrancy attacks.

### 3.2. Rate Limiting (Positive with Note)
Rate limiting is enforced on withdrawals:
- **Dual Limits**: Supports both per-transaction and per-period (24h) limits.
- **Default Safety**: If no limit is explicitly configured, the contract attempts to default to 0.1% of the total token supply.
- **Observation**: The fallback `DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY` is set to 100 * 10^18. While safe for high-supply tokens with 18 decimals, this might be high for low-supply or low-decimal tokens if the supply query fails.

### 3.3. Decimal Normalization (Positive)
The `normalize_decimals` function in `withdraw.rs` safely handles conversion between source and destination chain decimals:
- Uses `Uint256` and `Uint512` for intermediate calculations to prevent overflow.
- Correctly handles scaling up and scaling down.

### 3.4. Access Control (Positive)
- **RBAC**: Distinct roles for Admin, Operators, and Cancelers.
- **Emergency Pause**: Admin can pause the contract, stopping all bridge operations.
- **Asset Recovery**: `execute_recover_asset` allows the admin to recover stuck funds, but *only* when the contract is paused. This prevents admin abuse during active operation.

### 3.5. Address Encoding (Positive)
The `address_codec.rs` module implements a universal 32-byte address format compatible with the EVM side:
- Distinguishes between chain types (EVM=1, Cosmos=2).
- Enforces strict validation of reserved bytes.

## 4. Recommendations

1.  **Review Default Rate Limit**: Verify that `DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY` (100 * 10^18) is appropriate for all intended native tokens, especially if `cosmwasm_1_2` feature is not enabled or supply queries fail. Consider making this default configurable or lower.
2.  **Operational Security**: Ensure that `OPERATORS` and `CANCELERS` are different entities in the production deployment. If the same entity controls both, the security benefits of the Watchtower pattern are negated.
3.  **Monitoring**: Set up off-chain monitoring to alert Cancelers immediately when `WithdrawApprove` is emitted, ensuring they have the full `WITHDRAW_DELAY` window to react.

## 5. Conclusion

The `contracts-terraclassic` bridge implementation is well-structured and follows best practices for security. The adoption of the Watchtower pattern significantly enhances the security posture against compromised operators. No critical vulnerabilities were found during this review.
