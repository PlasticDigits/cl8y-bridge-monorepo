# Security Review: Terra Classic Bridge Contracts
**Date:** 2026-02-19
**Scope:** `packages/contracts-terraclassic` (specifically `bridge` contract)

## 1. Executive Summary

A comprehensive security audit was conducted on the Terra Classic bridge smart contracts. The audit focused on the V2 "Watchtower" implementation and general security posture.

**Critical Findings:**
- **CRITICAL**: The multisig functionality (`min_signatures`) is defined but **completely ignored** in the withdrawal approval process. A single operator can approve any withdrawal, regardless of the configured threshold.
- **HIGH**: The nonce replay protection mechanism (`WITHDRAW_NONCE_USED`) is **ineffective**. The contract writes to this storage but **never reads/checks it**, allowing multiple withdrawals with the same source nonce to be approved if they differ in other parameters (e.g., amount or recipient).

**Recommendations:**
- Immediately patch `execute_withdraw_approve` to enforce `min_signatures` by tracking a set of approvers per withdrawal.
- Immediately patch `execute_withdraw_approve` (or `execute_withdraw_submit`) to check `WITHDRAW_NONCE_USED` and reject duplicate nonces.
- Consider restricting `WithdrawSubmit` or adding a fee to prevent state bloat.

## 2. Detailed Findings

### 2.1 Critical Severity

#### 2.1.1 Multisig Bypass in Withdrawal Approval

**Description:**
The contract configuration includes a `min_signatures` parameter, intended to enforce a minimum number of operator approvals before a withdrawal is considered "approved". However, the `execute_withdraw_approve` function in `src/execute/withdraw.rs` lacks any logic to count or aggregate signatures.

**Vulnerable Code:**
```rust
// src/execute/withdraw.rs lines 187-238
pub fn execute_withdraw_approve(...) {
    // ... verifies sender is an operator ...
    
    // Immediately marks withdrawal as approved
    pending.approved = true; 
    pending.approved_at = env.block.time.seconds();
    PENDING_WITHDRAWS.save(deps.storage, &hash_bytes, &pending)?;
    
    // ...
}
```

**Impact:**
Any single registered operator (even if compromised) can approve fraudulent withdrawals, completely bypassing the security guarantees of the intended multisig scheme. This centralizes trust in every single operator rather than the aggregate set.

**Recommendation:**
Modify `PendingWithdraw` to store a list of approvers (`approvers: Vec<Addr>`). In `execute_withdraw_approve`:
1. Check if `info.sender` has already approved.
2. Add `info.sender` to `approvers`.
3. Only set `approved = true` if `approvers.len() >= config.min_signatures`.

### 2.2 High Severity

#### 2.2.1 Ineffective Nonce Replay Protection (`WITHDRAW_NONCE_USED`)

**Description:**
The contract attempts to track used nonces per source chain via the `WITHDRAW_NONCE_USED` map. While `execute_withdraw_approve` writes `true` to this map, **no function ever reads from it** to validate uniqueness.

**Vulnerable Code:**
```rust
// src/execute/withdraw.rs lines 222-223 (Write only)
let nonce_key = (pending.src_chain.as_slice(), pending.nonce);
WITHDRAW_NONCE_USED.save(deps.storage, nonce_key, &true)?;
```

**Impact:**
The contract fails to enforce "at most one withdrawal per source nonce". An attacker (or buggy operator) could submit and approve multiple variations of a withdrawal (same nonce, different amount/recipient) resulting in double-spending, provided they generate valid hashes. While honest operators should reject this off-chain, the on-chain protection is dead code.

**Recommendation:**
In `execute_withdraw_approve` (or `execute_withdraw_submit`), add a check:
```rust
if WITHDRAW_NONCE_USED.may_load(deps.storage, nonce_key)?.unwrap_or(false) {
    return Err(ContractError::NonceAlreadyUsed);
}
```

### 2.3 Medium Severity

#### 2.3.1 Unbounded Withdrawal Submission (Spam Risk)

**Description:**
The `execute_withdraw_submit` function allows any address to submit a withdrawal request. There is no minimum fee (native funds are forwarded as a tip, but can be 0) or whitelist.

**Impact:**
An attacker can flood the `PENDING_WITHDRAWS` storage with invalid or garbage requests. This increases storage costs for validators and potentially degrades query performance for operators monitoring the queue.

**Recommendation:**
- Implement a minimum fee (burned or sent to fee collector) for `WithdrawSubmit`.
- Or restrict submission to a trusted set of relayers (though this impacts decentralization).

### 2.4 Informational

#### 2.4.1 Dead Code / Misleading Logic
The `WITHDRAW_NONCE_USED` map consumes gas to write but provides no security benefit in its current state.

#### 2.4.2 Missing Rate Limits on Deposits
Comments indicate "Deposit limits are not enforced (only withdraw limits apply)". While this protects the bridge's solvency on this chain, it allows unlimited minting/locking. Consider if `max_bridge_amount` should apply to deposits to prevent lopsided liquidity.

## 3. Conclusion

The bridge contract contains **Critical** vulnerabilities that undermine its core security assumptions (multisig and replay protection). These must be addressed immediately before mainnet deployment or further use. The Watchtower pattern implementation logic itself (cancel window) appears sound, but the approval mechanism feeding it is flawed.
