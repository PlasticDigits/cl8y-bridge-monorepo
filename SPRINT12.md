# Sprint 12: Operator Fix & Production Readiness

**Previous Sprint:** [SPRINT11.md](./SPRINT11.md) - Operator Integration & Real Transfer Tests

---

## CRITICAL: Priority 0 - Fix Operator Startup

### Root Cause Analysis

**The operator fails to start due to a Tendermint version mismatch.**

```
Error: subtle encoding error / bad encoding at line 1 column 2161
Location: src/watchers/terra.rs:107
```

**Investigation Findings:**

| Component | Version | Issue |
|-----------|---------|-------|
| `tendermint-rpc` crate | **0.35** | Expects old ABCI 1.0 format |
| LocalTerra Tendermint | **0.38.19** | Uses new ABCI 2.0 format |

**The Breaking Change:**

Tendermint 0.38 (ABCI 2.0) changed the `block_results` response format:

```json
// OLD (Tendermint 0.34-0.37) - What tendermint-rpc 0.35 expects
{
  "begin_block_events": [...],
  "end_block_events": [...],
  "txs_results": [...]
}

// NEW (Tendermint 0.38+) - What LocalTerra returns
{
  "finalize_block_events": [...],  // Combined, replaces begin/end
  "txs_results": [...]
}
```

The `tendermint-rpc` 0.35 crate cannot parse the new format, causing the JSON decode error.

### Fix Options

#### Option A: Upgrade tendermint-rpc to 0.38 (Recommended)

```toml
# packages/operator/Cargo.toml
tendermint = "0.38"
tendermint-rpc = { version = "0.38", features = ["http-client"] }
```

**Pros:** Matches LocalTerra, future-proof
**Cons:** May require API changes in Terra watcher code

#### Option B: Skip block_results call

The `block_results` call on line 103-107 is marked "kept for future use" and the actual work uses LCD queries. We could remove/skip it:

```rust
// Current code (fails):
let _block_results = self.rpc_client.block_results(...).await?;

// Quick fix:
// Remove or comment out - we don't use the result anyway
```

**Pros:** Quick fix, no dependency changes
**Cons:** Loses potential functionality, doesn't fix underlying issue

#### Option C: Downgrade LocalTerra

Use an older LocalTerra image with Tendermint < 0.38.

**Pros:** No code changes
**Cons:** May break Terra Classic compatibility, not recommended

### Recommended Fix Implementation

1. **Update Cargo.toml:**
   ```toml
   tendermint = "0.38"
   tendermint-rpc = { version = "0.38", features = ["http-client"] }
   ```

2. **Update terra.rs if needed:**
   - Check if `block_results` API changed
   - Update any event parsing for `finalize_block_events`

3. **Test operator startup:**
   ```bash
   cd packages/operator && cargo build --release
   source .env.e2e && ./target/release/cl8y-relayer
   ```

---

## Sprint 11 Retrospective (Updated)

### What Actually Works Now

1. **E2E Setup is Fully Automated:**
   - Token deployment (ERC20 + CW20) happens automatically
   - OPERATOR_ROLE and CANCELER_ROLE granted in setup
   - Environment file generated with all addresses
   - Operator startup attempted (but fails - see Priority 0)

2. **All 10 E2E Tests Pass:**
   - EVM/Terra connectivity ✓
   - Bridge configuration (300s delay) ✓
   - Watchtower approve → delay → execute flow ✓
   - Watchtower cancel flow ✓
   - Hash parity tests ✓

3. **Documentation Complete:**
   - `docs/deployment-terraclassic-upgrade.md`
   - `docs/runbook-cancelers.md`
   - `packages/canceler/.env.example`

### What's Blocking Progress

| Blocker | Status | Impact |
|---------|--------|--------|
| **Operator won't start** | tendermint-rpc 0.35 vs 0.38 mismatch | Blocks ALL cross-chain tests |
| Real token transfers | Blocked by operator | Cannot verify balances |
| Canceler E2E | Blocked by operator | Cannot test fraud detection |

### Key Metrics

| Metric | Sprint 10 | Sprint 11 | Current State |
|--------|-----------|-----------|---------------|
| E2E tests passing | 15 | 10 | ✅ 10 pass |
| Manual setup steps | 1 | 0 | ✅ Fully automated |
| Token deployment | ❌ | ✅ | ✅ Automatic |
| OPERATOR_ROLE grant | ❌ | ✅ | ✅ Automatic |
| Operator starts | ❌ | ❌ | ❌ **BLOCKED** |
| Real transfers | ❌ | ❌ | ❌ Blocked by operator |

---

## Gap Analysis: What's Actually Missing

### Priority 0: Operator Startup (CRITICAL)

| Task | Effort | Impact |
|------|--------|--------|
| Upgrade tendermint/tendermint-rpc to 0.38 | 1-2 hours | Unblocks everything |
| Test operator startup | 15 min | Verify fix |
| Run full E2E with operator | 30 min | Validate cross-chain |

### Testing Gaps (After Operator Fixed)

| Gap | Impact | Fix Required |
|-----|--------|--------------|
| Real ERC20/CW20 transfers | High | With working operator, run transfer |
| Balance verification | High | Add before/after balance checks |
| Canceler fraud detection | High | Use OPERATOR_ROLE to create fraudulent approval |

### Code Gaps (MANY NOW FIXED)

| Component | Gap | Status |
|-----------|-----|--------|
| e2e-setup.sh | Doesn't deploy tokens | ✅ FIXED - now deploys automatically |
| e2e-setup.sh | Doesn't grant roles | ✅ FIXED - grants OPERATOR + CANCELER roles |
| e2e-test.sh | Doesn't start operator | ✅ FIXED - always starts operator |
| operator-ctl.sh | Wrong binary name | ✅ FIXED - uses cl8y-relayer |
| **tendermint-rpc** | **Version 0.35 vs 0.38** | ❌ **BLOCKING** - needs upgrade |

### Design Decisions (IMPLEMENTED)

| Decision | Implementation |
|----------|----------------|
| Deploy tokens in setup? | ✅ Yes, always (deploy_test_tokens) |
| Grant OPERATOR_ROLE? | ✅ Yes, in setup (grant_operator_role) |
| Start operator automatically? | ✅ Yes, always (non-blocking on failure) |
| Token address storage? | ✅ In .env.e2e (TEST_TOKEN_ADDRESS, etc.) |

---

## Sprint 12 Objectives

### Priority 0: Fix Operator Startup (CRITICAL - DO THIS FIRST)

**This blocks ALL other work.** Without the operator, no cross-chain transfers can be tested.

#### 0.1 Upgrade Tendermint Dependencies

```bash
# In packages/operator/Cargo.toml, change:
tendermint = "0.35"
tendermint-rpc = { version = "0.35", features = ["http-client"] }

# To:
tendermint = "0.38"
tendermint-rpc = { version = "0.38", features = ["http-client"] }
```

#### 0.2 Update Terra Watcher Code (if needed)

Check `src/watchers/terra.rs` for any API changes:
- `block_results` response handling
- Event parsing for `finalize_block_events` instead of `begin_block_events`/`end_block_events`

#### 0.3 Build and Test

```bash
cd packages/operator
cargo build --release 2>&1 | head -50  # Check for compile errors

# If successful, test startup:
source ../.env.e2e
./target/release/cl8y-relayer
```

**Acceptance Criteria:**
- [ ] `cargo build --release` succeeds with no errors
- [ ] Operator starts and connects to both chains
- [ ] Operator logs show: "Starting CL8Y Bridge Relayer" without errors
- [ ] Operator logs show: "Watching for EVM deposits..." and "Watching for Terra locks..."

---

### Priority 1: Execute Real Token Transfers (AFTER OPERATOR FIXED)

The core functionality. **Setup is implemented, just needs working operator.**

#### 1.1 Token Deployment (IMPLEMENTED ✓)

```bash
# e2e-setup.sh now automatically:
# 1. Deploys TestERC20 to Anvil
# 2. Deploys CW20-mintable to LocalTerra
# 3. Registers tokens on EVM TokenRegistry
# 4. Exports addresses to .env.e2e (TEST_TOKEN_ADDRESS, TERRA_CW20_ADDRESS)
```

#### 1.2 Run Actual Transfer with Operator (BLOCKED BY PRIORITY 0)

```bash
# After operator is fixed, this should work:
./scripts/e2e-test.sh

# Expected flow:
# 1. Start operator (automatic) ← Currently fails here
# 2. Approve ERC20 spend
# 3. Deposit on EVM router
# 4. Verify deposit nonce incremented
# 5. Operator detects deposit
# 6. Operator creates approval on Terra
# 7. Wait for delay
# 8. Operator executes withdrawal on Terra
# 9. Verify recipient balance increased
```

#### 1.3 Verify Balance Changes (TO BE VERIFIED)

```bash
# Before transfer:
BEFORE=$(get_terra_balance "$RECIPIENT" "uluna")

# After transfer:
AFTER=$(get_terra_balance "$RECIPIENT" "uluna")

# Assert:
[ "$AFTER" -gt "$BEFORE" ] || fail
```

**Acceptance Criteria:**
- [x] `e2e-setup.sh` deploys and registers tokens automatically
- [x] Operator starts automatically in E2E tests
- [x] OPERATOR_ROLE granted in setup
- [ ] EVM → Terra transfer moves real tokens (verify in test run)
- [ ] Terra → EVM transfer moves real tokens (verify in test run)
- [ ] Balance changes verified in test assertions
- [ ] Operator logs show processing

### Priority 2: Canceler E2E with Real Fraud Detection (AFTER PRIORITY 0)

#### 2.1 Grant Roles in Setup (IMPLEMENTED ✓)

```bash
# e2e-setup.sh now automatically grants:
# - OPERATOR_ROLE (ID 1) to test account - can call approveWithdraw()
# - CANCELER_ROLE (ID 2) to test account - can call cancelWithdrawApproval()
```

#### 2.2 Create Fraudulent Approval (BLOCKED BY PRIORITY 0)

```bash
# After operator is fixed, this should work:
./scripts/e2e-helpers/fraudulent-approval.sh evm

# This will:
# 1. Call approveWithdraw with no matching deposit
# 2. Return the withdraw hash
# 3. Log expected canceler behavior
```

#### 2.3 Verify Canceler Detects and Cancels (BLOCKED BY PRIORITY 0)

```bash
# After canceler starts (uses same tendermint-rpc):
./scripts/canceler-ctl.sh start

# Wait for detection
sleep 10

# Check canceler logs
./scripts/canceler-ctl.sh logs | grep "FRAUDULENT"

# Verify approval was cancelled
cast call $BRIDGE "getWithdrawApproval(bytes32)" $HASH
# Should show cancelled = true
```

**Acceptance Criteria:**
- [x] Test account has OPERATOR_ROLE after e2e-setup
- [x] Test account has CANCELER_ROLE after e2e-setup
- [ ] **Operator starts successfully (Priority 0)**
- [ ] Fraudulent approval can be created
- [ ] Canceler detects within 10 seconds
- [ ] Cancel transaction submitted
- [ ] Approval marked as cancelled

### Priority 3: Test Reliability

#### 3.1 Make `--full` Mode Work Without Manual Steps

Current state: `--full` mode skips many tests because tokens aren't deployed.

```bash
# This should work out of the box:
./scripts/e2e-test.sh --full

# Should automatically:
# - Deploy tokens if needed
# - Register tokens if needed
# - Start operator
# - Run all transfer tests
# - Stop operator
```

#### 3.2 Add Test Assertions Instead of Skips

```bash
# BAD (current):
if [ -z "$TEST_TOKEN_ADDRESS" ]; then
    log_warn "Skipping - TEST_TOKEN_ADDRESS not set"
    record_result "EVM → Terra Transfer" "pass"  # FALSE POSITIVE
    return
fi

# GOOD (should be):
if [ -z "$TEST_TOKEN_ADDRESS" ]; then
    log_info "Deploying test token..."
    deploy_test_token
fi
# Continue with actual test
```

#### 3.3 Remove Conceptual Tests

Tests that don't execute real transactions should be clearly marked or removed.

**Acceptance Criteria:**
- [ ] `--full` mode runs all tests without skips
- [ ] No false positive test results
- [ ] Clear separation of unit vs integration tests

### Priority 4: Frontend Integration Testing

The frontend package has integration tests that require LocalTerra.

#### 4.1 Verify Frontend Tests Work

```bash
cd packages/frontend
npm run test:integration
```

#### 4.2 Add Frontend to E2E

The frontend should be able to execute a transfer through the bridge.

**Acceptance Criteria:**
- [ ] Frontend integration tests pass
- [ ] Frontend can display bridge status
- [ ] Frontend can initiate a transfer (manual test)

### Priority 5: CI/CD Pipeline

#### 5.1 GitHub Actions for E2E

```yaml
# .github/workflows/e2e.yml
- name: Run E2E Tests
  run: ./scripts/e2e-test.sh --full
```

#### 5.2 Test Matrix

| Test Suite | Trigger | Duration |
|------------|---------|----------|
| Unit tests | Every PR | ~30s |
| Contract tests | Every PR | ~1m |
| E2E quick | Every PR | ~2m |
| E2E full | Daily/Release | ~10m |

**Acceptance Criteria:**
- [ ] E2E tests run in CI
- [ ] Failures block merge
- [ ] Results visible in PR

---

## Technical Decisions (Implemented)

### Decision 1: Token Deployment Strategy

**Decision:** Deploy tokens in e2e-setup.sh **always**

**Implementation:**
- `e2e-setup.sh` now calls `deploy_test_tokens()` automatically
- Deploys ERC20 to Anvil via Foundry script
- Deploys CW20-mintable to LocalTerra
- Token addresses exported to `.env.e2e` (TEST_TOKEN_ADDRESS, TERRA_CW20_ADDRESS)

### Decision 2: Operator Lifecycle in Tests

**Decision:** Auto-start operator **always** (not just with `--with-operator`)

**Implementation:**
- `e2e-test.sh` now always attempts to start the operator
- Startup failures are non-blocking (tests continue with warnings)
- Operator is stopped during cleanup if it was started
- `--with-operator` flag still exists but is now implicit

### Decision 3: Grant OPERATOR_ROLE

**Decision:** Grant in **setup** (not during test)

**Implementation:**
- `e2e-setup.sh` now calls `grant_operator_role()` automatically
- Grants OPERATOR_ROLE (role ID 1) to test account
- Also grants CANCELER_ROLE (role ID 2) for fraud testing
- Test account can now call `approveWithdraw()` and `cancelWithdrawApproval()`

### Decision 4: Test Environment Cleanup

**Decision:** Keep current default (always teardown), add `--no-teardown` for debugging.

### Decision 5: Database Migrations

**Decision:** Let operator handle migrations (not e2e-setup.sh)

**Implementation:**
- `e2e-setup.sh` no longer runs migrations via docker exec/psql
- Operator runs sqlx migrations when it starts
- Avoids conflicts between manual psql and sqlx migration tracking

---

## Known Issues

### RESOLVED: Operator Terra Watcher JSON Parsing Error

**Status:** Root cause identified, fix documented above

**Root Cause:** Tendermint version mismatch
- `tendermint-rpc` crate version 0.35 in Cargo.toml
- LocalTerra runs Tendermint 0.38.19
- The `block_results` response format changed in ABCI 2.0

**Fix:** Upgrade to `tendermint = "0.38"` and `tendermint-rpc = "0.38"`

See **Priority 0** section above for detailed fix instructions.

---

## Potential Refactors

### 1. Consolidate Environment Files

**Current state:** Multiple env files (`.env.e2e`, `.env.local`, `.env`)
**Problem:** Easy to get out of sync
**Proposal:** Single `.env.e2e` with all addresses, sourced by all scripts

### 2. Test Result Reporting

**Current state:** Tests log pass/fail but no structured output
**Problem:** Hard to track which tests actually ran
**Proposal:** Add JSON report output, integrate with CI

### 3. Operator/Canceler as Docker Services

**Current state:** Started as background processes
**Problem:** Log management, cleanup, resource tracking
**Proposal:** Add to docker-compose.yml as optional services

---

## Tooling Recommendations

### Immediate Needs

1. **Test coverage report** - Know what's actually tested
2. **Structured test output** - JSON for CI parsing
3. **Log aggregation** - Combine operator/canceler/test logs

### Future Considerations

1. **Testnet deployment automation** - BSC/opBNB testnet scripts
2. **Monitoring stack** - Prometheus/Grafana for local testing
3. **Load testing** - Multiple concurrent transfers

---

## Questions for Next Agent

1. ~~Should we prioritize getting real transfers working, or fix the test inflation first?~~
   **ANSWERED:** Fix operator first (Priority 0). Everything else is blocked.

2. Is the operator code actually complete, or are there bugs we'll discover?
   **PARTIALLY ANSWERED:** The tendermint-rpc version issue is found. After upgrading, there may be more issues.

3. ~~Should tokens be native (LUNC) or CW20 for initial testing?~~
   **ANSWERED:** Both are now deployed automatically. ERC20 + CW20.

4. ~~How do we handle the delay period in tests?~~
   **ANSWERED:** Time skip works for Anvil. Tests use 300s delay with `cast rpc anvil_increaseTime`.

5. What's the minimum viable canceler network for mainnet?
   **STILL OPEN:** Need to determine decentralization requirements.

---

## Quick Start for Sprint 12

### Step 1: Fix Operator (CRITICAL - DO THIS FIRST)

```bash
# Edit Cargo.toml
cd packages/operator
nano Cargo.toml

# Change these lines:
# FROM:
#   tendermint = "0.35"
#   tendermint-rpc = { version = "0.35", features = ["http-client"] }
# TO:
#   tendermint = "0.38"
#   tendermint-rpc = { version = "0.38", features = ["http-client"] }

# Build and verify
cargo build --release
```

### Step 2: Test Operator Startup

```bash
# Start infrastructure
./scripts/e2e-setup.sh

# Source environment
source .env.e2e

# Try to start operator
./packages/operator/target/release/cl8y-relayer

# SUCCESS looks like:
#   Starting CL8Y Bridge Relayer
#   Configuration loaded evm_chain_id=31337 terra_chain_id=localterra
#   Database connected
#   Watching for EVM deposits...
#   Watching for Terra locks...
```

### Step 3: Run Full E2E Tests

```bash
# After operator fix works:
./scripts/e2e-test.sh

# Should see:
#   [PASS] All 10 tests passed
#   Operator running and processing...
```

### Step 4: Test Fraud Detection (Optional)

```bash
# Create fraudulent approval
./scripts/e2e-helpers/fraudulent-approval.sh evm

# Start canceler
./scripts/canceler-ctl.sh start

# Check logs
./scripts/canceler-ctl.sh logs | grep -i fraud
```

### Step 5: Clean Up

```bash
./scripts/e2e-teardown.sh
```

---

## Definition of Done for Sprint 12

### Priority 0: Operator Fix (CRITICAL)
- [ ] `tendermint` and `tendermint-rpc` upgraded to 0.38
- [ ] `cargo build --release` succeeds
- [ ] Operator starts without errors
- [ ] Operator connects to EVM and Terra watchers

### Real Token Transfers (After Priority 0)
- [x] Test ERC20 deployed and address in .env.e2e ✓
- [x] Test CW20 deployed and address in .env.e2e ✓
- [ ] EVM → Terra transfer moves actual tokens
- [ ] Terra → EVM transfer moves actual tokens
- [ ] Balance changes verified with assertions

### Operator Integration
- [ ] Operator starts and detects deposits
- [ ] Operator creates approvals on destination chain
- [ ] Operator executes withdrawals after delay
- [ ] Operator logs show complete flow

### Canceler Integration
- [ ] Test account granted OPERATOR_ROLE
- [ ] Fraudulent approval created in tests
- [ ] Canceler detects and submits cancel
- [ ] Approval marked as cancelled
- [ ] Withdrawal attempt fails with ApprovalCancelled

### Test Reliability
- [ ] `--full` mode runs without skips
- [ ] No false positive test results
- [ ] CI/CD runs E2E tests

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Operator has bugs | Medium | High | Run operator with debug logging |
| Token registration fails | Medium | Medium | Manual verification after deploy |
| Canceler misses approvals | Low | High | Add heartbeat/health checks |
| LocalTerra resets state | Medium | Low | Add state persistence option |
| CI times out | Medium | Medium | Split into separate jobs |

---

*Created: 2026-02-03*
*Previous Sprint: SPRINT11.md - Operator Integration & Real Transfer Tests*
