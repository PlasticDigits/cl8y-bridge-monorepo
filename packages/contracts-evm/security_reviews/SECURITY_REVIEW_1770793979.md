# Security Review: contracts-evm Package

**Date:** 2026-02-11 (UTC)  
**Epoch:** 1770793979  
**Reviewer:** Security Review  
**Package:** `packages/contracts-evm`

---

## Executive Summary

This review covers the current state of `packages/contracts-evm`, including source inspection and a full Foundry test run.

- **Test status:** 16 suites, 321 tests passed, 0 failed, 0 skipped.
- **Security findings:** All addressed. M-01 and I-01 fixed; TokenRegistry now reverts on zero `destToken` at write-time.
- **Remediation status:** M-01 (Medium), I-01 (Informational), and TokenRegistry zero-destToken hardening are implemented.
- **Cross-chain:** TerraClassic bridge (CosmWasm) documents v1 never deployed and rate limit purpose; see [TerraClassic OPERATIONAL_NOTES](../../contracts-terraclassic/docs/OPERATIONAL_NOTES.md).

### Severity Summary (Current State)

| Severity | Open | Fixed |
|----------|------|-------|
| High | 0 | — |
| Medium | 0 | 1 (M-01) |
| Low | 0 | — |
| Informational | 0 | 1 (I-01) |

---

## Scope

Primary contracts reviewed:

- `src/Bridge.sol`
- `src/ChainRegistry.sol`
- `src/TokenRegistry.sol`
- `src/LockUnlock.sol`
- `src/MintBurn.sol`
- `src/GuardBridge.sol`
- `src/Create3Deployer.sol`
- `src/interfaces/IBridge.sol`

Test references:

- `test/Bridge.t.sol`
- `test/Bridge.inv.t.sol`
- `test/ChainRegistry.t.sol`
- `test/TokenRegistry.t.sol`
- full suite run via `forge test`

---

## Methodology

1. Manual code review of bridge-critical flows (deposit/withdraw, RBAC, fee flow, hashing).
2. Validation of edge cases and config coupling (chain/token registration, destination mapping, native token flow).
3. Full test execution to confirm runtime behavior.
4. Differential review against prior report expectations.

---

## Findings

## [M-01] Missing destination-token mapping validation in ERC20 deposit flows

**Severity:** Medium  
**Status:** ✅ Fixed  
**Location:** `src/Bridge.sol` (`depositERC20`, `depositERC20Mintable`)

### Description

`depositERC20` and `depositERC20Mintable` fetch `destToken` from `TokenRegistry` and use it to compute the transfer hash, but did not validate that `destToken != bytes32(0)`.

`depositNative` already performed this validation and reverts with `DestTokenMappingNotSet` when missing, but ERC20 paths did not.

### Impact

If a token were registered but destination mapping for a specific `destChain` were unset (or explicitly set to zero), users could deposit and assets would be locked/burned with no valid destination representation.

### Remediation (Implemented)

1. **Bridge.sol** – Both `depositERC20` and `depositERC20Mintable` now read `destToken` before fee transfer/lock/burn and revert with `DestTokenMappingNotSet(token, destChain)` when `destToken == bytes32(0)`.
2. **TokenRegistry.sol** – `setTokenDestination` and `setTokenDestinationWithDecimals` now revert with `InvalidDestToken()` when `destToken == bytes32(0)` (write-time validation).
3. **Tests added:** `test_DepositERC20_RevertsIfDestMappingNotSet`, `test_DepositERC20Mintable_RevertsIfDestMappingNotSet`, `test_SetTokenDestination_RevertsIfDestTokenZero`, `test_SetTokenDestinationWithDecimals_RevertsIfDestTokenZero`.

---

## [I-01] Cancel-window NatSpec comment contradicts runtime behavior

**Severity:** Informational  
**Status:** ✅ Fixed  
**Location:** `src/Bridge.sol` (`_validateWithdrawExecution`)

### Description

The function-level NatSpec previously said execution was allowed at an inclusive boundary (`>= approvedAt + cancelWindow`), while implementation is exclusive: execution allowed only when `block.timestamp > windowEnd`.

### Remediation (Implemented)

NatSpec updated to: "Execution is allowed only when block.timestamp > approvedAt + cancelWindow (exclusive boundary)." Also added `@param` tags for `w` and `xchainHashId`.

---

## [I-02] Trust-model dependency remains operator-centric

**Severity:** Informational  
**Status:** Acknowledged design  
**Location:** `src/Bridge.sol` withdrawal lifecycle

### Description

Withdrawal correctness depends on operator/canceler actions (`withdrawApprove`, `withdrawCancel`, `withdrawUncancel`) rather than on-chain cryptographic proof verification.

### Impact

This is an architecture choice, not a bug. It requires strict operational controls, key management, and monitoring for role holders.

### Recommendation

Maintain strong operator controls (multi-sig, key rotation, alerting, role audits) and keep operational notes aligned with this trust model.

---

## Positive Security Observations

- Reentrancy protection is consistently applied on external state-changing bridge entrypoints.
- Role separation is clear (`onlyOwner`, `onlyOperator`, `onlyCanceler`) with explicit owner override.
- Native and ERC20 deposit paths enforce destination token mapping (Bridge read-time and TokenRegistry write-time).
- Cancel-window execution guard and state transitions are coherent with tests.
- UUPS upgrade authorization is owner-gated across upgradeable contracts.

---

## Test Execution Snapshot

Command run:

- `forge test`

Result:

- **Suites:** 16
- **Tests:** 321 passed, 0 failed, 0 skipped
- Includes invariants from `test/Bridge.inv.t.sol` (4 passing invariants)

---

## Remediation Summary

| Finding | Status |
|---------|--------|
| M-01 – ERC20 deposit destToken validation | ✅ Fixed (Bridge + TokenRegistry write-time) |
| I-01 – Cancel-window NatSpec | ✅ Fixed |
| I-02 – Operator trust model | Acknowledged design; operational hardening only |

---

## Changelog

**vs initial review (this document):**
- M-01, I-01, and TokenRegistry zero-destToken hardening implemented.
- Test count: 321 (added 4 regression tests).

**vs previous EVM Review (`20260211_170530`):**
- M-01 addressed; TokenRegistry now rejects zero `destToken` at write-time.
- I-01 NatSpec aligned with implementation.

---

## Appendix: Related Documentation

| Document | Purpose |
|----------|---------|
| [OPERATIONAL_NOTES.md](../OPERATIONAL_NOTES.md) | Fee recipient, guard semantics, cancel window, unsupported tokens, deployer requirements |
| [TerraClassic OPERATIONAL_NOTES](../../contracts-terraclassic/docs/OPERATIONAL_NOTES.md) | v1 never deployed; rate limit purpose (withdrawal-only, asset-loss mitigation) |
| [SECURITY_REVIEW_20260211_170530.md](./SECURITY_REVIEW_20260211_170530.md) | Prior EVM review (guard integration, Create3Deployer) |

---

*End of Security Review*
