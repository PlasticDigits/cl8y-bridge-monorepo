# Operational Notes & Security Considerations

This document covers operational requirements and constraints that deployers and integrators should be aware of when using the contracts-evm package.

---

## 1. Fee Recipient Must Accept Plain ETH

**Applies to:** `Bridge.setFeeParams`, `depositNative`

When the bridge collects fees on native (ETH) deposits, it forwards ETH to `feeConfig.feeRecipient` via a low-level `call` with `value`. 

**Requirement:** The `feeRecipient` address **must accept plain ETH transfers**. This means:
- **EOAs:** Can always receive ETH.
- **Contracts:** Must implement a `receive()` function or `fallback() external payable` that does not revert on plain ETH receives.

If `feeRecipient` is a contract that reverts on ETH (e.g., a token contract, or a contract with no payable receiver), `depositNative` will revert when fees are collected, blocking all native deposits.

**Recommendation:** Use an EOA or a simple forwarding contract that accepts ETH. Test fee collection before mainnet deployment.

---

## 2. Guard Module Semantics: State Mutations

**Applies to:** `GuardBridge`, guard modules (e.g., `TokenRateLimit`, `BlacklistBasic`)

The `GuardBridge` contract orchestrates guard modules via `checkAccount`, `checkDeposit`, and `checkWithdraw`. Despite the name "check," **guard modules may mutate state**.

For example, `TokenRateLimit.checkDeposit` and `TokenRateLimit.checkWithdraw` update `depositWindowPerToken` and `withdrawWindowPerToken` to track usage within 24h windows. These are not pure view checks—they have side effects.

**Implications for integrators:**
- Calling guard checks multiple times in the same transaction may accumulate state changes.
- Reverted transactions that had already called a guard may have partially updated guard state (depending on revert depth).
- When composing guards, order and idempotency matter.

**Recommendation:** Treat guard `check*` functions as potentially stateful. If integrating guards into a custom flow, ensure each logical operation (e.g., one deposit) results in exactly one guard check call where intended.

---

## 3. Cancel Window Boundary

**Applies to:** `Bridge.withdrawExecuteUnlock`, `Bridge.withdrawExecuteMint`, `_validateWithdrawExecution`

After a withdrawal is approved, there is a **cancel window** (default 5 minutes) during which a canceler can cancel the withdrawal. Withdrawals can only be **executed** after this window has elapsed.

**Boundary semantics:** Execution is allowed when `block.timestamp > approvedAt + cancelWindow` (exclusive). At the exact moment `block.timestamp == approvedAt + cancelWindow`, execution is **not** permitted—the window is treated as still active. Execution becomes permitted in the first block where `block.timestamp` strictly exceeds `windowEnd`.

This is an **exclusive** boundary: execution requires `block.timestamp > windowEnd` (matches `_validateWithdrawExecution` in Bridge.sol).

---

## 4. Unsupported Token Types

**Applies to:** `LockUnlock`, `MintBurn`, token registration

The bridge does **not** support the following token behaviors:

| Token Type | Issue |
|------------|-------|
| **Rebasing tokens** | Balance changes over time; lock/unlock and mint/burn balance checks will fail or produce incorrect accounting. |
| **Fee-on-transfer tokens** | A fee is taken on transfer; the actual amount received differs from the amount sent. `netAmount` and fee calculations assume standard ERC20 semantics. |
| **Inflationary/deflationary tokens** | Similar to rebasing; balance deltas do not match expected amounts. |
| **Tokens with callbacks** | Callbacks (e.g., ERC777) can introduce reentrancy or unexpected behavior. |

`LockUnlock` and `MintBurn` include balance checks before and after transfers to detect some non-standard behavior, but **do not** fully support rebasing, fee-on-transfer, or callback tokens.

**Recommendation:** Only register standard ERC20 tokens that do not modify balances unexpectedly. If in doubt, test with a mock that matches the token’s exact transfer semantics.

---

## 5. ChainRegistry and TokenRegistry: Owner-Only, No Operators

**Applies to:** `ChainRegistry`, `TokenRegistry`

ChainRegistry and TokenRegistry **do not have operators or cancelers**. All chain and token registration is **owner-only**.

**Current behavior:**
- `registerChain`, `unregisterChain` → `onlyOwner`
- `registerToken`, `setTokenDestination`, `setTokenType` → `onlyOwner`

**For deployers:** Use the owner (admin) account for all registry configuration. The Bridge contract has its own operator role (for `withdrawApprove` only), which is separate from the registries.

---

## 6. Wrapped Native (WETH) Provided at Deployment

**Applies to:** `Bridge.initialize`, `depositNative`, deployment scripts

The `wrappedNative` address (WETH, WMATIC, etc.) is **set at deployment only** via `Bridge.initialize(_wrappedNative)`. There is no `setWrappedNative` function—the value cannot be changed after deployment without upgrading the contract.

**Requirement:** When deploying, pass the chain-specific wrapped native token address:
- Use `address(0)` to disable `depositNative` (it will revert with `WrappedNativeNotSet`).
- Use the canonical WETH address for the chain (e.g., WETH9 on Ethereum) to enable native deposits.

**Deploy scripts:**
- `Deploy.s.sol`: Requires `WETH_ADDRESS` environment variable.
- `DeployLocal.s.sol`: Deploys Solady WETH automatically for local testing.

**Note:** The Bridge receives raw ETH on `depositNative` and retains it; it does not wrap to WETH. The `wrappedNative` address is used as the token identifier for cross-chain matching. The destination chain must mint or unlock the wrapped equivalent.

---

## 7. Withdraw Execute Routing: Caller Must Choose Correct Function

**Applies to:** `Bridge.withdrawExecuteUnlock`, `Bridge.withdrawExecuteMint`, integrators

The Bridge does **not** automatically route to `withdrawExecuteUnlock` or `withdrawExecuteMint` based on token type. The caller (recipient or relayer) **must** invoke the correct function for the token:

| Token Type | Function to Call |
|------------|------------------|
| LockUnlock | `withdrawExecuteUnlock` |
| MintBurn | `withdrawExecuteMint` |

**Consequences of calling the wrong function:**
- Calling `withdrawExecuteMint` for a LockUnlock token will revert (token has no `mint`).
- Calling `withdrawExecuteUnlock` for a MintBurn token when LockUnlock has no balance will revert.

**Recommendation:** Integrators must query `TokenRegistry.getTokenType(token)` (or equivalent) and call the appropriate execute function. Document this requirement in integration guides.

---

## 8. Guard–Bridge Integration

**Applies to:** `Bridge`, `GuardBridge`, guard modules (`TokenRateLimit`, `BlacklistBasic`)

The Bridge now integrates with `GuardBridge` via `setGuardBridge(address)`. When a guard bridge is configured (non-zero address):

- **Deposits:** `checkDeposit(token, netAmount, sender)` is called after fee calculation but before lock/burn.
- **Withdraw executions:** `checkWithdraw(token, normalizedAmount, recipient)` is called after decimal normalization but before unlock/mint.

**Guard disabled by default:** `guardBridge` starts as `address(0)`. When disabled, all guard checks are no-ops (zero-cost skip).

**Setting up guards:**
1. Deploy `GuardBridge` via `AccessManagerEnumerable`
2. Register guard modules (`TokenRateLimit`, `BlacklistBasic`) on the `GuardBridge`
3. Call `bridge.setGuardBridge(address(guardBridge))` as owner

**Note:** Guard `check*` calls may mutate state (see §2). The Bridge's `nonReentrant` modifier prevents re-entry but does not prevent guard side effects.

---

## 9. Test Coverage: Standard Infrastructure

**Applies to:** `GuardBridge`, `Create3Deployer`, deployment scripts

The following are **not required** for this package's security posture but are documented for transparency:

| Component | Status | Notes |
|-----------|--------|-------|
| **Guard–Bridge integration tests** | Not required | `GuardBridge` is tested in isolation. Integration with the main Bridge is optional; if used in production, integration tests would be a future enhancement. |
| **Create3Deployer tests** | Not required | Standard Solady CREATE3 wrapper. Deployment scripts use it; direct unit tests are deferred. See CODE_REVIEW.md for coverage details. |
| **Invariant tests** | Implemented | `Bridge.inv.t.sol` covers core invariants (depositNonce monotonicity, registries set, cancel window bounds). |

---

## 10. Create3Deployer (Future: Add Deployment Script Tests)

**Status:** TODO for future work.

`Create3Deployer` is a thin wrapper around Solady's CREATE3 for deterministic deployments. Deployment scripts use it to achieve the same contract addresses across chains. Direct unit tests are intentionally minimal; deployment script verification (e.g., dry-run or fork tests) is a recommended future addition.

---

## 11. Canceler RBAC: Design Divergence from TerraClassic

**Applies to:** `Bridge.withdrawCancel`, cross-chain parity

On **EVM**, the contract owner (admin) can also cancel withdrawals — `withdrawCancel` checks `cancelers[msg.sender] || msg.sender == owner()`. This is a deliberate design choice for operational flexibility: the admin can act as an emergency canceler without a separate transaction to add themselves.

On **TerraClassic**, the admin **cannot** cancel withdrawals; only explicitly registered cancelers can. The admin must first `AddCanceler(their_address)` before they can cancel.

**Rationale:** The EVM approach reduces latency in emergency scenarios where the admin needs to immediately halt a suspicious withdrawal. Since the admin already has full control (pause, upgrade, set cancel window), allowing cancel does not expand their effective authority.

**Cross-chain impact:** None — canceler behavior is local to each chain. A cancel on EVM does not affect TerraClassic and vice versa.

---

## 12. Incoming Token Mapping: Off-Chain Validation

**Applies to:** `Bridge.withdrawSubmit`, cross-chain parity, token registration

On **TerraClassic**, the contract maintains bidirectional token mappings: outgoing (local → dest) and incoming (src → local) via `TOKEN_SRC_MAPPINGS`. The incoming mapping stores `src_decimals`, enabling on-chain validation that the source token is recognized.

On **EVM**, only outgoing mappings are stored in `TokenRegistry`. Incoming token validation (verifying the source chain's token/decimals) is performed **off-chain by the operator** during the `withdrawApprove` step. The `srcDecimals` parameter in `withdrawSubmit` is provided by the submitter and verified by the operator before approval.

**Rationale:** The operator approval step already serves as the trust boundary for incoming withdrawals. Adding a redundant on-chain incoming mapping would increase gas costs and storage complexity without meaningful security improvement, since the operator must validate the entire withdrawal (hash, amount, source chain, token) regardless.

**For operators:** When approving a withdrawal, verify that `srcDecimals` matches the known decimals of the token on the source chain. Reject withdrawals with incorrect `srcDecimals`.

---
