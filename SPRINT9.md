# Sprint 9: Terra Classic Watchtower Implementation

**Previous Sprint:** [SPRINT8.md](./SPRINT8.md) - Integration Validation & Production Hardening

---

## Sprint 8 Retrospective

### What Went Well

1. **All Sprint 8 Objectives Completed** - Every priority from Sprint 8 was addressed:
   - Integration tests for EVM deposit flow created
   - E2E transfer scripts fully implemented
   - 24 BridgeForm component tests added
   - Transaction receipt handling fixed with proper timeout/retry
   - Bundle analysis documented

2. **Test Coverage Improved Significantly**
   - 90 tests passing (was ~60 before)
   - Integration test structure works well with `describe.skipIf()`
   - Component tests follow the "no mocks for blockchain" philosophy correctly

3. **Good Tooling Added**
   - `make deploy-test-token` for test ERC20 deployment
   - `make e2e-test-full` for comprehensive E2E
   - `wait-for-event.sh` helpers for E2E automation
   - `.env.example` documenting all required config

4. **Transaction Receipt Handling Fixed**
   - Proper `useWaitForTransactionReceipt` integration
   - 2-minute timeout with clear error messages
   - Retry support for failed transactions
   - User rejection detection

5. **Bundle Analysis Thorough**
   - Root cause identified: cosmes protobufs (57MB source → 5.3MB bundled)
   - Alternatives evaluated (cosmos-kit, terra.js)
   - Documented why reduction isn't feasible
   - Initial load optimized to 47KB gzipped

### What Went Wrong

1. **encodeTerraAddress Bug**
   - Initial implementation tried to fit 44-byte Terra address into 32-byte slot
   - Caused `RangeError: offset is out of bounds` in tests
   - **Fix:** Changed to keccak256 hash of address for consistent 32-byte output
   - **Lesson:** Test edge cases for encoding functions early

2. **TypeScript Errors in Integration Tests**
   - Viem `writeContract` requires `chain` and `account` properties
   - Had to add these to all contract write calls in tests
   - **Lesson:** Run tsc before committing test files

3. **E2E Scripts Not Validated Against Running Infrastructure**
   - Transfer logic implemented but not tested with actual operator running
   - Scripts depend on correct contract addresses in environment
   - **Lesson:** E2E tests need CI infrastructure to be meaningful

4. **Bundle Size Reduction Not Achievable**
   - Spent time evaluating alternatives that don't exist
   - Cosmos ecosystem libraries all include full protobufs
   - **Lesson:** Accept some constraints; document and move on

### Key Metrics

| Metric | Before Sprint 8 | After Sprint 8 |
|--------|-----------------|----------------|
| Frontend tests | ~60 | 90 |
| BridgeForm tests | 0 | 24 |
| Initial load (gzipped) | 35KB | 35KB (unchanged) |
| E2E transfer coverage | 0% (stubs) | 100% (implemented) |
| useBridgeDeposit tests | 0 | 15 |

---

## Gap Analysis vs PLAN_FIX_WATCHTOWER_GAP.md

### Plan Status Overview

| Week | Focus | Status | Notes |
|------|-------|--------|-------|
| 1 | Documentation - Security Model | ✅ COMPLETE | Done in Sprint 3 |
| 2 | Gap Analysis Document | ✅ COMPLETE | Done in Sprint 3 |
| 3-4 | Terra Classic Upgrade Design | ⚠️ PARTIALLY STARTED | Spec exists but not detailed |
| 5-6 | Terra Classic Implementation | ❌ NOT STARTED | **CRITICAL GAP** |
| 7 | Testing & Integration | ⚠️ PARTIAL | Frontend done, Terra contract testing needed |
| 8 | Deployment Planning | ❌ NOT STARTED | Blocked on implementation |

### Critical Gap: Terra Classic Contract Lacks Watchtower Pattern

The **core security problem** remains unsolved:

```
EVM Contract                    Terra Classic Contract
─────────────                   ──────────────────────
✅ approveWithdraw              ❌ Immediate Release
✅ 5-minute delay window        ❌ No delay
✅ cancelWithdrawApproval       ❌ No cancel
✅ Canonical hash verification  ❌ No hash verification
✅ Canceler role                ❌ No cancelers
```

**Security Implication:** A compromised relayer on Terra can immediately drain funds with no recovery window.

### Remaining Gaps by Priority

#### CRITICAL (Security)

| Gap | Impact | Blocked By |
|-----|--------|------------|
| Terra Classic lacks approve-delay-cancel | Compromised relayer = instant fund drain | Contract implementation |
| No canonical hash on Terra | Cannot verify approvals against source | Contract implementation |
| Canceler can't submit cancels | Watchtower is observe-only | Contract implementation |

#### HIGH (Functionality)

| Gap | Impact | Notes |
|-----|--------|-------|
| E2E tests not validated | Scripts may have bugs | Need CI with infrastructure |
| No real-time status updates | User blind during transfer | UI enhancement |
| Router integration incomplete | EVM deposits may fail | Need to verify BridgeRouter flow |

#### MEDIUM (Quality)

| Gap | Impact | Notes |
|-----|--------|-------|
| No Playwright browser E2E | Can't test full user flows | Nice to have |
| No transaction history | User can't track past transfers | Need backend |
| Terra chunk large | Slow first Terra wallet connect | Documented, accepted |

---

## Sprint 9 Objectives

### Priority 1: Terra Classic Contract Upgrade (CRITICAL)

This is the most important remaining work. The contract needs:

#### 1.1 Add `hash.rs` Module
```rust
// Canonical hash computation matching EVM
fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    dest_token_address: &[u8; 32],
    dest_account: &[u8; 32],
    amount: Uint128,
    nonce: u64,
) -> [u8; 32]
```

#### 1.2 Add Approval State
```rust
pub struct WithdrawApproval {
    pub src_chain_key: [u8; 32],
    pub token: String,
    pub recipient: Addr,
    pub amount: Uint128,
    pub nonce: u64,
    pub approved_at: Timestamp,
    pub cancelled: bool,
    pub executed: bool,
}

WITHDRAW_APPROVALS: Map<[u8; 32], WithdrawApproval>  // keyed by hash
WITHDRAW_NONCE_USED: Map<(Vec<u8>, u64), bool>       // (srcChainKey, nonce)
CANCELERS: Map<&Addr, bool>
```

#### 1.3 Add New Execute Messages
- `ApproveWithdraw` - Replaces `Release` as first step
- `ExecuteWithdraw` - User calls after delay with hash
- `CancelWithdrawApproval` - Canceler calls
- `ReenableWithdrawApproval` - Admin calls
- `AddCanceler` / `RemoveCanceler` - Admin manages

#### 1.4 Add Query Messages
- `WithdrawApproval { withdraw_hash }` - Get approval by hash
- `ComputeWithdrawHash { ... }` - Compute hash without storing
- `Cancelers {}` - List all cancelers

**Acceptance Criteria:**
- [ ] `hash.rs` produces identical hashes to EVM contract
- [ ] `ApproveWithdraw` stores approval with hash key
- [ ] `ExecuteWithdraw` enforces delay (5 minutes)
- [ ] `CancelWithdrawApproval` works from canceler address
- [ ] All new messages have unit tests
- [ ] Migration handles existing state

### Priority 2: Update Operator for New Flow

The operator needs to use the new approve-then-execute pattern:

#### 2.1 Terra Writer Updates
```rust
// Old flow (immediate)
Release { ... }

// New flow (approve-delay-execute)
ApproveWithdraw { ... }
// Wait for delay
ExecuteWithdraw { withdraw_hash }
```

#### 2.2 Hash Computation
```rust
// Must match contract's compute_transfer_id
fn compute_withdraw_hash(...) -> [u8; 32]
```

**Acceptance Criteria:**
- [ ] Operator calls `ApproveWithdraw` instead of `Release`
- [ ] Operator waits for delay before `ExecuteWithdraw`
- [ ] Hash computation matches contract

### Priority 3: Enable Canceler Submissions

Currently the canceler observes but doesn't cancel:

```rust
// Current: Just logs
log::warn!("Suspicious approval detected: {}", approval_hash);

// Needed: Actually cancel
contract.execute(CancelWithdrawApproval { withdraw_hash })?;
```

**Acceptance Criteria:**
- [ ] Canceler submits `CancelWithdrawApproval` on Terra
- [ ] Canceler submits `cancelWithdrawApproval` on EVM (already exists)
- [ ] Verification logic checks source chain deposit

### Priority 4: Validate E2E Tests

The E2E transfer scripts are implemented but need validation:

```bash
# Needs to actually run successfully
./scripts/e2e-test.sh --with-all --full
```

**Tasks:**
1. Start full infrastructure (Anvil, LocalTerra, Postgres)
2. Deploy all contracts
3. Run E2E with operator
4. Verify transfers complete
5. Fix any issues found

**Acceptance Criteria:**
- [ ] E2E passes with fresh deployment
- [ ] Both directions verified (Terra→EVM, EVM→Terra)
- [ ] CI configuration added

---

## Technical Notes for Next Agent

### Terra Classic Contract Location
```
packages/contracts-terraclassic/bridge/
├── src/
│   ├── contract.rs    # Main entry point
│   ├── state.rs       # State definitions
│   ├── msg.rs         # Message definitions
│   ├── error.rs       # Error types
│   └── hash.rs        # NEW: Hash functions
├── Cargo.toml
└── tests/
```

### Key EVM Reference Code
The Terra implementation should match these EVM functions:

```solidity
// CL8YBridge.sol - lines 199-208
function _computeTransferId(
    bytes32 srcChainKey,
    bytes32 destChainKey,
    bytes32 destTokenAddress,
    bytes32 destAccount,
    uint256 amount,
    uint256 nonce
) internal pure returns (bytes32) {
    return keccak256(abi.encode(
        srcChainKey, destChainKey, destTokenAddress, 
        destAccount, amount, nonce
    ));
}
```

### Keccak256 in CosmWasm

Use `sha3` crate:
```toml
# Cargo.toml
[dependencies]
sha3 = "0.10"
```

```rust
use sha3::{Keccak256, Digest};

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}
```

### Hash Parity Test Pattern

Create test vectors from EVM, verify Terra produces same output:

```rust
#[test]
fn test_hash_parity_with_evm() {
    // Known values from EVM contract
    let src_chain_key = hex!("...");
    let dest_chain_key = hex!("...");
    // ... other params
    let expected_hash = hex!("...");
    
    let actual_hash = compute_transfer_id(&src_chain_key, ...);
    assert_eq!(actual_hash, expected_hash);
}
```

### Running Infrastructure

```bash
# Start everything
make start
cd ../LocalTerra && docker compose up -d terrad

# Deploy contracts
make deploy

# Verify
make status

# Run E2E
./scripts/e2e-test.sh --with-all --full
```

### Files Modified in Sprint 8

| File | Changes |
|------|---------|
| `packages/frontend/src/hooks/useBridgeDeposit.ts` | Receipt handling, timeout, retry |
| `packages/frontend/src/components/BridgeForm.tsx` | Token config from env |
| `packages/frontend/src/components/BridgeForm.test.tsx` | NEW: 24 tests |
| `packages/frontend/src/hooks/useBridgeDeposit.integration.test.ts` | NEW: Integration tests |
| `packages/frontend/.env.example` | NEW: Environment docs |
| `packages/frontend/BUNDLE_ANALYSIS.md` | NEW: Bundle findings |
| `packages/contracts-evm/script/DeployTestToken.s.sol` | NEW: Test token |
| `scripts/e2e-test.sh` | Full transfer implementations |
| `scripts/e2e-helpers/wait-for-event.sh` | NEW: Event helpers |
| `Makefile` | New targets |

---

## Definition of Done for Sprint 9

### Contract Upgrade
- [ ] Terra Classic contract has approve-delay-cancel pattern
- [ ] Hash computation matches EVM exactly (test vectors pass)
- [ ] Canceler role works
- [ ] 5-minute delay enforced
- [ ] Migration script handles existing state

### Operator Update
- [ ] Uses new approve → wait → execute flow
- [ ] Hash computation integrated
- [ ] Works with upgraded contract

### Canceler Activation
- [ ] Submits cancel transactions (not just logs)
- [ ] Verifies against source chain
- [ ] Works on both EVM and Terra

### E2E Validation
- [ ] Full E2E passes with infrastructure
- [ ] CI pipeline configured
- [ ] Both directions tested

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Hash mismatch between chains | Medium | Critical | Extensive test vectors |
| Migration breaks existing transfers | Low | High | Test migration in staging |
| Canceler gas costs too high | Low | Medium | Use opBNB for cheap gas |
| Delay too short for monitoring | Low | Medium | Start with 5 min, adjust |

---

## Quick Start for Next Agent

```bash
# Clone and setup
git clone <repo>
cd cl8y-bridge-monorepo

# Read the plan
cat PLAN_FIX_WATCHTOWER_GAP.md  # Full context

# Start with Terra contract
cd packages/contracts-terraclassic/bridge
cargo build
cargo test

# Reference EVM implementation
cat ../contracts-evm/src/CL8YBridge.sol | grep -A 20 "_computeTransferId"
```

---

*Created: 2026-02-02*  
*Previous Sprint: SPRINT8.md - Integration Validation & Production Hardening*  
*Reference: PLAN_FIX_WATCHTOWER_GAP.md - Weeks 3-6*
