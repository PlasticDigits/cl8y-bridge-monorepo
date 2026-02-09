# Agent Handoff: Complete Bridge Overhaul P0–P3

## Context

You are continuing a bridge architecture overhaul for a cross-chain bridge (EVM ↔ Terra Classic). The master plan is at `docs/BRIDGE_OVERHAUL_BREAKING.md` — read the **"Current Status"** section at the top and **"Section 16: Prioritized Remaining Work"** at the bottom first.

**What's done:** EVM contracts (100%), multichain-rs shared library (100%), operator (100%), canceler (100%).
**What's remaining:** Terra contract V2 alignment, fee system integration, E2E test files, and LOC refactoring.

**Important references:**
- [E2E Failure Analysis & Fixes](./HANDOFF_E2E_FAILURES.md) — Root cause analysis and fix log for all E2E failures. Includes two sessions of comprehensive audits covering ABI mismatches, V2 chain ID confusion, port conflicts, wrong-chain polling, and byte offset bugs across all packages.
- [Cross-Chain Hash Parity](./crosschain-parity.md) — Token encoding, hash computation parity between EVM and Terra.

**Key conventions established during audit:**
- Operator API port: **9091** (not 9090 — avoids LocalTerra gRPC conflict). Configurable via `OPERATOR_API_PORT` env var.
- Canceler health port: **9099**. Configurable via `HEALTH_PORT` env var.
- V2 approval direction rule: EVM→Terra deposits are approved on **Terra**; Terra→EVM and EVM→EVM deposits are approved on **EVM**. Use `poll_terra_for_approval()` vs `poll_for_approval()` accordingly.
- V2 Deposit event nonce: at data offset `[120..128]` (uint64 right-aligned in slot `[96..128]`), NOT `[88..96]`.

---

## Code Quality Rules (ENFORCED)

**Target: 300–600 LOC per file. Hard cap: 900 LOC. Files above 900 LOC must be split.**

### Files Requiring Refactoring (currently > 900 LOC)

| File | Current LOC | Refactoring Plan |
|------|-------------|------------------|
| `packages/e2e/src/setup.rs` | 1323 | Split into `setup/mod.rs` + `setup/evm.rs` + `setup/terra.rs` + `setup/contracts.rs` |
| `packages/e2e/src/tests/integration.rs` | 954 | Split test groups into `tests/integration_deposit.rs` + `tests/integration_withdraw.rs` |
| `packages/multichain-rs/src/testing/user_eoa.rs` | 951 | Split into `testing/user_eoa.rs` (shared + EvmUser, ~500) + `testing/terra_user.rs` (TerraUser, ~450) |

### Files to Monitor (600–900 LOC, split if they grow)

| File | Current LOC | Notes |
|------|-------------|-------|
| `packages/e2e/src/chain_config.rs` | 889 | Consider splitting chain configs by type |
| `packages/e2e/src/terra.rs` | 888 | Consider splitting LCD helpers vs tx helpers |
| `packages/e2e/src/tests/helpers.rs` | 885 | Consider splitting EVM helpers vs Terra helpers |
| `packages/e2e/src/tests/operator_helpers.rs` | 838 | OK if not growing |
| `packages/e2e/src/tests/operator_execution_advanced.rs` | 801 | OK — test file |
| `packages/operator/src/db/mod.rs` | 777 | Consider splitting into `db/queries.rs` + `db/schema.rs` |
| `packages/canceler/src/watcher.rs` | 765 | Consider splitting EVM vs Terra watchers |
| `packages/e2e/src/tokens.rs` | 763 | OK if not growing |
| `packages/contracts-terraclassic/bridge/src/msg.rs` | 742 | Will grow with V2 messages — preemptively split into `msg/execute.rs` + `msg/query.rs` |
| `packages/multichain-rs/src/terra/signer.rs` | 731 | OK — self-contained |
| `packages/multichain-rs/src/evm/watcher.rs` | 730 | OK — self-contained |
| `packages/contracts-evm/test/AccessManagerEnumerable.t.sol` | 736 | OK — test file |
| `packages/contracts-terraclassic/bridge/src/query.rs` | 642 | Will grow with V2 queries — monitor |
| `packages/contracts-terraclassic/bridge/src/execute/config.rs` | 600 | At boundary — monitor |

### Rules for New Code
- Every new file must be 300–600 LOC
- If a change pushes a file past 600 LOC, split before committing
- If a file would exceed 900 LOC, split is mandatory before proceeding
- Prefer many small focused modules over monolithic files
- Always update the parent `mod.rs` when splitting

---

## Task 1 — P0: Terra Contract V2 Withdrawal Flow (HIGHEST PRIORITY)

**Read first:**
- `docs/BRIDGE_OVERHAUL_BREAKING.md` Section 5 (User-Initiated Withdrawal Flow)
- `docs/BRIDGE_OVERHAUL_BREAKING.md` Section 11.2 (Complete method naming reference)
- `packages/contracts-evm/src/Bridge.sol` — the EVM reference implementation (666 LOC)
- `packages/contracts-terraclassic/bridge/src/msg.rs` — current message types
- `packages/contracts-terraclassic/bridge/src/execute/watchtower.rs` — current withdrawal handlers
- `packages/contracts-terraclassic/bridge/src/state.rs` — current state structs

**What to do:**

1. **Add `WithdrawSubmit` to ExecuteMsg** — user-initiated withdrawal on destination chain:
   ```rust
   WithdrawSubmit {
       src_chain: Binary,      // bytes4 source chain ID
       token: String,          // denom or CW20 address
       amount: Uint128,
       nonce: u64,
   }
   ```
   The user calls this (paying gas). It creates a `PendingWithdraw` record with a 5-minute cancel window.

2. **Rename existing handlers to V2 naming:**
   - `ApproveWithdraw` → `WithdrawApprove { withdraw_hash: Binary }` — operator approves (simplified: just the hash, not 8 params)
   - `CancelWithdrawApproval` → `WithdrawCancel { withdraw_hash: Binary }`
   - `ReenableWithdrawApproval` → `WithdrawUncancel { withdraw_hash: Binary }`
   - `ExecuteWithdraw` → split into `WithdrawExecuteUnlock { withdraw_hash: Binary }` + `WithdrawExecuteMint { withdraw_hash: Binary }`

3. **Update `PendingWithdraw` struct in state.rs** to match V2:
   ```rust
   pub struct PendingWithdraw {
       pub src_chain: [u8; 4],
       pub src_account: [u8; 32],
       pub token: String,
       pub recipient: Addr,
       pub amount: Uint128,
       pub nonce: u64,
       pub operator_gas: Uint128,
       pub submitted_at: u64,
       pub approved_at: u64,
       pub approved: bool,
       pub cancelled: bool,
       pub executed: bool,
   }
   ```

4. **Update `execute/watchtower.rs`** — rename file to `execute/withdraw.rs`, implement the new flow:
   - `execute_withdraw_submit()` — creates PendingWithdraw, emits WithdrawSubmit event
   - `execute_withdraw_approve()` — operator verifies deposit, approves (receives gas tip)
   - `execute_withdraw_cancel()` — canceler cancels within window
   - `execute_withdraw_uncancel()` — operator uncancels
   - `execute_withdraw_execute_unlock()` — unlocks tokens after window
   - `execute_withdraw_execute_mint()` — mints tokens after window

5. **Update `contract.rs` routing** to dispatch new message variants.

6. **Update `query.rs`** — rename watchtower query variants to V2 naming.

7. **Update `execute/outgoing.rs` naming** (lower priority, can be done in P1):
   - `Lock` → `DepositNative`
   - Keep `Receive(Cw20ReceiveMsg)` but update internal handling for `DepositCw20Lock`/`DepositCw20MintableBurn`

**LOC budget for new/modified files:**
- `execute/withdraw.rs` (renamed from watchtower.rs): target 400–550 LOC
- `msg.rs`: if it exceeds 900 LOC after changes, split into `msg/execute.rs` + `msg/query.rs` + `msg/mod.rs`
- `state.rs`: should stay under 400 LOC

**Verification:** After changes, run `cargo build` in `packages/contracts-terraclassic/bridge/` and `cargo test`. All existing integration tests must still pass (update them for new naming).

---

## Task 2 — P0: Terra Chain ID System

**Read first:**
- `docs/BRIDGE_OVERHAUL_BREAKING.md` Section 2 (Chain Registry System)
- `packages/contracts-evm/src/ChainRegistry.sol` — EVM reference
- `packages/contracts-terraclassic/bridge/src/state.rs` — current ChainConfig

**What to do:**

1. Update `ChainConfig` to use 4-byte registered chain IDs instead of `chain_id: u64`:
   ```rust
   pub struct ChainConfig {
       pub chain_id: [u8; 4],      // was: u64
       pub identifier: String,      // e.g. "evm_1", "terraclassic_columbus-5"
       pub identifier_hash: [u8; 32], // keccak256(identifier)
       pub enabled: bool,
   }
   ```

2. Add chain registration with auto-incrementing bytes4 IDs (matching EVM `ChainRegistry.registerChain()`).

3. Update all references from `dest_chain_id: u64` to `dest_chain: Binary` (4 bytes) in msg.rs and execute handlers.

4. Update `execute/config.rs` — the `AddChain` handler should become `RegisterChain { identifier: String }`.

**Verification:** `cargo build` and `cargo test` pass.

---

## Task 3 — P0: Terra Fee System Integration

**Read first:**
- `docs/BRIDGE_OVERHAUL_BREAKING.md` Section 3 (Fee System Overhaul)
- `packages/contracts-terraclassic/bridge/src/fee_manager.rs` — existing module (387 LOC, already implemented but unwired)
- `packages/contracts-evm/src/lib/FeeCalculatorLib.sol` — EVM reference

**What to do:**

1. Replace single `fee_bps: u32` in `Config` with a reference to `FeeConfig` from `fee_manager.rs`:
   ```rust
   pub const FEE_CONFIG: Item<FeeConfig> = Item::new("fee_config");
   ```

2. Wire `fee_manager::calculate_fee()` into deposit handlers in `execute/outgoing.rs` — it should be called during deposit to determine fee amount.

3. Add execute message handlers for fee management:
   - `SetFeeParams { standard_fee_bps, discounted_fee_bps, cl8y_threshold, cl8y_token, fee_recipient }` — operator only
   - `SetCustomAccountFee { account, fee_bps }` — operator only, capped at 100 bps (1%)
   - `RemoveCustomAccountFee { account }` — operator only

4. Add query handlers:
   - `FeeConfig {}` — returns current fee parameters
   - `AccountFee { account }` — returns (fee_bps, fee_type) for an account
   - `HasCustomFee { account }` — returns bool

5. Update `InstantiateMsg` to initialize `FeeConfig` instead of single `fee_bps`.

**Verification:** `cargo build`, `cargo test`, and verify fee calculation works with a new unit test.

---

## Task 4 — P1: Terra Deposit Naming Alignment

**What to do:**

1. Rename `Lock { dest_chain_id, recipient }` → `DepositNative { dest_chain: Binary, dest_account: Binary }` in ExecuteMsg.
2. Update CW20 Receive handler internals to distinguish `DepositCw20Lock` vs `DepositCw20MintableBurn` based on token type.
3. Update `dest_account` to use bytes32-encoded universal addresses (from `address_codec.rs`) instead of hex strings.
4. Update `execute/outgoing.rs` handler function names to match.
5. Update integration test for new naming.

---

## Task 5 — P2: E2E Test Files for V2 Features

**Read first:**
- `packages/e2e/src/tests/mod.rs` — existing test module structure
- `packages/multichain-rs/src/testing/user_eoa.rs` — user simulation helpers to use

**What to create (5 new test files, each 300–500 LOC):**

1. `packages/e2e/src/tests/address_codec.rs` — Cross-chain encoding round-trip E2E tests
   - Encode EVM address → bytes32 → decode back
   - Encode Terra address → bytes32 → decode back
   - Verify encoding matches between Rust (multichain-rs) and on-chain contracts

2. `packages/e2e/src/tests/chain_registry.rs` — Chain registration flow E2E
   - Register chain on EVM, verify chain ID assigned
   - Register chain on Terra, verify chain ID assigned
   - Query registered chains on both sides
   - Reject duplicate registration

3. `packages/e2e/src/tests/fee_system.rs` — Fee calculation E2E
   - Standard fee (0.5%) on deposit
   - CL8Y holder discount (0.1%) on deposit
   - Custom per-account fee
   - Fee priority: custom > discount > standard
   - Fee collection to recipient

4. `packages/e2e/src/tests/deposit_flow.rs` — Deposit flow using `multichain-rs::testing::user_eoa`
   - Native deposit (ETH → Terra, LUNA → EVM)
   - ERC20/CW20 lock deposit
   - ERC20/CW20 mintable burn deposit
   - Verify deposit events emitted correctly

5. `packages/e2e/src/tests/withdraw_flow.rs` — Full V2 withdraw cycle
   - User calls `withdrawSubmit` (using `multichain-rs::testing::user_eoa`)
   - Operator calls `withdrawApprove`
   - Wait for cancel window
   - Execute unlock/mint
   - Test cancel during window
   - Test uncancel after cancel

**Don't forget:** Register each new test file in `packages/e2e/src/tests/mod.rs`.

---

## Task 6 — P2: Refactor 900+ LOC Files

### 6a. Split `packages/multichain-rs/src/testing/user_eoa.rs` (951 LOC)

Split into:
- `testing/user_eoa.rs` — `EvmUser` struct + EVM methods + shared helpers (~500 LOC)
- `testing/terra_user.rs` — `TerraUser` struct + Terra methods (~450 LOC)

Update `testing/mod.rs` to export both. Run `cargo test --features full` to verify 73 tests still pass.

### 6b. Split `packages/e2e/src/setup.rs` (1323 LOC)

Split into:
- `setup/mod.rs` — re-exports + `TestEnvironment` struct + `setup()`/`teardown()` orchestration (~300 LOC)
- `setup/evm.rs` — EVM chain setup, contract deployment, account funding (~400 LOC)
- `setup/terra.rs` — Terra chain setup, contract upload/instantiate (~400 LOC)
- `setup/contracts.rs` — contract registration helpers (chain/token registration on both sides) (~300 LOC)

Update all imports across `packages/e2e/src/`. Run `cargo build` in e2e package.

### 6c. Split `packages/e2e/src/tests/integration.rs` (954 LOC)

Split into:
- `tests/integration.rs` — core integration test helpers + shared setup (~300 LOC)
- `tests/integration_deposit.rs` — deposit flow integration tests (~350 LOC)
- `tests/integration_withdraw.rs` — withdraw flow integration tests (~300 LOC)

Update `tests/mod.rs`. Run `cargo test` in e2e package.

---

## Task 7 — P3: Polish

1. **Terra cw2 version tracking** — Add `cw2::set_contract_version()` in instantiate and migrate handlers.
2. **Cross-chain decimal normalization** — Add `src_decimals`/`dest_decimals` fields to `PendingWithdraw` on both chains, convert at execution time.
3. **Update README files** — Brief description of each package and how they fit together.

---

## Execution Order

```
1. Task 6a (refactor user_eoa.rs)           — unblocks clean Terra user testing
2. Task 1 (Terra V2 withdrawal flow)        — largest change, do first
3. Task 2 (Terra chain ID system)           — depends on Task 1 state changes
4. Task 3 (Terra fee system wiring)         — independent of 1/2 but same files
5. Task 4 (Terra deposit naming)            — depends on Task 2 for chain ID format
6. Task 6b (refactor setup.rs)              — unblocks clean E2E test writing
7. Task 6c (refactor integration.rs)        — unblocks clean E2E test writing
8. Task 5 (E2E test files)                  — depends on Tasks 1-4 + 6b/6c
9. Task 7 (polish)                          — last
```

## Verification Checklist

After all tasks, verify:
- [ ] `cargo build` passes in `packages/contracts-terraclassic/bridge/`
- [ ] `cargo test` passes in `packages/contracts-terraclassic/bridge/`
- [ ] `cargo check --features full` passes in `packages/multichain-rs/` (73 tests, 0 warnings)
- [ ] `cargo test --features full` passes in `packages/multichain-rs/`
- [ ] `cargo build` passes in `packages/e2e/`
- [ ] `cargo build` passes in `packages/operator/`
- [ ] `cargo build` passes in `packages/canceler/`
- [ ] No file in the repo exceeds 900 LOC
- [ ] All new files are 300–600 LOC
- [ ] `forge build` passes in `packages/contracts-evm/` (EVM contracts unchanged)
