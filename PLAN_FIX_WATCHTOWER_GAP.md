# Plan: Fix Watchtower Security Gap

This document outlines the multi-week plan to document, analyze, and fix the security model gap between EVM and Terra Classic contracts.

## Problem Statement

The CL8Y Bridge uses a **watchtower security model** on EVM where:
1. **Relayers** call `approveWithdraw` to approve a withdrawal
2. A **delay period** (default 5 minutes) must pass before execution
3. **Cancelers** can monitor relayer behavior and call `cancelWithdrawApproval` to cancel suspicious approvals during the delay window
4. Users/relayers call `withdraw` after the delay to complete the transfer

This creates a security layer where canceling is **cheap** (just flag the approval) while approving requires the relayer to stake their reputation. Misbehaving relayers can be caught and stopped before funds are released.

**The Problem:** Terra Classic's contract uses **immediate release** - when a relayer calls `Release` with valid signatures, tokens are transferred instantly. There is:
- No approval step
- No delay window
- No cancel functionality
- No watchtower protection

This asymmetry means Terra Classic lacks the same security guarantees as EVM.

---

## Current State Comparison

### EVM Contract (`CL8YBridge.sol`)

| Feature | Implementation |
|---------|----------------|
| `approveWithdraw` | Relayers approve withdrawals with fee terms |
| `withdrawDelay` | 5-minute default delay between approval and execution |
| `withdraw` | Executes only after delay elapses |
| `cancelWithdrawApproval` | Cancelers can cancel suspicious approvals during delay |
| `reenableWithdrawApproval` | Admin can re-enable cancelled approvals |
| `WithdrawApproval` struct | Tracks: `isApproved`, `cancelled`, `executed`, `approvedAt` |

### Terra Classic Contract (`contract.rs`)

| Feature | Implementation |
|---------|----------------|
| `Release` | Single-step immediate release with signature verification |
| Delay mechanism | **MISSING** |
| Cancel functionality | **MISSING** |
| Approval tracking | **MISSING** |
| Canceler role | **MISSING** |

---

## Multi-Week Implementation Plan

### Week 1: Documentation - Security Model

**Objective:** Document the approve-delay-withdraw-cancel security model

**Tasks:**
- [ ] Create `docs/security-model.md` explaining:
  - The watchtower pattern
  - Relayer vs Canceler roles
  - Why delay windows matter
  - Economic incentives (approve is expensive, cancel is cheap)
  - Reorg handling with cancel/reenable
  
- [ ] Update `docs/contracts-evm.md`:
  - Add "Security Model" section
  - Document the canceler role in the Roles table
  - Add sequence diagrams for cancel/reenable flows

- [ ] Update `docs/architecture.md`:
  - Add security model section
  - Explain the trust model

**Deliverables:**
- `docs/security-model.md` (new file)
- Updated `docs/contracts-evm.md`
- Updated `docs/architecture.md`

---

### Week 2: Gap Analysis Document

**Objective:** Formal gap analysis between EVM and Terra Classic contracts

**Tasks:**
- [ ] Create `docs/gap-analysis-terraclassic.md` documenting:
  - Feature comparison table (EVM vs Terra Classic)
  - Missing features on Terra Classic
  - Security implications of current Terra Classic design
  - Risk assessment
  
- [ ] Create `docs/crosschain-parity.md`:
  - Document what parity means for the bridge
  - Define target state for Terra Classic

**Deliverables:**
- `docs/gap-analysis-terraclassic.md`
- `docs/crosschain-parity.md`

---

### Week 3-4: Terra Classic Upgrade Design

**Objective:** Design the approve-delay-cancel pattern for Terra Classic (CosmWasm)

**Tasks:**

#### State Structures
- [ ] Design `WithdrawApproval` struct:
  ```rust
  pub struct WithdrawApproval {
      pub fee: Uint128,
      pub fee_recipient: Addr,
      pub approved_at: Timestamp,
      pub is_approved: bool,
      pub deduct_from_amount: bool,
      pub cancelled: bool,
      pub executed: bool,
  }
  ```
- [ ] Design `PENDING_APPROVALS: Map<String, WithdrawApproval>`
- [ ] Design `CANCELERS: Map<&Addr, bool>`

#### New Messages
- [ ] `ApproveWithdraw` - replaces `Release` as first step
- [ ] `ExecuteWithdraw` - user/relayer calls after delay
- [ ] `CancelWithdrawApproval` - cancelers call
- [ ] `ReenableWithdrawApproval` - admin calls
- [ ] `AddCanceler` / `RemoveCanceler` - admin manages cancelers
- [ ] `SetWithdrawDelay` - admin configures delay

#### Configuration Updates
- [ ] Add `withdraw_delay: u64` to Config
- [ ] Add canceler role management

#### Technical Specification
- [ ] Create `docs/terraclassic-upgrade-spec.md`
- [ ] Include state migrations
- [ ] Include backward compatibility considerations

**Deliverables:**
- `docs/terraclassic-upgrade-spec.md`
- Updated contract design diagrams

---

### Week 5-6: Terra Classic Implementation

**Objective:** Implement the approve-delay-cancel pattern in Terra Classic contract

**Tasks:**

#### Update `state.rs`
- [ ] Add `WithdrawApproval` struct
- [ ] Add `PENDING_APPROVALS: Map<String, WithdrawApproval>`
- [ ] Add `CANCELERS: Map<&Addr, bool>`
- [ ] Add `CANCELER_COUNT: Item<u32>`
- [ ] Add `withdraw_delay` to `Config`

#### Update `msg.rs`
- [ ] Add `ApproveWithdraw` execute message
- [ ] Add `ExecuteWithdraw` execute message
- [ ] Add `CancelWithdrawApproval` execute message
- [ ] Add `ReenableWithdrawApproval` execute message
- [ ] Add `AddCanceler` / `RemoveCanceler` execute messages
- [ ] Add `SetWithdrawDelay` execute message
- [ ] Add `PendingApproval` query message
- [ ] Add `Cancelers` query message

#### Update `contract.rs`
- [ ] Implement `execute_approve_withdraw`
- [ ] Implement `execute_withdraw` (modified from `execute_release`)
- [ ] Implement `execute_cancel_withdraw_approval`
- [ ] Implement `execute_reenable_withdraw_approval`
- [ ] Implement `execute_add_canceler` / `execute_remove_canceler`
- [ ] Implement `execute_set_withdraw_delay`
- [ ] Add delay validation helper

#### Update `error.rs`
- [ ] Add `WithdrawNotApproved` error
- [ ] Add `ApprovalCancelled` error
- [ ] Add `ApprovalExecuted` error
- [ ] Add `WithdrawDelayNotElapsed` error
- [ ] Add `NotCanceller` error
- [ ] Add `ApprovalNotCancelled` error

#### Migration
- [ ] Create migration handler for existing state
- [ ] Ensure backward compatibility during transition
- [ ] Handle in-flight transactions

**Deliverables:**
- Updated `contracts-terraclassic/bridge/src/state.rs`
- Updated `contracts-terraclassic/bridge/src/msg.rs`
- Updated `contracts-terraclassic/bridge/src/contract.rs`
- Updated `contracts-terraclassic/bridge/src/error.rs`
- Migration script

---

### Week 7: Testing & Integration

**Objective:** Comprehensive testing of the upgraded Terra Classic contract

**Tasks:**

#### Unit Tests
- [ ] Test approve flow (happy path)
- [ ] Test delay enforcement (cannot execute before delay)
- [ ] Test cancel flow (canceler can cancel)
- [ ] Test reenable flow (admin can reenable)
- [ ] Test nonce handling (no double approve)
- [ ] Test unauthorized access (non-canceler cannot cancel)
- [ ] Test edge cases:
  - Double approve same nonce
  - Cancel after execute
  - Reenable after execute
  - Execute cancelled approval

#### Integration Tests
- [ ] Test relayer integration with new flow
- [ ] Test canceler monitoring simulation
- [ ] End-to-end test: EVM → Terra with new flow
- [ ] End-to-end test: Terra → EVM (unchanged)

#### Relayer Updates
- [ ] Update Terra writer to use approve-then-execute pattern
- [ ] Add configuration for new flow
- [ ] (Optional) Add canceler functionality to relayer

#### Documentation Updates
- [ ] Update `docs/contracts-terraclassic.md` with new messages
- [ ] Update `docs/crosschain-flows.md` with new diagrams
- [ ] Update `docs/relayer.md` with new flow

**Deliverables:**
- Test suite for new functionality
- Updated relayer (packages/relayer)
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
- [ ] Update `docs/deployment.md`

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

With the watchtower pattern:
1. Cancelers provide an additional security layer
2. Attacks can be detected and stopped during the delay window
3. Economic incentives align: canceling is cheap, so monitoring is viable

### Risk During Transition

- **In-flight transactions**: Handle gracefully during migration
- **Relayer coordination**: Ensure all relayers upgrade simultaneously
- **Canceler availability**: Must have cancelers ready before enabling

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

---

## References

- EVM Contract: `packages/contracts-evm/src/CL8YBridge.sol`
- Terra Contract: `packages/contracts-terraclassic/bridge/src/contract.rs`
- Current EVM Docs: `docs/contracts-evm.md`
- Current Terra Docs: `docs/contracts-terraclassic.md`
- Architecture: `docs/architecture.md`
- Crosschain Flows: `docs/crosschain-flows.md`
