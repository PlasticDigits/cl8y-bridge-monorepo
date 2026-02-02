# Sprint: Watchtower Security Gap - Foundation

**Sprint Duration:** 2 weeks  
**Sprint Goal:** Complete documentation of the security model and gap analysis between EVM and Terra Classic contracts  
**Reference:** [PLAN_FIX_WATCHTOWER_GAP.md](./PLAN_FIX_WATCHTOWER_GAP.md)

---

## Sprint Overview

This sprint covers **Weeks 1-2** of the larger 8-week plan, establishing the documentation foundation needed before implementation begins.

| Week | Focus | Outcome |
|------|-------|---------|
| 1 | Security Model Documentation | Complete understanding of watchtower pattern documented |
| 2 | Gap Analysis & Parity Definition | Clear specification of what Terra Classic needs |

---

## Week 1: Security Model Documentation

### Day 1-2: Create `docs/security-model.md`

**Objective:** Document the watchtower security pattern comprehensively

**Tasks:**
- [ ] **1.1** Create `docs/security-model.md` file structure
- [ ] **1.2** Document the watchtower pattern:
  - Approve → Delay → Execute flow
  - Cancel mechanism during delay window
  - Reenable flow for false positives
- [ ] **1.3** Document Relayer vs Canceler roles:
  - Relayer responsibilities and trust requirements
  - Canceler responsibilities and minimal trust requirements
  - Economic incentives (approve is expensive, cancel is cheap)
- [ ] **1.4** Document why delay windows matter:
  - Attack detection window
  - Cross-chain verification time
  - Reorg handling
- [ ] **1.5** Document the centralized operator + decentralized cancelers model:
  - Why single operator (speed, cost, simplicity, UX)
  - Why this is secure (canceler network asymmetry)
  - Comparison with traditional multisig

**Acceptance Criteria:**
- [ ] `docs/security-model.md` exists and is comprehensive
- [ ] Non-technical stakeholders can understand the security model
- [ ] All diagrams from PLAN doc are included or referenced

---

### Day 3: Document Canceler Infrastructure

**Objective:** Document opBNB + Raspberry Pi infrastructure for cancelers

**Tasks:**
- [ ] **1.6** Add "Canceler Infrastructure" section to `docs/security-model.md`:
  - opBNB chain choice and rationale
  - Hardware requirements (Raspberry Pi compatible)
  - Bandwidth/storage requirements
  - Cost analysis (~$0.001 per cancel)
- [ ] **1.7** Document why accessibility matters:
  - Geographic distribution
  - Community participation
  - No cloud dependencies

**Acceptance Criteria:**
- [ ] Infrastructure requirements are clearly specified
- [ ] Community members understand how to run a canceler

---

### Day 4: Update EVM Contract Documentation

**Objective:** Add security model context to existing EVM docs

**Tasks:**
- [ ] **1.8** Update `docs/contracts-evm.md`:
  - Add "Security Model" section linking to `security-model.md`
  - Add Canceler role to the Roles table
  - Document `cancelWithdrawApproval` function
  - Document `reenableWithdrawApproval` function
- [ ] **1.9** Add sequence diagrams:
  - Normal approve → delay → execute flow
  - Cancel flow during delay window
  - Reenable flow after false positive cancellation
- [ ] **1.10** Document canonical TransferId hash:
  - Hash computation formula
  - Why each field is included
  - Cross-chain verification mechanism

**Acceptance Criteria:**
- [ ] EVM docs include complete security model section
- [ ] All security-related functions are documented
- [ ] Sequence diagrams are clear and accurate

---

### Day 5: Update Architecture Documentation

**Objective:** Integrate security model into overall architecture docs

**Tasks:**
- [ ] **1.11** Update `docs/architecture.md`:
  - Add "Trust Model" section
  - Explain relayer/canceler relationship
  - Document cross-chain security guarantees
- [ ] **1.12** Create architecture diagram showing:
  - Relayer submission path
  - Canceler monitoring path
  - Delay window enforcement
- [ ] **1.13** Review and finalize all Week 1 deliverables

**Deliverables Checklist - Week 1:**
- [ ] `docs/security-model.md` (new file, complete)
- [ ] `docs/contracts-evm.md` (updated with security model)
- [ ] `docs/architecture.md` (updated with trust model)

---

## Week 2: Gap Analysis & Cross-Chain Parity

### Day 6-7: Create Gap Analysis Document

**Objective:** Formal comparison between EVM and Terra Classic contracts

**Tasks:**
- [ ] **2.1** Create `docs/gap-analysis-terraclassic.md`
- [ ] **2.2** Build feature comparison table:

| Feature | EVM | Terra Classic | Gap |
|---------|-----|---------------|-----|
| Canonical TransferId Hash | ✅ `_computeTransferId()` | ❌ Missing | Critical |
| Deposit Hash Storage | ✅ `_depositHashes` | ❌ Missing | Critical |
| Withdraw Hash Storage | ✅ `_withdrawHashes` | ❌ Missing | Critical |
| Nonce per Source Chain | ✅ `_withdrawNonceUsed[srcChainKey][nonce]` | ⚠️ Global only | High |
| Approve Step | ✅ `approveWithdraw` | ❌ Missing | Critical |
| Delay Period | ✅ `withdrawDelay` | ❌ Missing | Critical |
| Cancel Functionality | ✅ `cancelWithdrawApproval` | ❌ Missing | Critical |
| Reenable Functionality | ✅ `reenableWithdrawApproval` | ❌ Missing | High |
| Canceler Role | ✅ Implemented | ❌ Missing | Critical |
| Cross-chain Verification | ✅ Hash-based | ❌ Missing | Critical |

- [ ] **2.3** Document security implications:
  - What attacks are possible without each feature
  - Risk severity for each gap
  - Priority ordering for fixes

**Acceptance Criteria:**
- [ ] All gaps are identified and categorized
- [ ] Risk assessment is complete
- [ ] Priorities are clearly defined

---

### Day 8: Document Risk Assessment

**Objective:** Detailed risk analysis of current Terra Classic state

**Tasks:**
- [ ] **2.4** Add risk assessment section to gap analysis:
  - Compromised relayer attack scenario
  - Double-spend attack scenario
  - Amount/recipient manipulation scenario
- [ ] **2.5** Document current mitigations:
  - What protections exist today (signature verification)
  - Why they're insufficient
- [ ] **2.6** Document target state mitigations:
  - How each gap fix addresses risks
  - Residual risks after implementation

**Acceptance Criteria:**
- [ ] Attack scenarios are clearly described
- [ ] Current vs target security posture is clear

---

### Day 9: Create Cross-Chain Parity Document

**Objective:** Define what parity means and target state for Terra Classic

**Tasks:**
- [ ] **2.7** Create `docs/crosschain-parity.md`
- [ ] **2.8** Define parity requirements:
  - Hash computation must be identical
  - Security guarantees must be equivalent
  - Verification mechanisms must be bidirectional
- [ ] **2.9** Document target state for Terra Classic:
  - Required new messages (ApproveWithdraw, ExecuteWithdraw, Cancel, etc.)
  - Required state structures
  - Required configuration options
- [ ] **2.10** Document verification flow:
  - How EVM cancelers verify Terra deposits
  - How Terra cancelers verify EVM deposits
  - Hash parity requirements

**Acceptance Criteria:**
- [ ] Parity definition is clear and testable
- [ ] Target state is fully specified
- [ ] Verification flows are documented

---

### Day 10: Finalize and Review

**Objective:** Complete all Week 2 deliverables and prepare for implementation

**Tasks:**
- [ ] **2.11** Review all documentation for consistency
- [ ] **2.12** Cross-reference with EVM implementation:
  - Verify feature descriptions match code
  - Verify hash computation documentation is accurate
- [ ] **2.13** Create implementation readiness checklist:
  - [ ] Security model documented
  - [ ] Gaps identified and prioritized
  - [ ] Target state defined
  - [ ] Parity requirements specified
- [ ] **2.14** Update `PLAN_FIX_WATCHTOWER_GAP.md`:
  - Mark Week 1-2 tasks as complete
  - Add any discovered items to later weeks
- [ ] **2.15** Prepare for Week 3-4 design phase:
  - Identify open questions needing resolution
  - Flag any blocking issues

**Deliverables Checklist - Week 2:**
- [ ] `docs/gap-analysis-terraclassic.md` (new file, complete)
- [ ] `docs/crosschain-parity.md` (new file, complete)
- [ ] Updated `PLAN_FIX_WATCHTOWER_GAP.md` with progress

---

## Sprint Summary

### All Deliverables

| Deliverable | Type | Status |
|-------------|------|--------|
| `docs/security-model.md` | New | ✅ Complete |
| `docs/contracts-evm.md` | Update | ✅ Complete |
| `docs/architecture.md` | Update | ✅ Complete |
| `docs/gap-analysis-terraclassic.md` | New | ✅ Complete |
| `docs/crosschain-parity.md` | New | ✅ Complete |

### Definition of Done

- [ ] All documentation is reviewed for accuracy
- [ ] All documentation is reviewed for clarity (non-technical readers can understand)
- [ ] All diagrams are included and render correctly
- [ ] Cross-references between documents are working
- [ ] No blocking issues for Week 3-4 design phase

### Sprint Exit Criteria

Before closing this sprint:
1. All Week 1-2 tasks in `PLAN_FIX_WATCHTOWER_GAP.md` are checked off
2. All new documentation files exist and are complete
3. Implementation team can begin design phase with full context

---

## Notes

### Dependencies
- Access to EVM contract source (`packages/contracts-evm/src/CL8YBridge.sol`)
- Access to Terra contract source (`packages/contracts-terraclassic/bridge/src/contract.rs`)
- Understanding of current documentation structure

### Risks
| Risk | Mitigation |
|------|------------|
| EVM contract changes during sprint | Freeze main branch for reference |
| Unclear hash computation details | Read EVM source directly |
| Missing context for canceler infrastructure | Research opBNB documentation |

### Resolved Decisions

The following questions were resolved before implementation:

| Question | Decision | Rationale |
|----------|----------|-----------|
| Deprecate `Release`? | **Yes, fully deprecate** | Not deployed to mainnet, no migration needed |
| Initial cancelers? | **Team-operated only** | Control during launch phase |
| Default withdraw delay? | **5 minutes** | Match EVM for parity |
| Include rate limiting? | **Yes** | Defense in depth |
| Keccak implementation? | **cosmwasm-crypto, fallback tiny-keccak** | Best compatibility |
| Store deposit hashes on Terra? | **Yes, full parity** | Enables bidirectional verification |
| Address encoding? | **Cosmos canonical bytes (20), left-padded to 32** | Matches EVM padding convention |

These decisions are documented in:
- [Gap Analysis](./docs/gap-analysis-terraclassic.md) - Deprecation and implementation decisions
- [Cross-Chain Parity](./docs/crosschain-parity.md) - Technical specifications
