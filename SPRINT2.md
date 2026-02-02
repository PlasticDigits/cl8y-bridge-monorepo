# Sprint 2: Terra Classic Upgrade Design

**Sprint Duration:** 2 weeks (Weeks 3-4 of plan)  
**Sprint Goal:** Design the approve-delay-cancel pattern with canonical hashing for Terra Classic  
**Predecessor:** Sprint 1 (Documentation) - COMPLETE  
**Reference:** [PLAN_FIX_WATCHTOWER_GAP.md](./PLAN_FIX_WATCHTOWER_GAP.md)

---

## Handoff Summary

### Sprint 1 Completed Deliverables

All documentation foundation work is complete:

| Document | Purpose | Location |
|----------|---------|----------|
| Security Model | Watchtower pattern, roles, canceler infrastructure | [`docs/security-model.md`](./docs/security-model.md) |
| Gap Analysis | Feature comparison, 10 critical gaps, risk matrix | [`docs/gap-analysis-terraclassic.md`](./docs/gap-analysis-terraclassic.md) |
| Cross-Chain Parity | Parity requirements, target state, Rust specs | [`docs/crosschain-parity.md`](./docs/crosschain-parity.md) |
| EVM Contracts (updated) | Security section, canceler role, sequence diagrams | [`docs/contracts-evm.md`](./docs/contracts-evm.md) |
| Architecture (updated) | Trust model, defense layers | [`docs/architecture.md`](./docs/architecture.md) |

### Resolved Decisions (Binding)

These decisions were made in Sprint 1 and should NOT be revisited:

| Decision | Value | Rationale |
|----------|-------|-----------|
| Deprecate `Release`? | **Yes, fully** | Not on mainnet, no migration needed |
| Initial cancelers | **Team-operated only** | Control during launch phase |
| Withdraw delay | **5 minutes** | Match EVM for parity |
| Include rate limiting | **Yes** | Defense in depth |
| Keccak implementation | **cosmwasm-crypto, fallback tiny-keccak** | Best compatibility |
| Store deposit hashes | **Yes, full parity** | Bidirectional verification |
| Address encoding | **Cosmos canonical bytes (20), left-padded to 32** | Matches EVM convention |

---

## Sprint 2 Objectives

### Primary Deliverable

Create `docs/terraclassic-upgrade-spec.md` - a complete technical specification for the Terra Classic contract upgrade.

### Secondary Deliverables

- Updated contract design diagrams
- Hash computation test vectors (for Sprint 3 implementation)

---

## Task Breakdown

### Week 1: Hash & State Design

#### Task 2.1: Canonical TransferId Hash Design

Design the hash computation function that produces identical output to EVM.

**EVM Reference** (`packages/contracts-evm/src/CL8YBridge.sol`):
```solidity
function _computeTransferId(
    bytes32 srcChainKey,
    bytes32 destChainKey,
    bytes32 destTokenAddress,
    bytes32 destAccount,
    uint256 amount,
    uint256 nonce
) internal pure returns (bytes32) {
    return keccak256(abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce));
}
```

**Terra Classic Target**:
```rust
fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    dest_token_address: &[u8; 32],
    dest_account: &[u8; 32],
    amount: Uint128,
    nonce: u64,
) -> [u8; 32]
```

**Critical**: Must match Solidity's `abi.encode` byte layout exactly:
- All values padded to 32-byte slots
- `amount` (u128) → left-pad to 32 bytes, big-endian
- `nonce` (u64) → left-pad to 32 bytes, big-endian

**Subtasks**:
- [ ] Design `compute_transfer_id()` function
- [ ] Design `evm_chain_key(chain_id: u64)` function
- [ ] Design `cosmos_chain_key(chain_id: &str, prefix: &str)` function
- [ ] Design `encode_terra_address(addr: &Addr) -> [u8; 32]` helper
- [ ] Generate test vectors from EVM contract for verification

#### Task 2.2: State Structure Design

Design the new state structures for withdrawal tracking.

**Required Structures**:

```rust
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

pub struct DepositInfo {
    pub dest_chain_key: [u8; 32],
    pub dest_token_address: [u8; 32],
    pub dest_account: [u8; 32],
    pub amount: Uint128,
    pub nonce: u64,
    pub deposited_at: Timestamp,
}

pub struct RateLimitConfig {
    pub max_per_transaction: Uint128,
    pub max_per_period: Uint128,
    pub period_duration: u64,
}
```

**Required State Maps**:

| Map | Key | Value | Purpose |
|-----|-----|-------|---------|
| `WITHDRAW_APPROVALS` | `[u8; 32]` (hash) | `WithdrawApproval` | Track pending/executed approvals |
| `WITHDRAW_NONCE_USED` | `([u8; 32], u64)` | `bool` | Prevent duplicate approvals per source chain |
| `DEPOSIT_HASHES` | `[u8; 32]` (hash) | `DepositInfo` | Enable verification of outgoing deposits |
| `CANCELERS` | `&Addr` | `bool` | Authorized canceler addresses |
| `RATE_LIMITS` | `&str` (token) | `RateLimitConfig` | Per-token rate limits |
| `PERIOD_TOTALS` | `(&str, u64)` | `Uint128` | Track volume per period |

**Subtasks**:
- [ ] Design `WithdrawApproval` struct with all fields
- [ ] Design `DepositInfo` struct for outgoing transfers
- [ ] Design `RateLimitConfig` struct
- [ ] Design state map schemas
- [ ] Document storage key formats

---

### Week 2: Message & Migration Design

#### Task 2.3: Execute Message Design

Design all new execute messages.

**New Messages**:

| Message | Authorization | Purpose |
|---------|---------------|---------|
| `ApproveWithdraw` | Operator | First step, stores approval with delay |
| `ExecuteWithdraw` | Anyone | Complete withdrawal after delay |
| `CancelWithdrawApproval` | Canceler | Block fraudulent approval |
| `ReenableWithdrawApproval` | Admin | Restore false-positive cancellation |
| `AddCanceler` | Admin | Register canceler address |
| `RemoveCanceler` | Admin | Unregister canceler address |
| `SetWithdrawDelay` | Admin | Configure delay period |
| `SetRateLimit` | Admin | Configure per-token limits |

**Message Definitions**:

```rust
pub enum ExecuteMsg {
    ApproveWithdraw {
        src_chain_key: Binary,      // 32 bytes
        token: String,
        recipient: String,
        dest_account: Binary,       // 32 bytes
        amount: Uint128,
        nonce: u64,
        fee: Uint128,
        fee_recipient: String,
        deduct_from_amount: bool,
    },
    ExecuteWithdraw {
        withdraw_hash: Binary,      // 32-byte transferId
    },
    CancelWithdrawApproval {
        withdraw_hash: Binary,
    },
    ReenableWithdrawApproval {
        withdraw_hash: Binary,
    },
    AddCanceler { address: String },
    RemoveCanceler { address: String },
    SetWithdrawDelay { delay_seconds: u64 },
    SetRateLimit {
        token: String,
        max_per_transaction: Uint128,
        max_per_period: Uint128,
        period_duration: u64,
    },
}
```

**Subtasks**:
- [ ] Define all execute message variants
- [ ] Document authorization requirements
- [ ] Document validation rules
- [ ] Document emitted events/attributes

#### Task 2.4: Query Message Design

Design query messages for state inspection.

```rust
pub enum QueryMsg {
    WithdrawApproval { withdraw_hash: Binary },
    ComputeWithdrawHash {
        src_chain_key: Binary,
        dest_chain_key: Binary,
        dest_token_address: Binary,
        dest_account: Binary,
        amount: Uint128,
        nonce: u64,
    },
    DepositHash { nonce: u64 },
    Cancelers {},
    WithdrawDelay {},
    RateLimit { token: String },
}
```

**Subtasks**:
- [ ] Define all query message variants
- [ ] Define response types
- [ ] Document use cases for each query

#### Task 2.5: Error Design

Design error types for new functionality.

```rust
pub enum ContractError {
    // ... existing errors ...
    
    WithdrawNotApproved {},
    ApprovalCancelled {},
    ApprovalAlreadyExecuted {},
    WithdrawDelayNotElapsed { remaining_seconds: u64 },
    NonceAlreadyApproved { src_chain_key: Binary, nonce: u64 },
    NotCanceler {},
    ApprovalNotCancelled {},
    RateLimitExceeded { limit_type: String, limit: Uint128, requested: Uint128 },
}
```

**Subtasks**:
- [ ] Define all new error variants
- [ ] Document when each error is returned

#### Task 2.6: Configuration Updates

Design configuration changes.

```rust
pub struct Config {
    // ... existing fields ...
    pub withdraw_delay: u64,  // seconds, default 300
}
```

**Subtasks**:
- [ ] Document new config fields
- [ ] Document default values
- [ ] Document admin update process

#### Task 2.7: Migration Design

Design the contract migration strategy.

**Migration Requirements**:
- Add `withdraw_delay` to Config (default: 300 seconds)
- Initialize empty `CANCELERS` map
- Initialize empty `WITHDRAW_APPROVALS` map
- No in-flight transaction handling needed (not on mainnet)

**Subtasks**:
- [ ] Design migration entry point
- [ ] Document state changes
- [ ] Document rollback procedure (if possible)

---

## Key File Locations

### Reference Implementation (EVM)

| File | Purpose |
|------|---------|
| `packages/contracts-evm/src/CL8YBridge.sol` | Core bridge with approve/cancel pattern |
| `packages/contracts-evm/src/ChainRegistry.sol` | Chain key computation |
| `packages/contracts-evm/src/TokenRateLimit.sol` | Rate limiting implementation |

**Key EVM Functions to Reference**:
- `_computeTransferId()` - lines 199-208
- `approveWithdraw()` - lines 469-513
- `withdraw()` - lines 540-580
- `cancelWithdrawApproval()` - lines 518-526
- `reenableWithdrawApproval()` - lines 528-538
- `getChainKeyEVM()` / `getChainKeyCosmos()` - ChainRegistry.sol

### Target Implementation (Terra Classic)

| File | Purpose |
|------|---------|
| `packages/contracts-terraclassic/bridge/src/contract.rs` | Main contract logic |
| `packages/contracts-terraclassic/bridge/src/state.rs` | State definitions |
| `packages/contracts-terraclassic/bridge/src/msg.rs` | Message definitions |
| `packages/contracts-terraclassic/bridge/src/error.rs` | Error types |
| `packages/contracts-terraclassic/bridge/Cargo.toml` | Dependencies |

### Documentation

| File | Purpose |
|------|---------|
| `docs/security-model.md` | Security pattern explanation |
| `docs/gap-analysis-terraclassic.md` | Current gaps and risks |
| `docs/crosschain-parity.md` | Parity requirements and Rust specs |
| `docs/contracts-evm.md` | EVM reference with sequence diagrams |

---

## Technical Context

### Keccak256 in CosmWasm

**Option 1 (Preferred)**: cosmwasm-crypto
```toml
[dependencies]
cosmwasm-crypto = "1.5"
```
```rust
use cosmwasm_crypto::keccak256;
```

**Option 2 (Fallback)**: tiny-keccak
```toml
[dependencies]
tiny-keccak = { version = "2.0", features = ["keccak"] }
```
```rust
use tiny_keccak::{Hasher, Keccak};

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}
```

### Address Encoding

Terra addresses must be encoded consistently with EVM's approach:

```rust
fn encode_terra_address(deps: Deps, addr: &Addr) -> StdResult<[u8; 32]> {
    let canonical = deps.api.addr_canonicalize(addr.as_str())?;
    let bytes = canonical.as_slice();
    let mut result = [0u8; 32];
    // Left-pad: 20-byte address goes in last 20 bytes
    result[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(result)
}
```

### abi.encode Byte Layout

Solidity's `abi.encode` produces:
- Each value occupies exactly 32 bytes
- Integers are big-endian, left-padded with zeros
- `bytes32` values are copied as-is

For `abi.encode(srcChainKey, destChainKey, destToken, destAccount, amount, nonce)`:
```
Bytes 0-31:    srcChainKey (32 bytes)
Bytes 32-63:   destChainKey (32 bytes)
Bytes 64-95:   destTokenAddress (32 bytes)
Bytes 96-127:  destAccount (32 bytes)
Bytes 128-159: amount (uint256, left-padded)
Bytes 160-191: nonce (uint256, left-padded)
Total: 192 bytes → keccak256 → 32-byte hash
```

---

## Acceptance Criteria

### Sprint 2 Complete When:

- [ ] `docs/terraclassic-upgrade-spec.md` exists with:
  - [ ] Complete hash computation specification
  - [ ] All state structure definitions
  - [ ] All message definitions (execute + query)
  - [ ] All error definitions
  - [ ] Configuration updates
  - [ ] Migration plan
  - [ ] Test vector examples

- [ ] Contract design diagrams updated showing:
  - [ ] New state relationships
  - [ ] Message flow for approve/execute/cancel

- [ ] Test vectors generated from EVM contract:
  - [ ] At least 5 hash computation test cases
  - [ ] Edge cases (max values, zero values)

### Definition of Done

- Technical spec reviewed for completeness
- Hash computation matches EVM exactly (verified with test vectors)
- All security requirements from gap analysis addressed
- Ready for implementation in Sprint 3

---

## Sprint 3 Preview (Implementation)

After Sprint 2, the next sprint will implement:

1. **New file**: `contracts-terraclassic/bridge/src/hash.rs`
   - `compute_transfer_id()`
   - `evm_chain_key()`
   - `cosmos_chain_key()`
   - Unit tests for hash parity

2. **Updated**: `state.rs`, `msg.rs`, `contract.rs`, `error.rs`
   - All structures and messages from spec
   - Handler implementations
   - Rate limiting logic

3. **Updated**: `Cargo.toml`
   - keccak256 dependency

4. **Migration**: Contract upgrade path

---

## Notes for Next Agent

1. **Start by reading**: `docs/crosschain-parity.md` - contains Rust code specs already drafted
2. **Reference EVM**: The EVM contract is the source of truth for behavior
3. **Hash parity is critical**: Generate test vectors from EVM before finalizing spec
4. **Rate limiting included**: Per the resolved decisions, include rate limiting design
5. **No backward compatibility needed**: `Release` is fully deprecated, clean implementation

Good luck!
