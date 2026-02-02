# Sprint 3: Terra Classic Upgrade Implementation

**Sprint Duration:** 2 weeks  
**Sprint Goal:** Implement the watchtower security pattern in the Terra Classic contract  
**Status:** COMPLETE  
**Predecessor:** Sprint 2 (Design) - COMPLETE  
**Successor:** [Sprint 4](./SPRINT4.md) - Integration Testing & Deployment  
**Reference:** [terraclassic-upgrade-spec.md](./docs/terraclassic-upgrade-spec.md)

---

## Handoff Summary

### Sprint 2 Completed Deliverables

| Document | Location | Purpose |
|----------|----------|---------|
| Technical Specification | [`docs/terraclassic-upgrade-spec.md`](./docs/terraclassic-upgrade-spec.md) | Complete implementation spec |
| Hash Test Vectors | [`packages/contracts-evm/test/HashVectors.t.sol`](./packages/contracts-evm/test/HashVectors.t.sol) | EVM-verified hash outputs |
| Updated Terra Docs | [`docs/contracts-terraclassic.md`](./docs/contracts-terraclassic.md) | Message and state documentation |

### Resolved Decisions (Binding)

These decisions are final and should NOT be revisited:

| Decision | Value | Rationale |
|----------|-------|-----------|
| Deprecate `Release`? | **Yes, fully** | Not on mainnet, clean implementation |
| Rename `RELAYERS` to? | **`OPERATORS`** | Clarity, no migration needed |
| Withdraw delay | **5 minutes (300s)** | Match EVM parity |
| Rate limit period | **24 hours fixed** | Match EVM exactly |
| Keccak implementation | **tiny-keccak** | Guaranteed wasm compatibility |
| Store deposit hashes | **Yes** | Bidirectional verification |
| Address encoding | **20 bytes, left-padded to 32** | Match EVM convention |
| Fee when not deducted | **Recipient sends extra funds** | In `ExecuteWithdraw` message |

---

## Sprint 3 Objectives

### Primary Deliverables

1. **New file:** `src/hash.rs` - Canonical hash computation
2. **Updated:** `src/state.rs` - New state structures
3. **Updated:** `src/msg.rs` - New message types
4. **Updated:** `src/error.rs` - New error variants
5. **Updated:** `src/contract.rs` - Handler implementations
6. **Updated:** `Cargo.toml` - Add tiny-keccak dependency
7. **New:** Integration test comparing EVM and Terra hashes

### Definition of Done

- [x] All hash test vectors pass (match EVM output exactly) - **11 tests passing** (including chain keys)
- [x] `ApproveWithdraw` creates pending approval with delay
- [x] `ExecuteWithdraw` enforces delay and rate limits
- [x] `CancelWithdrawApproval` blocks fraudulent approvals
- [x] `ReenableWithdrawApproval` restores cancelled approvals
- [x] Deposit hash stored on `Lock` for verification
- [x] Contract builds as wasm32-unknown-unknown
- [x] Unit tests for hash module (handlers need integration tests in Sprint 4)

---

## Task Breakdown

### Week 1: Core Implementation

#### Task 3.1: Hash Module (`src/hash.rs`)

**Priority: P0 - Must complete first**

Create the hash computation module. This is the foundation for all verification.

**Implementation:**

```rust
// src/hash.rs
use tiny_keccak::{Hasher, Keccak};
use cosmwasm_std::{Addr, Deps, StdResult};

/// Compute keccak256 hash
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Compute canonical transferId matching EVM's _computeTransferId
pub fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    dest_token_address: &[u8; 32],
    dest_account: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    // Pre-allocate exact size: 6 * 32 = 192 bytes
    let mut data = [0u8; 192];
    
    // Copy fixed-size values
    data[0..32].copy_from_slice(src_chain_key);
    data[32..64].copy_from_slice(dest_chain_key);
    data[64..96].copy_from_slice(dest_token_address);
    data[96..128].copy_from_slice(dest_account);
    
    // uint256 amount - left-padded to 32 bytes, big-endian
    let amount_bytes = amount.to_be_bytes(); // 16 bytes
    data[128 + 16..160].copy_from_slice(&amount_bytes);
    
    // uint256 nonce - left-padded to 32 bytes, big-endian
    let nonce_bytes = nonce.to_be_bytes(); // 8 bytes
    data[160 + 24..192].copy_from_slice(&nonce_bytes);
    
    keccak256(&data)
}

/// Encode Terra address as 32 bytes (left-padded)
pub fn encode_terra_address(deps: Deps, addr: &Addr) -> StdResult<[u8; 32]> {
    let canonical = deps.api.addr_canonicalize(addr.as_str())?;
    let bytes = canonical.as_slice();
    
    let mut result = [0u8; 32];
    let start = 32 - bytes.len();
    result[start..].copy_from_slice(bytes);
    
    Ok(result)
}
```

**Test Vectors (from EVM):**

| Test Case | Expected Hash |
|-----------|---------------|
| All zeros | `0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d` |
| Simple values (1,2,3,4,1e18,42) | `0x7226dd6b664f0c50fb3e50adfa82057dab4819f592ef9d35c08b9c4531b05150` |
| Max values | `0x8433decfd52c831dd32c2ce04d46812b4dc8c2ee057f1edae791837d230be522` |

**Subtasks:**
- [ ] Create `src/hash.rs` with `keccak256()` function
- [ ] Implement `compute_transfer_id()` with exact byte layout
- [ ] Implement `encode_terra_address()` helper
- [ ] Add unit tests verifying all 8 test vectors from EVM
- [ ] Export module in `src/lib.rs`

---

#### Task 3.2: Update Cargo.toml

Add tiny-keccak dependency:

```toml
[dependencies]
# ... existing deps ...
tiny-keccak = { version = "2.0", features = ["keccak"] }
```

**Subtasks:**
- [ ] Add tiny-keccak to dependencies
- [ ] Verify wasm32 target builds successfully

---

#### Task 3.3: State Structures (`src/state.rs`)

Add new state structures and maps.

**New Structures:**

```rust
/// Withdrawal approval tracking
#[cw_serde]
pub struct WithdrawApproval {
    pub src_chain_key: [u8; 32],
    pub token: String,
    pub recipient: Addr,
    pub dest_account: [u8; 32],
    pub amount: Uint128,
    pub nonce: u64,
    pub fee: Uint128,
    pub fee_recipient: Addr,
    pub approved_at: Timestamp,
    pub is_approved: bool,
    pub deduct_from_amount: bool,
    pub cancelled: bool,
    pub executed: bool,
}

/// Deposit info for outgoing transfers
#[cw_serde]
pub struct DepositInfo {
    pub dest_chain_key: [u8; 32],
    pub dest_token_address: [u8; 32],
    pub dest_account: [u8; 32],
    pub amount: Uint128,
    pub nonce: u64,
    pub deposited_at: Timestamp,
}

/// Rate limit window (24h fixed)
#[cw_serde]
pub struct RateLimitWindow {
    pub window_start: Timestamp,
    pub used: Uint128,
}

/// Rate limit configuration per token
#[cw_serde]
pub struct RateLimitConfig {
    pub max_per_transaction: Uint128,
    pub max_per_period: Uint128,
    // period_duration is always 24 hours (86400 seconds)
}
```

**New State Maps:**

```rust
/// Rename: RELAYERS → OPERATORS
pub const OPERATORS: Map<&Addr, bool> = Map::new("operators");
pub const OPERATOR_COUNT: Item<u32> = Item::new("operator_count");

/// New watchtower state
pub const WITHDRAW_DELAY: Item<u64> = Item::new("withdraw_delay");
pub const WITHDRAW_APPROVALS: Map<&[u8], WithdrawApproval> = Map::new("withdraw_approvals");
pub const WITHDRAW_NONCE_USED: Map<(&[u8], u64), bool> = Map::new("withdraw_nonce_used");
pub const DEPOSIT_HASHES: Map<&[u8], DepositInfo> = Map::new("deposit_hashes");
pub const CANCELERS: Map<&Addr, bool> = Map::new("cancelers");
pub const RATE_LIMITS: Map<&str, RateLimitConfig> = Map::new("rate_limits");
pub const RATE_WINDOWS: Map<&str, RateLimitWindow> = Map::new("rate_windows");
```

**Subtasks:**
- [ ] Add `WithdrawApproval` struct
- [ ] Add `DepositInfo` struct  
- [ ] Add `RateLimitConfig` and `RateLimitWindow` structs
- [ ] Rename `RELAYERS` → `OPERATORS` throughout codebase
- [ ] Add all new state maps
- [ ] Update imports in other files

---

#### Task 3.4: Message Types (`src/msg.rs`)

Add new execute and query messages.

**New Execute Messages:**

```rust
pub enum ExecuteMsg {
    // ... existing messages ...
    
    // Watchtower pattern
    ApproveWithdraw {
        src_chain_key: Binary,
        token: String,
        recipient: String,
        dest_account: Binary,
        amount: Uint128,
        nonce: u64,
        fee: Uint128,
        fee_recipient: String,
        deduct_from_amount: bool,
    },
    ExecuteWithdraw {
        withdraw_hash: Binary,
    },
    CancelWithdrawApproval {
        withdraw_hash: Binary,
    },
    ReenableWithdrawApproval {
        withdraw_hash: Binary,
    },
    
    // Canceler management
    AddCanceler { address: String },
    RemoveCanceler { address: String },
    
    // Configuration
    SetWithdrawDelay { delay_seconds: u64 },
    SetRateLimit {
        token: String,
        max_per_transaction: Uint128,
        max_per_period: Uint128,
    },
}
```

**New Query Messages:**

```rust
pub enum QueryMsg {
    // ... existing queries ...
    
    #[returns(WithdrawApprovalResponse)]
    WithdrawApproval { withdraw_hash: Binary },
    
    #[returns(ComputeHashResponse)]
    ComputeWithdrawHash {
        src_chain_key: Binary,
        dest_chain_key: Binary,
        dest_token_address: Binary,
        dest_account: Binary,
        amount: Uint128,
        nonce: u64,
    },
    
    #[returns(Option<DepositInfoResponse>)]
    DepositHash { deposit_hash: Binary },
    
    #[returns(CancelersResponse)]
    Cancelers {},
    
    #[returns(WithdrawDelayResponse)]
    WithdrawDelay {},
    
    #[returns(Option<RateLimitResponse>)]
    RateLimit { token: String },
}
```

**Subtasks:**
- [ ] Add all new ExecuteMsg variants
- [ ] Add all new QueryMsg variants
- [ ] Add response types for new queries
- [ ] Rename `Relayers` query → `Operators`

---

#### Task 3.5: Error Types (`src/error.rs`)

Add new error variants.

```rust
pub enum ContractError {
    // ... existing errors ...
    
    #[error("Withdrawal not approved")]
    WithdrawNotApproved {},
    
    #[error("Withdrawal approval cancelled")]
    ApprovalCancelled {},
    
    #[error("Withdrawal already executed")]
    ApprovalAlreadyExecuted {},
    
    #[error("Withdrawal delay not elapsed: {remaining_seconds} seconds remaining")]
    WithdrawDelayNotElapsed { remaining_seconds: u64 },
    
    #[error("Nonce already approved for source chain")]
    NonceAlreadyApproved { src_chain_key: String, nonce: u64 },
    
    #[error("Approval not cancelled")]
    ApprovalNotCancelled {},
    
    #[error("Caller is not a canceler")]
    NotCanceler {},
    
    #[error("Rate limit exceeded: {limit_type} limit is {limit}, requested {requested}")]
    RateLimitExceeded {
        limit_type: String,
        limit: Uint128,
        requested: Uint128,
    },
    
    #[error("Invalid hash length: expected 32 bytes")]
    InvalidHashLength {},
    
    #[error("Insufficient fee: expected {expected}, got {got}")]
    InsufficientFee { expected: Uint128, got: Uint128 },
}
```

**Subtasks:**
- [ ] Add all new error variants
- [ ] Document when each error is returned

---

### Week 2: Handler Implementation

#### Task 3.6: Contract Handlers (`src/contract.rs`)

Implement all new execute handlers.

**Handler: `execute_approve_withdraw`**

```rust
fn execute_approve_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    src_chain_key: Binary,
    token: String,
    recipient: String,
    dest_account: Binary,
    amount: Uint128,
    nonce: u64,
    fee: Uint128,
    fee_recipient: String,
    deduct_from_amount: bool,
) -> Result<Response, ContractError> {
    // 1. Verify caller is operator
    // 2. Validate inputs (32-byte keys, valid addresses)
    // 3. Check nonce not used for this source chain
    // 4. Compute withdraw hash
    // 5. Store WithdrawApproval with approved_at = now
    // 6. Mark nonce as used
    // 7. Emit WithdrawApproved event
}
```

**Handler: `execute_execute_withdraw`**

```rust
fn execute_execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    // 1. Load approval by hash
    // 2. Verify: is_approved && !cancelled && !executed
    // 3. Verify: block_time >= approved_at + withdraw_delay
    // 4. Check rate limits (24h window)
    // 5. If !deduct_from_amount, verify info.funds covers fee
    // 6. Mark executed = true
    // 7. Calculate net amount (if deduct_from_amount)
    // 8. Transfer tokens to recipient
    // 9. Transfer fee to fee_recipient
    // 10. Emit WithdrawExecuted event
}
```

**Handler: `execute_cancel_withdraw_approval`**

```rust
fn execute_cancel_withdraw_approval(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    // 1. Verify caller is canceler or admin
    // 2. Load approval
    // 3. Verify: is_approved && !cancelled && !executed
    // 4. Set cancelled = true
    // 5. Emit WithdrawApprovalCancelled event
}
```

**Handler: `execute_reenable_withdraw_approval`**

```rust
fn execute_reenable_withdraw_approval(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    // 1. Verify caller is admin
    // 2. Load approval
    // 3. Verify: is_approved && cancelled && !executed
    // 4. Set cancelled = false
    // 5. Reset approved_at = now (restart delay)
    // 6. Emit WithdrawApprovalReenabled event
}
```

**Update: `execute_lock_native` and `execute_receive`**

Add deposit hash storage:

```rust
// In execute_lock_native and execute_receive, after storing transaction:

// Compute and store deposit hash for verification
let deposit_info = DepositInfo {
    dest_chain_key: compute_dest_chain_key(dest_chain_id),
    dest_token_address: compute_token_address(&token),
    dest_account: parse_recipient_as_bytes32(&recipient)?,
    amount: net_amount,
    nonce,
    deposited_at: env.block.time,
};

let deposit_hash = compute_transfer_id(
    &this_chain_key(),
    &deposit_info.dest_chain_key,
    &deposit_info.dest_token_address,
    &deposit_info.dest_account,
    deposit_info.amount.u128(),
    deposit_info.nonce,
);

DEPOSIT_HASHES.save(deps.storage, &deposit_hash, &deposit_info)?;
```

**Subtasks:**
- [ ] Implement `execute_approve_withdraw`
- [ ] Implement `execute_execute_withdraw`
- [ ] Implement `execute_cancel_withdraw_approval`
- [ ] Implement `execute_reenable_withdraw_approval`
- [ ] Implement `execute_add_canceler`
- [ ] Implement `execute_remove_canceler`
- [ ] Implement `execute_set_withdraw_delay`
- [ ] Implement `execute_set_rate_limit`
- [ ] Update `execute_lock_native` to store deposit hash
- [ ] Update `execute_receive` to store deposit hash
- [ ] Rename all `relayer` references to `operator`

---

#### Task 3.7: Query Handlers

Implement new query handlers.

```rust
fn query_withdraw_approval(deps: Deps, env: Env, withdraw_hash: Binary) -> StdResult<WithdrawApprovalResponse> {
    let hash_bytes: [u8; 32] = withdraw_hash.to_vec().try_into()
        .map_err(|_| StdError::generic_err("Invalid hash length"))?;
    
    let approval = WITHDRAW_APPROVALS.may_load(deps.storage, &hash_bytes)?;
    
    match approval {
        Some(a) => {
            let delay = WITHDRAW_DELAY.load(deps.storage)?;
            let elapsed = env.block.time.seconds() - a.approved_at.seconds();
            let remaining = if elapsed >= delay { 0 } else { delay - elapsed };
            
            Ok(WithdrawApprovalResponse {
                exists: true,
                // ... all fields ...
                delay_remaining: remaining,
            })
        }
        None => Ok(WithdrawApprovalResponse { exists: false, ..Default::default() })
    }
}
```

**Subtasks:**
- [ ] Implement `query_withdraw_approval`
- [ ] Implement `query_compute_withdraw_hash`
- [ ] Implement `query_deposit_hash`
- [ ] Implement `query_cancelers`
- [ ] Implement `query_withdraw_delay`
- [ ] Implement `query_rate_limit`

---

#### Task 3.8: Rate Limiting Logic

Implement 24-hour fixed window rate limiting.

```rust
const RATE_LIMIT_PERIOD: u64 = 86400; // 24 hours in seconds

fn check_and_update_rate_limit(
    deps: DepsMut,
    env: &Env,
    token: &str,
    amount: Uint128,
) -> Result<(), ContractError> {
    let config = RATE_LIMITS.may_load(deps.storage, token)?;
    
    let Some(config) = config else {
        return Ok(()); // No limit configured
    };
    
    // Check per-transaction limit
    if !config.max_per_transaction.is_zero() && amount > config.max_per_transaction {
        return Err(ContractError::RateLimitExceeded {
            limit_type: "per_transaction".to_string(),
            limit: config.max_per_transaction,
            requested: amount,
        });
    }
    
    // Check per-period limit
    if config.max_per_period.is_zero() {
        return Ok(()); // No period limit
    }
    
    let mut window = RATE_WINDOWS.may_load(deps.storage, token)?
        .unwrap_or(RateLimitWindow {
            window_start: env.block.time,
            used: Uint128::zero(),
        });
    
    // Reset if window expired (24 hours)
    if env.block.time.seconds() >= window.window_start.seconds() + RATE_LIMIT_PERIOD {
        window = RateLimitWindow {
            window_start: env.block.time,
            used: Uint128::zero(),
        };
    }
    
    let new_used = window.used + amount;
    if new_used > config.max_per_period {
        return Err(ContractError::RateLimitExceeded {
            limit_type: "per_period".to_string(),
            limit: config.max_per_period,
            requested: amount,
        });
    }
    
    window.used = new_used;
    RATE_WINDOWS.save(deps.storage, token, &window)?;
    
    Ok(())
}
```

**Subtasks:**
- [ ] Implement `check_and_update_rate_limit()`
- [ ] Integrate into `execute_execute_withdraw`
- [ ] Add unit tests for rate limiting edge cases

---

#### Task 3.9: Migration Entry Point

Update migration to initialize new state.

```rust
pub const CONTRACT_VERSION: &str = "2.0.0";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    // Initialize withdraw delay (300 seconds = 5 minutes)
    WITHDRAW_DELAY.save(deps.storage, &300u64)?;
    
    // Migrate RELAYERS to OPERATORS
    // (If there's existing data, copy it over)
    
    Ok(Response::new()
        .add_attribute("method", "migrate")
        .add_attribute("version", CONTRACT_VERSION)
        .add_attribute("withdraw_delay", "300"))
}
```

**Subtasks:**
- [ ] Update contract version to 2.0.0
- [ ] Initialize `WITHDRAW_DELAY` with default
- [ ] Handle RELAYERS → OPERATORS migration (if any data exists)

---

#### Task 3.10: Integration Test

Create a test that verifies hash parity between EVM and Terra.

**Approach:** Create a Foundry test that outputs expected values, then a Rust test that computes and compares.

```rust
// tests/hash_parity.rs
#[test]
fn test_hash_parity_with_evm() {
    // Vector 1: All zeros
    let hash = compute_transfer_id(
        &[0u8; 32],
        &[0u8; 32],
        &[0u8; 32],
        &[0u8; 32],
        0,
        0,
    );
    assert_eq!(
        hex::encode(hash),
        "1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d"
    );
    
    // Vector 2: Simple values
    // ... etc for all 8 vectors
}
```

**Subtasks:**
- [ ] Create `tests/hash_parity.rs`
- [ ] Test all 8 vectors from `HashVectors.t.sol`
- [ ] Add to CI

---

## Fee Model Review

### Tradeoffs for `deduct_from_amount = false`

**Option A: Recipient sends fee in ExecuteWithdraw (SELECTED)**

Pros:
- Matches EVM behavior conceptually
- Clear separation: fee is explicit payment
- No hidden deductions from expected amount

Cons:
- Recipient must have native tokens to pay fee
- Extra complexity in frontend (estimate fee, attach funds)

**Implementation:**

```rust
// In execute_execute_withdraw:
if !approval.deduct_from_amount && !approval.fee.is_zero() {
    // Verify caller sent enough native funds for fee
    let sent = info.funds.iter()
        .find(|c| c.denom == "uluna")
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());
    
    if sent < approval.fee {
        return Err(ContractError::InsufficientFee {
            expected: approval.fee,
            got: sent,
        });
    }
    
    // Transfer fee to fee_recipient
    messages.push(BankMsg::Send {
        to_address: approval.fee_recipient.to_string(),
        amount: vec![Coin { denom: "uluna".to_string(), amount: approval.fee }],
    });
}
```

**Option B: Operator pays fee from their funds (NOT SELECTED)**

Would require operator to have Terra balance, adds complexity.

**Decision: Option A is approved.**

---

## Key File Locations

### Implementation Files

| File | Purpose |
|------|---------|
| `packages/contracts-terraclassic/bridge/src/hash.rs` | **NEW** - Hash computation |
| `packages/contracts-terraclassic/bridge/src/state.rs` | State structures |
| `packages/contracts-terraclassic/bridge/src/msg.rs` | Message types |
| `packages/contracts-terraclassic/bridge/src/error.rs` | Error types |
| `packages/contracts-terraclassic/bridge/src/contract.rs` | Handlers |
| `packages/contracts-terraclassic/bridge/Cargo.toml` | Dependencies |

### Reference Files

| File | Purpose |
|------|---------|
| `packages/contracts-evm/src/CL8YBridge.sol` | EVM reference implementation |
| `packages/contracts-evm/test/HashVectors.t.sol` | Hash test vectors |
| `docs/terraclassic-upgrade-spec.md` | Complete specification |

---

## Acceptance Criteria

### Sprint 3 Complete When:

- [x] `cargo build --release --target wasm32-unknown-unknown` succeeds
- [x] All hash test vectors pass (**11 tests**, all 8 vectors verified including chain keys)
- [x] Unit tests pass for hash module
- [x] Integration test confirms hash parity (verified via unit tests against EVM output)

### Manual Testing Checklist

1. **ApproveWithdraw**: Creates approval with correct hash
2. **ExecuteWithdraw before delay**: Fails with `WithdrawDelayNotElapsed`
3. **ExecuteWithdraw after delay**: Succeeds, transfers tokens
4. **CancelWithdrawApproval**: Blocks subsequent execution
5. **ReenableWithdrawApproval**: Restores cancelled approval, resets timer
6. **Rate limit**: Blocks when exceeded, resets after 24h
7. **Lock**: Stores deposit hash for verification

---

## Notes for Next Agent

1. **Start with `src/hash.rs`** - Get hash parity verified FIRST before any other changes
2. **Rename carefully** - `RELAYERS` → `OPERATORS` affects many files; use find-replace
3. **Test vectors are golden** - The hashes in `HashVectors.t.sol` are the source of truth
4. **Rate limit is simple** - 24h fixed window, no configurable period
5. **Fee model is decided** - Recipient pays fee in `ExecuteWithdraw` when `!deduct_from_amount`
6. **No backwards compatibility needed** - Contract not deployed, clean implementation

Good luck!
