## Bridge Operator Implementation Guide

This document specifies how to implement and operate the centralized `bridgeOperator` service that observes deposits on a source chain and approves corresponding withdrawals on a destination chain, with correct fee semantics, timing, and safety controls.

### Scope

- Implements off-chain logic only; all on-chain business rules live in `Cl8YBridge`, `BridgeRouter`, guard modules, and registries.
- Supports both Mint/Burn and Lock/Unlock paths as configured per token in `TokenRegistry`.
- Supports ERC20 withdrawals and native withdrawals (wrapped-native path via router).

---

## Contracts Overview

- `src/CL8YBridge.sol`: core bridge state machine for deposits, approvals, and withdrawals (with delay and fee semantics).
- `src/BridgeRouter.sol`: user entrypoint that applies guard checks; handles native unwrap and fee split for native path.
- `src/TokenRegistry.sol`: token registration, per-destination-chain token address/decimals, and bridge type selection.
- `src/ChainRegistry.sol`: canonical chain key registry and generators.
- `src/GuardBridge.sol` + guard modules: composable policy checks for accounts, deposits, and withdrawals.

Key artifacts (signatures abridged):

```solidity
// CL8YBridge emits on deposit
event DepositRequest(
    bytes32 indexed destChainKey,
    bytes32 indexed destTokenAddress,
    bytes32 indexed destAccount,
    address token,
    uint256 amount,
    uint256 nonce
);

// Withdrawal approval lifecycle
function approveWithdraw(
    bytes32 srcChainKey,
    address token,
    address to,
    uint256 amount,
    uint256 nonce,
    uint256 fee,
    address feeRecipient,
    bool deductFromAmount
) external;
```

---

## Roles, Addresses, and Configuration

### Authorization

- Grant `BridgeRouter` permission to call `Cl8YBridge.deposit` and `Cl8YBridge.withdraw` (restricted).
- Grant `bridgeOperator` permission to call:
  - `approveWithdraw`, `cancelWithdrawApproval`, `reenableWithdrawApproval`.
  - Optionally `setWithdrawDelay` and guard module management via `GuardBridge`.

### Registry Setup

- `ChainRegistry`: add all supported chains using the canonical generators (e.g., `getChainKeyEVM(chainId)`).
- `TokenRegistry` for each token on each origin chain:
  - `addToken(token, bridgeTypeLocal)` with `MintBurn` or `LockUnlock`.
  - `addTokenDestChainKey(token, destChainKey, destChainTokenAddressBytes32, destChainTokenDecimals)` for every supported destination chain.

### Withdraw Path Policy

- ERC20 path (user receives ERC20): set approval with `deductFromAmount = false`. User pays fee via `msg.value` at withdraw.
- Native path (wrapped-native only): set approval with `deductFromAmount = true` and `to = BridgeRouter` on destination chain. Router unwraps and splits fee from the bridged amount.

### Delay

- Global delay `withdrawDelay` (default 5 minutes) applies between approval time and user execution time. Tune as needed.

---

## End-to-End Flow

### Deposit on Source Chain

1. User calls `BridgeRouter.deposit(token, amount, destChainKey, destAccount)` or `depositNative`.
2. Router performs guard checks and calls `Cl8YBridge.deposit(payer, ...)`.
3. Bridge verifies token/chain, increments nonce, emits `DepositRequest`.
4. Bridge executes token action based on `TokenRegistry`:
   - `MintBurn.burn(payer, token, amount)` or
   - `LockUnlock.lock(payer, token, amount)`.

### Off-Chain Observation (bridgeOperator)

1. Watch `DepositRequest` on the source chain with a chain-appropriate finality threshold.
2. For each finalized event, compute the approval for the destination chain:
   - `srcChainKey`: canonical key for the source chain.
   - `token`: destination chain’s local token address (decode from `destTokenAddress` low 20 bytes for EVM).
   - `to` and `deductFromAmount`:
     - ERC20 path: `to = decode(destAccount)`, `deductFromAmount = false`.
     - Native path: `to = BridgeRouter(destChain)`, `deductFromAmount = true`.
   - `amount`: from event; `nonce`: from event.
   - `fee` and `feeRecipient`: set by policy; `feeRecipient` must be non-zero if `fee > 0`.
3. Call `approveWithdraw(srcChainKey, token, to, amount, nonce, fee, feeRecipient, deductFromAmount)` on destination chain.
4. If deposit is reorged out later, call `cancelWithdrawApproval(withdrawHash)`. If reappears and should be honored, call `reenableWithdrawApproval(withdrawHash)` (resets the delay timer).

### Withdraw on Destination Chain (User)

- ERC20 path:

  1. User calls `BridgeRouter.withdraw(srcChainKey, token, to, amount, nonce)` with `msg.value >= fee`.
  2. Router verifies approval with `deductFromAmount == false`, forwards `msg.value` to bridge, and bridge forwards to `feeRecipient`.
  3. Bridge performs `MintBurn.mint(to, token, amount)` or `LockUnlock.unlock(to, token, amount)`.

- Native path (wrapped-native):
  1. User calls `BridgeRouter.withdrawNative(srcChainKey, amount, nonce, to)`.
  2. Router first calls `Cl8YBridge.withdraw(srcChainKey, wrappedNative, router, amount, nonce)`; approval must match `to = router` and `deductFromAmount = true`.
  3. Router unwraps and splits: `fee` to `feeRecipient`, `amount - fee` to the user `to` address.

---

## Identifiers, Address Encoding, and Idempotency

- `srcChainKey`: compute using the same scheme as `ChainRegistry` (e.g., EVM via `getChainKeyEVM(chainId)`).
- EVM address encoding in `bytes32`: decode low 20 bytes
  - `address(uint160(uint256(bytes32Value)))` for both `destAccount` and `destTokenAddress`.
- Approval uniqueness and idempotency:
  - At destination chain, only one approval is allowed per `(srcChainKey, nonce)`.
  - If a write may have succeeded, first read `getWithdrawApproval(withdrawHash)` before re-submitting.
- Optional off-chain dedupe: compute `depositHash = keccak256(abi.encode(Deposit{...}))` if useful for tracking.

---

## Fee Semantics

- ERC20 path:

  - Set `deductFromAmount = false`.
  - User must supply `msg.value >= fee` when invoking `withdraw`.
  - Bridge forwards `msg.value` to `feeRecipient` after execution.

- Native path:
  - Set `deductFromAmount = true`.
  - Approval must target `to = BridgeRouter` for wrapped-native token.
  - Router unwraps and pays `fee` from the bridged amount, then sends `amount - fee` to user.

Validation rules enforced by bridge:

- If `deductFromAmount = true`, `msg.value` must be zero.
- If `deductFromAmount = false`, `msg.value` must be `>= fee`, and `feeRecipient != address(0)` when any ETH is sent.

---

## Access Control and Guards

- `AccessManager` should authorize:

  - `BridgeRouter` to call restricted bridge entrypoints.
  - `bridgeOperator` to manage approvals and optional delay.
  - Guard management (if owned by ops) via `GuardBridge.add/removeGuardModule*`.

- Guard modules (called by `BridgeRouter`):
  - `checkAccount(account)`: KYC/blacklist/allowlist.
  - `checkDeposit(token, amount, sender)`: per-token/chain/account limits.
  - `checkWithdraw(token, amount, sender)`: withdrawal policy.

---

## Service Architecture (Recommended)

- SourceChainWatcher: subscribes to `DepositRequest`, applies finality, persists jobs.
- ApprovalPlanner: maps deposits to destination approvals, computes `fee`, selects path.
- ApprovalWriter: submits approvals, retries idempotently; supports cancel/reenable.
- StateReader: reads `getWithdrawApproval` and `getWithdrawFromHash` for reconciliation.
- ReorgHandler: monitors canonicality to cancel/reenable as needed.
- Metrics/Alerts: counts, latencies, failed writes, approval-to-execution lag, cancellations.

Data model (suggested):

- Deposits: `(sourceChainId, txHash, logIndex)` and `(srcChainKey, nonce)`.
- Approvals: `(destChainId, withdrawHash)` and `(srcChainKey, nonce)` with status: planned | approved | cancelled | reenabled | executed.

Concurrency:

- Serialize by `(srcChainKey, nonce)` to avoid write conflicts.

---

## Minimal Pseudocode

```ts
// Triggered on finalized DepositRequest event
async function onDeposit(event) {
  const srcChainKey = chainKeyForSource();
  const destChainKey = event.destChainKey;
  const destToken = evmAddressFromBytes32(event.destTokenAddress);
  const destAccount = evmAddressFromBytes32(event.destAccount);
  const amount = event.amount;
  const nonce = event.nonce;

  const path = policy.selectPath(destToken); // "erc20" | "native"
  const fee = fees.compute(destToken, amount, srcChainKey, destChainKey);
  const feeRecipient = config.feeRecipient(destChainKey);

  const approval = {
    srcChainKey,
    token: destToken,
    to: path === "native" ? addresses.bridgeRouter(destChainKey) : destAccount,
    amount,
    nonce,
    fee,
    feeRecipient,
    deductFromAmount: path === "native",
  };

  if (!(await alreadyApproved(srcChainKey, nonce, destChainKey))) {
    const tx = await cl8yBridge(destChainKey).approveWithdraw(
      approval.srcChainKey,
      approval.token,
      approval.to,
      approval.amount,
      approval.nonce,
      approval.fee,
      approval.feeRecipient,
      approval.deductFromAmount
    );
    await tx.wait();
    await markApproved(srcChainKey, nonce, destChainKey, tx.hash);
  }
}
```

---

## Operations Runbook

- Pause/Resume:

  - `Cl8YBridge.pause()` / `unpause()` and/or `BridgeRouter.pause()` / `unpause()`.

- Adjust Delay:

  - `setWithdrawDelay(newDelaySeconds)` as needed for risk/UX balance.

- Bad Approvals:

  - `cancelWithdrawApproval(withdrawHash)` to disable. This can be called during the withdrawDelay window; no delay check applies to cancellation.
  - `reenableWithdrawApproval(withdrawHash)` to re-activate (resets `approvedAt`).

- Common Errors:

  - `WithdrawNotApproved`: approval missing or parameter mismatch.
  - `WithdrawDelayNotElapsed`: user executed before delay elapsed.
  - `IncorrectFeeValue` / `FeeRecipientZero`: fee semantics mismatch at execution vs approval.
  - `NonceAlreadyApproved`: attempted duplicate approval for `(srcChainKey, nonce)`.

- Monitoring:
  - SLA from deposit finality to approval emission.
  - Cancellation and re-enable events.
  - Approval-to-withdraw execution times; alert on outliers.

---

## Testing Checklist

- ERC20 Path:

  - Deposit → approve with `deductFromAmount=false` → withdraw with `msg.value = fee`.
  - Verify fee forwarded and amount minted/unlocked exactly.

- Native Path:

  - Deposit wrapped native → approve with `to=BridgeRouter` and `deductFromAmount=true` → `withdrawNative`.
  - Verify unwrap, fee split to `feeRecipient`, payout to user.

- Reorg Handling:

  - Simulate reorg removing deposit → `cancelWithdrawApproval`.
  - Reappearance → `reenableWithdrawApproval` and ensure delay resets.

- Guards:

  - Trigger rejections via guard modules (blocked account, exceeded limits) on deposit/withdraw.

- Nonce Reuse:
  - Ensure only one approval per `(srcChainKey, nonce)`; duplicates revert.

---

## Notes

- Address decoding for EVM from `bytes32`: `address(uint160(uint256(x)))`.
- Do not modify on-chain fee semantics: approvals define how execution should pay or deduct fees; router and bridge enforce consistency.
