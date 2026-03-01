# Plan: Fix Watchtower Security Gap

This document outlines the multi-week plan to document, analyze, and fix the security model gap between EVM and Terra Classic contracts.

## Problem Statement

The CL8Y Bridge uses a **watchtower security model** on EVM where:
1. **Relayers** call `approveWithdraw` to approve a withdrawal
2. A **delay period** (default 5 minutes) must pass before execution
3. **Cancelers** can monitor relayer behavior and call `cancelWithdrawApproval` to cancel suspicious approvals during the delay window
4. Users/relayers call `withdraw` after the delay to complete the transfer

This creates a security layer where canceling is **cheap** (just flag the approval) while approving requires the relayer to stake their reputation. Misbehaving relayers can be caught and stopped before funds are released.

---

## Key Design Decision: Centralized Operator + Decentralized Cancelers

CL8Y Bridge intentionally uses a **single centralized operator** for approvals, rather than a traditional multisig. This may seem counterintuitive, but it's secured by a **decentralized network of cancelers**:

### Why a Single Operator?

| Benefit | Explanation |
|---------|-------------|
| **Speed** | No waiting for multiple signatures—transactions process immediately |
| **Cost** | One approval transaction instead of N signatures |
| **Simplicity** | Clear accountability; operator reputation is at stake |
| **UX** | Users don't wait for quorum or deal with stuck transactions |

### Why This Is Secure: The Canceler Network

Security comes from the **asymmetry between approving and canceling**:

- **Approving is expensive**: The operator must submit a valid on-chain transaction
- **Canceling is cheap**: Any canceler can flag a suspicious approval with minimal gas
- **One honest canceler wins**: Only one canceler needs to catch fraud to protect users
- **Delay window**: All approvals wait 5 minutes before execution—plenty of time to cancel

### Infrastructure: opBNB + Raspberry Pi

The canceler network is designed for **maximum accessibility**:

| Component | Choice | Why |
|-----------|--------|-----|
| **Chain** | opBNB (BNB Chain L2) | Gas costs are fractions of a cent |
| **Hardware** | Raspberry Pi compatible | ~$50 device, runs 24/7 on minimal power |
| **Bandwidth** | Low | Only needs to monitor events and query source chain |
| **Storage** | Minimal | No blockchain sync required, just RPC access |

This means anyone can run a canceler node—not just well-funded validators. A community member with a Raspberry Pi at home provides the same security guarantee as a professional node operator.

### Security Model Comparison

```
Traditional Multisig:
  ┌─────┐ ┌─────┐ ┌─────┐
  │Sig 1│ │Sig 2│ │Sig 3│ ──► 3-of-5 consensus ──► Execute
  └─────┘ └─────┘ └─────┘
  
  Problem: Slow, expensive, requires coordination

CL8Y Watchtower Model:
  ┌──────────┐
  │ Operator │──► Approve ──► Delay ──► Execute
  └──────────┘                  ↑
                                │
  ┌──────────┐ ┌──────────┐    │
  │Canceler 1│ │Canceler 2│ ───┴──► Cancel if fraud detected
  └──────────┘ └──────────┘
  (Raspberry Pi, opBNB)
  
  Advantage: Fast approval, cheap monitoring, same security
```

**The Problem:** Terra Classic's contract uses **immediate release** - when a relayer calls `Release` with valid signatures, tokens are transferred instantly. There is:
- No approval step
- No delay window
- No cancel functionality
- No watchtower protection
- No canonical hash-based verification

This asymmetry means Terra Classic lacks the same security guarantees as EVM.

---

## Canonical TransferId Hashing Mechanism (EVM)

The EVM contract uses a **canonical hashing mechanism** that enables cross-chain verification and prevents duplicates.

### How It Works

A **TransferId** (hash) is computed identically on both source and destination chains:

```solidity
bytes32 transferId = keccak256(abi.encode(
    srcChainKey,        // Source chain identifier
    destChainKey,       // Destination chain identifier  
    destTokenAddress,   // Token address on destination (bytes32)
    destAccount,        // Recipient account (bytes32)
    amount,             // Normalized amount
    nonce               // Unique nonce from source chain
));
```

### Key Properties

| Property | Mechanism |
|----------|-----------|
| **Uniqueness** | Nonce is included → same transfer cannot be approved twice |
| **Verifiability** | Hash computed from original deposit data → can verify against source chain |
| **Cross-chain consistency** | Same formula on both chains → deposit hash = withdraw hash |
| **Duplicate prevention** | `_withdrawNonceUsed[srcChainKey][nonce]` prevents double-approval per source chain |

### Flow

```
Source Chain (Deposit)                    Destination Chain (Withdraw)
─────────────────────                     ────────────────────────────
1. User deposits tokens                   
2. Contract computes depositHash          
3. Stores in _depositHashes set           
4. Emits DepositRequest event             
                                          5. Relayer observes event
                                          6. Relayer calls approveWithdraw with same params
                                          7. Contract computes withdrawHash (same formula)
                                          8. Hash matches deposit → approval valid
                                          9. Stores approval keyed by withdrawHash
                                          10. After delay, user calls withdraw(withdrawHash)
```

### Verification During Delay Window

Cancelers can verify approvals by:
1. Taking the `withdrawHash` from the `WithdrawApproved` event
2. Querying the source chain's `_depositHashes` set or `getDepositFromHash()`
3. If no matching deposit exists → cancel the approval (fraudulent)
4. If deposit exists but parameters don't match → cancel (manipulation)

### EVM Implementation Reference

```solidity
// Source: CL8YBridge.sol

/// @dev Computes canonical transferId used across deposit and withdraw
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

/// @dev Tracks nonce usage per source chain key to prevent duplicate approvals
mapping(bytes32 srcChainKey => mapping(uint256 nonce => bool used)) private _withdrawNonceUsed;
```

### Terra Classic Gap

The current Terra Classic contract:
- Uses nonce only for replay prevention (`USED_NONCES`)
- Does NOT compute a canonical hash from deposit data
- Does NOT store deposit hashes for verification
- Cannot be verified against source chain state

---

## Current State Comparison

### EVM Contract (`CL8YBridge.sol`)

| Feature | Implementation |
|---------|----------------|
| **Canonical TransferId Hash** | `keccak256(srcChainKey, destChainKey, destToken, destAccount, amount, nonce)` |
| **Deposit Hash Storage** | `_depositHashes` set + `_deposits` mapping for verification |
| **Withdraw Hash Storage** | `_withdrawHashes` set + `_withdraws` mapping |
| **Nonce Tracking** | `_withdrawNonceUsed[srcChainKey][nonce]` prevents duplicates per source chain |
| `approveWithdraw` | Relayers approve withdrawals with fee terms, keyed by withdrawHash |
| `withdrawDelay` | 5-minute default delay between approval and execution |
| `withdraw` | Executes only after delay elapses, looks up by withdrawHash |
| `cancelWithdrawApproval` | Cancelers can cancel suspicious approvals during delay |
| `reenableWithdrawApproval` | Admin can re-enable cancelled approvals |
| `WithdrawApproval` struct | Tracks: `isApproved`, `cancelled`, `executed`, `approvedAt` |
| **Cross-chain Verification** | Cancelers can verify withdrawHash exists on source chain |

### Terra Classic Contract (`contract.rs`)

| Feature | Implementation |
|---------|----------------|
| `Release` | Single-step immediate release with signature verification |
| Canonical TransferId Hash | **MISSING** - no hash computed from deposit data |
| Deposit Hash Storage | **MISSING** - cannot verify against source chain |
| Nonce Tracking | `USED_NONCES` - only prevents replay, not keyed by source chain |
| Delay mechanism | **MISSING** |
| Cancel functionality | **MISSING** |
| Approval tracking | **MISSING** |
| Canceler role | **MISSING** |
| Cross-chain verification | **MISSING** |

---

## Multi-Week Implementation Plan

### Week 1: Documentation - Security Model ✅ COMPLETE

**Objective:** Document the approve-delay-withdraw-cancel security model

**Tasks:**
- [x] Create `docs/security-model.md` explaining:
  - The watchtower pattern
  - Relayer vs Canceler roles
  - Why delay windows matter
  - Economic incentives (approve is expensive, cancel is cheap)
  - Reorg handling with cancel/reenable
  
- [x] Update `docs/contracts-evm.md`:
  - Add "Security Model" section
  - Document the canceler role in the Roles table
  - Add sequence diagrams for cancel/reenable flows

- [x] Update `docs/architecture.md`:
  - Add security model section
  - Explain the trust model

**Deliverables:**
- `docs/security-model.md` (new file) ✅
- Updated `docs/contracts-evm.md` ✅
- Updated `docs/architecture.md` ✅

---

### Week 2: Gap Analysis Document ✅ COMPLETE

**Objective:** Formal gap analysis between EVM and Terra Classic contracts

**Tasks:**
- [x] Create `docs/gap-analysis-terraclassic.md` documenting:
  - Feature comparison table (EVM vs Terra Classic)
  - Missing features on Terra Classic
  - Security implications of current Terra Classic design
  - Risk assessment
  
- [x] Create `docs/crosschain-parity.md`:
  - Document what parity means for the bridge
  - Define target state for Terra Classic

**Deliverables:**
- `docs/gap-analysis-terraclassic.md` ✅
- `docs/crosschain-parity.md` ✅

**Resolved Decisions:**
- `Release` message: Fully deprecated (not on mainnet)
- Initial cancelers: Team-operated only
- Withdraw delay: 5 minutes (match EVM)
- Rate limiting: Include in implementation
- Keccak: cosmwasm-crypto, fallback tiny-keccak
- Deposit hashes on Terra: Yes (full parity)
- Address encoding: Cosmos canonical bytes (20), left-padded to 32

---

### Week 3-4: Terra Classic Upgrade Design

**Objective:** Design the approve-delay-cancel pattern with canonical hashing for Terra Classic (CosmWasm)

**Tasks:**

#### Canonical TransferId Hash Design
- [ ] Design hash computation function matching EVM:
  ```rust
  /// Computes canonical transferId matching EVM's _computeTransferId
  fn compute_transfer_id(
      src_chain_key: &[u8; 32],
      dest_chain_key: &[u8; 32],
      dest_token_address: &[u8; 32],
      dest_account: &[u8; 32],
      amount: Uint128,
      nonce: u64,
  ) -> [u8; 32] {
      // keccak256(abi.encode(...)) equivalent
      let mut data = Vec::new();
      data.extend_from_slice(src_chain_key);
      data.extend_from_slice(dest_chain_key);
      data.extend_from_slice(dest_token_address);
      data.extend_from_slice(dest_account);
      data.extend_from_slice(&amount.to_be_bytes());
      data.extend_from_slice(&nonce.to_be_bytes());
      keccak256(&data)
  }
  ```
- [ ] Design chain key computation:
  ```rust
  /// EVM chain key: keccak256("EVM", chainId)
  fn evm_chain_key(chain_id: u64) -> [u8; 32];
  
  /// Cosmos chain key: keccak256("COSMOS", chain_id, address_prefix)
  fn cosmos_chain_key(chain_id: &str, prefix: &str) -> [u8; 32];
  ```

#### State Structures
- [ ] Design `WithdrawApproval` struct:
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
  ```
- [ ] Design `WITHDRAW_APPROVALS: Map<[u8; 32], WithdrawApproval>` (keyed by transferId hash)
- [ ] Design `WITHDRAW_NONCE_USED: Map<([u8; 32], u64), bool>` (srcChainKey + nonce → used)
- [ ] Design `DEPOSIT_HASHES: Map<[u8; 32], bool>` (for outgoing deposits)
- [ ] Design `CANCELERS: Map<&Addr, bool>`

#### New Messages
- [ ] `ApproveWithdraw` - replaces `Release` as first step, computes and stores hash
  ```rust
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
  }
  ```
- [ ] `ExecuteWithdraw` - user/relayer calls after delay with just the hash
  ```rust
  ExecuteWithdraw {
      withdraw_hash: Binary,  // 32-byte transferId
  }
  ```
- [ ] `CancelWithdrawApproval` - cancelers call
- [ ] `ReenableWithdrawApproval` - admin calls
- [ ] `AddCanceler` / `RemoveCanceler` - admin manages cancelers
- [ ] `SetWithdrawDelay` - admin configures delay

#### Query Messages
- [ ] `WithdrawApproval { withdraw_hash }` - get approval by hash
- [ ] `WithdrawHash { ... }` - compute hash without storing (for verification)
- [ ] `DepositHash { nonce }` - get hash for outgoing deposit
- [ ] `Cancelers {}` - list all cancelers

#### Configuration Updates
- [ ] Add `withdraw_delay: u64` to Config (seconds)
- [ ] Add canceler role management

#### Technical Specification
- [ ] Create `docs/terraclassic-upgrade-spec.md`
- [ ] Include state migrations
- [ ] Include backward compatibility considerations
- [ ] Document hash computation parity with EVM

**Deliverables:**
- `docs/terraclassic-upgrade-spec.md`
- Updated contract design diagrams

---

### Week 5-6: Terra Classic Implementation

**Objective:** Implement the approve-delay-cancel pattern with canonical hashing in Terra Classic contract

**Tasks:**

#### Add `hash.rs` (new file)
- [ ] Add keccak256 dependency (use `sha3` crate or cosmwasm's crypto)
- [ ] Implement `compute_transfer_id()` matching EVM formula
- [ ] Implement `evm_chain_key(chain_id: u64)` 
- [ ] Implement `cosmos_chain_key(chain_id: &str, prefix: &str)`
- [ ] Implement `this_chain_key()` for Terra Classic
- [ ] Add unit tests verifying hash parity with EVM contract

#### Update `state.rs`
- [ ] Add `WithdrawApproval` struct (with all fields for verification)
- [ ] Add `WITHDRAW_APPROVALS: Map<[u8; 32], WithdrawApproval>` (keyed by transferId)
- [ ] Add `WITHDRAW_NONCE_USED: Map<(Vec<u8>, u64), bool>` (srcChainKey + nonce)
- [ ] Add `DEPOSIT_HASHES: Map<[u8; 32], DepositInfo>` (for outgoing deposits)
- [ ] Add `CANCELERS: Map<&Addr, bool>`
- [ ] Add `CANCELER_COUNT: Item<u32>`
- [ ] Add `withdraw_delay: u64` to `Config`

#### Update `msg.rs`
- [ ] Add `ApproveWithdraw` execute message (with all params for hash computation)
- [ ] Add `ExecuteWithdraw { withdraw_hash: Binary }` execute message
- [ ] Add `CancelWithdrawApproval { withdraw_hash: Binary }` execute message
- [ ] Add `ReenableWithdrawApproval { withdraw_hash: Binary }` execute message
- [ ] Add `AddCanceler` / `RemoveCanceler` execute messages
- [ ] Add `SetWithdrawDelay` execute message
- [ ] Add `WithdrawApproval { withdraw_hash }` query message
- [ ] Add `ComputeWithdrawHash { ... }` query message (compute without storing)
- [ ] Add `DepositHash { nonce }` query message
- [ ] Add `Cancelers` query message

#### Update `contract.rs`
- [ ] Implement `execute_approve_withdraw`:
  - Compute transferId hash from params
  - Check `WITHDRAW_NONCE_USED[srcChainKey][nonce]` is false
  - Store approval keyed by hash
  - Mark nonce as used
  - Emit `WithdrawApproved` event with hash
- [ ] Implement `execute_withdraw`:
  - Load approval by hash
  - Verify delay has elapsed: `block.time >= approved_at + withdraw_delay`
  - Mark as executed
  - Transfer tokens
- [ ] Implement `execute_cancel_withdraw_approval`:
  - Verify caller is canceler
  - Load approval by hash
  - Set `cancelled = true`
- [ ] Implement `execute_reenable_withdraw_approval`:
  - Verify caller is admin
  - Load approval by hash
  - Set `cancelled = false`, reset `approved_at` to now
- [ ] Implement `execute_add_canceler` / `execute_remove_canceler`
- [ ] Implement `execute_set_withdraw_delay`
- [ ] Update `execute_lock` to compute and store deposit hash

#### Update `error.rs`
- [ ] Add `WithdrawNotApproved` error
- [ ] Add `ApprovalCancelled` error
- [ ] Add `ApprovalExecuted` error
- [ ] Add `WithdrawDelayNotElapsed { remaining_seconds: u64 }` error
- [ ] Add `NonceAlreadyApproved { src_chain_key: Binary, nonce: u64 }` error
- [ ] Add `NotCanceller` error
- [ ] Add `ApprovalNotCancelled` error
- [ ] Add `WithdrawDataMissing` error

#### Update `Cargo.toml`
- [ ] Add `sha3` crate for keccak256 (or use cosmwasm-crypto if available)

#### Migration
- [ ] Create migration handler for existing state
- [ ] Add default `withdraw_delay` value
- [ ] Initialize empty canceler set
- [ ] Ensure backward compatibility during transition
- [ ] Handle in-flight transactions

**Deliverables:**
- New `contracts-terraclassic/bridge/src/hash.rs`
- Updated `contracts-terraclassic/bridge/src/state.rs`
- Updated `contracts-terraclassic/bridge/src/msg.rs`
- Updated `contracts-terraclassic/bridge/src/contract.rs`
- Updated `contracts-terraclassic/bridge/src/error.rs`
- Updated `contracts-terraclassic/bridge/Cargo.toml`
- Migration script

---

### Week 7: Testing & Integration

**Objective:** Comprehensive testing of the upgraded Terra Classic contract

**Tasks:**

#### Hash Parity Tests (Critical)
- [ ] Create test vectors from EVM contract:
  - Deploy EVM contract to test network
  - Call `getDepositHash()` / `getWithdrawHash()` with known params
  - Record expected hashes
- [ ] Verify Terra Classic produces identical hashes:
  - Same inputs → same 32-byte output
  - Test with various chain IDs, amounts, nonces
  - Test edge cases (max values, zero values)
- [ ] Cross-chain verification test:
  - Create deposit on EVM → get depositHash
  - Create approval on Terra with same params → verify withdrawHash matches

#### Unit Tests
- [ ] Test approve flow (happy path)
- [ ] Test delay enforcement (cannot execute before delay)
- [ ] Test cancel flow (canceler can cancel)
- [ ] Test reenable flow (admin can reenable)
- [ ] Test nonce handling per source chain:
  - Same nonce from different source chains → allowed
  - Same nonce from same source chain → rejected
- [ ] Test unauthorized access (non-canceler cannot cancel)
- [ ] Test edge cases:
  - Double approve same nonce from same source chain
  - Cancel after execute
  - Reenable after execute
  - Execute cancelled approval
  - Approve with mismatched hash components

#### Integration Tests
- [ ] Test operator integration with new flow
- [ ] Test canceler monitoring simulation:
  - Canceler queries source chain for deposit hash
  - Canceler verifies approval matches
  - Canceler cancels invalid approval
- [ ] End-to-end test: EVM → Terra with new flow
- [ ] End-to-end test: Terra → EVM (unchanged)

#### Operator Updates
- [ ] Update Terra writer to use approve-then-execute pattern
- [ ] Update hash computation to match contract
- [ ] Add configuration for new flow
- [ ] (Optional) Add canceler monitoring module to operator

#### Documentation Updates
- [ ] Update `docs/contracts-terraclassic.md` with new messages
- [ ] Update `docs/crosschain-flows.md` with new diagrams
- [ ] Update `docs/operator.md` with new flow
- [ ] Document hash computation for third-party integrators

**Deliverables:**
- Hash parity test suite (critical for security)
- Unit test suite for new functionality
- Integration test suite
- Updated operator (packages/operator)
- Updated documentation

---

### Week 8: Deployment Planning

**Objective:** Prepare for production deployment

**Tasks:**

#### Deployment Checklist
- [ ] Contract migration steps
- [ ] Relayer update coordination
- [ ] Canceler wallet setup
- [ ] Funding canceler accounts
- [ ] Registering cancelers on contract

#### Rollback Plan
- [ ] Define rollback triggers
- [ ] Document rollback procedure
- [ ] Test rollback in staging

#### Operational Runbook
- [ ] How to add/remove cancelers
- [ ] How to adjust withdraw delay
- [ ] Monitoring and alerting for cancellations
- [ ] Incident response for detected misbehavior

#### Documentation
- [ ] Create `docs/deployment-terraclassic-upgrade.md`
- [ ] Create `docs/runbook-cancelers.md`
- [ ] Update `docs/deployment-guide.md`

**Deliverables:**
- `docs/deployment-terraclassic-upgrade.md`
- `docs/runbook-cancelers.md`
- Updated deployment documentation

---

## Summary Table

| Week | Focus | Key Deliverables |
|------|-------|------------------|
| 1 | Documentation | `docs/security-model.md`, updated EVM docs |
| 2 | Gap Analysis | `docs/gap-analysis-terraclassic.md`, `docs/crosschain-parity.md` |
| 3-4 | Design | `docs/terraclassic-upgrade-spec.md` |
| 5-6 | Implementation | Updated Terra Classic contract code |
| 7 | Testing | Test suite, updated relayer, updated docs |
| 8 | Deployment | Deployment runbooks, migration plan |

---

## Security Considerations

### Why This Matters

Without the watchtower pattern on Terra Classic:
1. A compromised relayer could immediately drain funds
2. There's no time window to detect and respond to attacks
3. The only defense is signature threshold (multi-sig)
4. Cannot verify approvals against source chain state

With the watchtower pattern + canonical hashing:
1. Cancelers provide an additional security layer
2. Attacks can be detected and stopped during the delay window
3. Economic incentives align: canceling is cheap, so monitoring is viable
4. **Verification is trivial**: Query source chain for depositHash, compare with withdrawHash
5. **Duplicates impossible**: Hash includes nonce, nonce tracked per source chain

### Canceler Network Infrastructure

The canceler network is intentionally designed for low-cost, decentralized participation:

| Requirement | Specification |
|-------------|---------------|
| **Chain** | opBNB (BNB Chain L2) |
| **Gas Costs** | ~$0.001 per cancel transaction |
| **Hardware** | Raspberry Pi 4 (4GB RAM) or equivalent |
| **Power** | ~5W continuous |
| **Bandwidth** | <1 Mbps (event monitoring only) |
| **Storage** | <1GB (no full node sync) |

**Why opBNB?**
- Gas costs are 100-1000x cheaper than mainnet
- Fast finality (sub-second blocks)
- Secured by BNB Chain validators
- Easy RPC access without running a full node

**Why Raspberry Pi compatibility matters:**
- Removes financial barriers to participation
- Enables geographic distribution (home nodes worldwide)
- Aligns incentives: community members protect their own assets
- No cloud infrastructure costs or dependencies

### Canonical Hash Security Properties

| Attack | Prevention |
|--------|------------|
| **Double-spend** | Nonce included in hash + `WITHDRAW_NONCE_USED` tracking |
| **Amount manipulation** | Amount included in hash, must match source |
| **Recipient manipulation** | `destAccount` included in hash, must match source |
| **Fake source chain** | `srcChainKey` included in hash, verifiable |
| **Cross-chain replay** | Hash includes both `srcChainKey` and `destChainKey` |

### Verification Flow for Cancelers

```
1. Observe WithdrawApproved event on Terra with withdrawHash
2. Query EVM source chain: bridge.getDepositFromHash(withdrawHash)
3. If deposit exists and matches → approval is valid
4. If deposit missing or mismatched → call cancelWithdrawApproval(withdrawHash)
5. All within the 5-minute delay window
```

### Risk During Transition

- **In-flight transactions**: Handle gracefully during migration
- **Relayer coordination**: Ensure all relayers upgrade simultaneously
- **Canceler availability**: Must have cancelers ready before enabling
- **Hash parity**: MUST verify Terra hashes match EVM hashes before deployment

---

## Open Questions

1. **Should `Release` be deprecated or kept for backward compatibility?**
   - Option A: Keep `Release` for a transition period, then remove
   - Option B: Remove immediately and coordinate relayer upgrade

2. **Who will run cancelers initially?**
   - Team-operated cancelers for launch
   - Community cancelers later

3. **What should the default `withdraw_delay` be on Terra Classic?**
   - Match EVM (5 minutes)?
   - Longer due to Terra block times?

4. **Should we add rate limiting to Terra Classic at the same time?**
   - Could be a separate effort
   - Would further align security models

5. **How to handle keccak256 in CosmWasm?**
   - Option A: Use `sha3` crate (pure Rust, larger binary)
   - Option B: Use cosmwasm-crypto if available
   - Option C: Custom implementation (not recommended)

6. **Should deposit hashes be stored on Terra for outgoing transfers?**
   - Enables EVM cancelers to verify Terra → EVM transfers
   - Adds storage cost but improves parity

7. **How to encode addresses in hash computation?**
   - EVM addresses are 20 bytes, padded to 32 bytes
   - Terra addresses are bech32 strings
   - Need consistent encoding: `bytes32(keccak256(terra_address))`?

---

## References

### Code
- EVM Contract: `packages/contracts-evm/src/CL8YBridge.sol`
  - `_computeTransferId()` - lines 199-208
  - `getWithdrawHash()` - lines 363-374
  - `getDepositHash()` - lines 380-390
  - `approveWithdraw()` - lines 469-513
  - `cancelWithdrawApproval()` - lines 518-526
- Terra Contract: `packages/contracts-terraclassic/bridge/src/contract.rs`
  - `execute_release()` - lines 437-568 (current immediate release)

### Documentation
- Current EVM Docs: `docs/contracts-evm.md`
- Current Terra Docs: `docs/contracts-terraclassic.md`
- Architecture: `docs/architecture.md`
- Crosschain Flows: `docs/crosschain-flows.md`

### Key EVM Structs
```solidity
// Withdraw request structure (line 23-36)
struct Withdraw {
    bytes32 srcChainKey;
    address token;
    bytes32 destAccount;
    address to;
    uint256 amount;
    uint256 nonce;
}

// Approval tracking (line 122-130)
struct WithdrawApproval {
    uint256 fee;
    address feeRecipient;
    uint64 approvedAt;
    bool isApproved;
    bool deductFromAmount;
    bool cancelled;
    bool executed;
}
```
