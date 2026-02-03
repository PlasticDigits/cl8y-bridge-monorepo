# Sprint 13: Security Hardening & Production Readiness

**Previous Sprint:** [SPRINT12.md](./SPRINT12.md) - Operator Fix & Production Readiness

---

## 🚨 CRITICAL: Pre-commit E2E Tests Blocking - Fix Before Any Other Work

### Context

The pre-commit hook now runs FULL E2E tests on every commit (no flags, no skipping). This is by design - security-critical code should not be committed without full validation. However, the E2E tests are currently failing, which blocks all commits.

**Current State:**
- Pre-commit runs: formatting ✅, clippy ✅, gitleaks ✅, E2E tests ❌
- E2E infrastructure starts correctly (Anvil, LocalTerra, PostgreSQL)
- EVM contract deployment works
- Terra contract deployment works
- Basic connectivity tests pass

### Failing Tests (must fix to unblock commits)

#### 1. Terra Bridge Configuration Query Fails
```
[FAIL] TEST: Terra Bridge Configuration
[WARN] Could not query Terra bridge config
```
- The `config {}` query to the Terra bridge contract returns empty
- Need to verify the contract was instantiated correctly
- May be a CosmWasm query format issue

#### 2. EVM → Terra Deposit Transaction Reverts
```
[INFO] Step 3: Executing deposit on router...
[ERROR] Deposit transaction failed
```
- Token approval succeeds
- Deposit call to router reverts
- Likely causes:
  - Token not registered on bridge
  - Chain not configured
  - LockUnlock adapter not set up correctly

### Files Changed (Uncommitted)

These changes are staged but can't be committed until E2E passes:

| File | Change |
|------|--------|
| `.githooks/pre-commit` | Runs FULL E2E tests with `make e2e-test` (no flags) |
| `scripts/e2e-test.sh` | Fixed SCRIPTS_DIR path issue for operator-ctl.sh/canceler-ctl.sh |
| `scripts/e2e-helpers/common.sh` | Fixed balance parsing (strip `[1e12]` formatting from cast output) |

### How to Debug

```bash
# Start infrastructure manually
./scripts/e2e-setup.sh

# Source environment
source .env.e2e

# Test Terra bridge config query
docker exec -it cl8y-bridge-monorepo-terrad-cli-1 terrad query wasm contract-state smart \
  $TERRA_BRIDGE_ADDRESS '{"config":{}}' --node http://localterra:26657

# Test EVM deposit manually
cast send $EVM_ROUTER_ADDRESS "deposit(address,uint256,bytes32,bytes32)" \
  $TEST_TOKEN 1000000 $TERRA_CHAIN_KEY $TERRA_DEST_ACCOUNT \
  --rpc-url $EVM_RPC_URL --private-key $EVM_PRIVATE_KEY

# Check if token is registered
cast call $TOKEN_REGISTRY_ADDRESS "isTokenSupported(address)" $TEST_TOKEN \
  --rpc-url $EVM_RPC_URL

# Teardown when done
./scripts/e2e-teardown.sh
```

### Priority

**This is the #1 priority.** All other Sprint 13 work is blocked until E2E tests pass.

The security enforcement is working correctly - it's blocking commits because tests fail. Fix the underlying issues, don't bypass the checks.

---

## Sprint 12 Retrospective

### What Went Right

1. **E2E Canceler Integration Tests Implemented:**
   - `test_canceler_fraudulent_detection()` now creates actual fraudulent approvals
   - `test_canceler_cancel_flow()` submits real cancel transactions
   - `test_canceler_withdrawal_fails()` verifies ApprovalCancelled revert
   - Tests use environment variables for fraud details between test stages

2. **CI/CD Pipeline Established:**
   - `.github/workflows/e2e.yml` runs full E2E suite on PRs and pushes to main
   - `.github/workflows/test.yml` runs unit tests for all packages (EVM, Operator, Canceler, Terra)
   - Workflow supports manual trigger with Terra tests option
   - Artifacts uploaded on failure for debugging

3. **Security-First Test Design:**
   - All security tests run by default (no opt-in required)
   - `--no-*` flags show explicit warnings about reduced coverage
   - Operator and canceler started automatically in E2E

4. **Operator Working:**
   - Tendermint/cosmrs version compatibility fixed
   - Operator connects to both EVM and Terra
   - Block processing working on both chains

### What Went Wrong

1. **E2E Tests Are Still "Best Effort":**
   - Tests pass even when OPERATOR_ROLE or CANCELER_ROLE not granted
   - The test verifies the *mechanism* works but doesn't verify *actual* canceler daemon
   - Manual `cast send` calls replace automated canceler detection

2. **CI Workflow May Have Issues:**
   - `dtolnay/rust-action@stable` might not be the correct action name
   - Should verify workflow runs successfully before merge

3. **Canceler Not Actually Started in E2E:**
   - The E2E tests manually submit cancel transactions
   - The canceler daemon itself is not started and verified to detect/cancel autonomously
   - This means we're testing the *contract* mechanism, not the *canceler service*

4. **No Integration With Running Canceler:**
   - `./scripts/canceler-ctl.sh start` is mentioned but never called in E2E
   - Canceler logs not verified for fraud detection messages

---

## Security Gap Analysis

### Critical Security Gaps (Bridges Are High-Value Targets)

| Gap | Severity | Risk | Mitigation Required |
|-----|----------|------|---------------------|
| **Canceler not tested as running service** | 🔴 Critical | Canceler may have bugs that aren't caught | Add E2E test that starts canceler and verifies detection |
| **Single canceler instance** | 🔴 Critical | SPOF for fraud detection | Deploy multiple independent cancelers |
| **No canceler health monitoring** | 🔴 Critical | Canceler failure goes unnoticed | Add /health endpoint and monitoring |
| **Terra VerifyDeposit query not tested** | 🟡 High | Terra→EVM cancellation may not work | Add Terra source verification test |
| **No rate limiting on approvals** | 🟡 High | Operator could spam approvals | Add per-block approval limits |
| **Private keys in env vars** | 🟡 High | Key exposure in logs/env | Use KMS/HSM for production |

### Testing Gaps

| Gap | Type | Impact |
|-----|------|--------|
| **Canceler daemon E2E** | Integration | Canceler service bugs undetected |
| **Multi-chain verification** | Integration | BSC/opBNB specific issues missed |
| **Reeneable flow** | Functional | Admin recovery path untested |
| **Rate limiting guards** | Security | TokenRateLimit bypass possible |
| **RPC failure handling** | Resilience | Network outages could cause missed cancellations |
| **Concurrent approvals** | Load | Race conditions untested |
| **Hash computation fuzzing** | Security | Edge cases in hash computation |
| **Frontend E2E** | Integration | User-facing flow untested |

### Operational Gaps

| Gap | Risk | Priority |
|-----|------|----------|
| **No canceler health endpoint** | Undetected failures | 🔴 Critical |
| **No time-to-cancel metrics** | Unknown detection latency | 🟡 High |
| **No testnet deployment** | Production issues discovered too late | 🟡 High |
| **Canceler failure runbook missing** | Slow incident response | 🟡 High |
| **No backup canceler nodes** | Single point of failure | 🔴 Critical |

---

## Sprint 13 Objectives

### Priority 0: Verify CI/CD Works

Before proceeding, verify the CI pipeline actually runs.

```bash
# 1. Create a test branch
git checkout -b test/verify-ci

# 2. Push and check GitHub Actions
git push origin test/verify-ci

# 3. Monitor workflow runs
# - e2e.yml should trigger
# - test.yml should trigger
# - Both should pass

# 4. Fix any issues (likely rust-action name)
```

**Acceptance Criteria:**
- [ ] CI workflow triggers on push
- [ ] All jobs pass (contracts-evm, operator, canceler, contracts-terra, e2e-evm)
- [ ] Failures block merge (branch protection configured)

### Priority 1: Real Canceler E2E Test

The current E2E tests verify cancel *works*, but don't verify the *canceler service* detects fraud.

#### 1.1 Add Canceler Daemon E2E Test

```bash
# New test function: test_canceler_autonomous_detection()
# 1. Start canceler daemon (./scripts/canceler-ctl.sh start)
# 2. Create fraudulent approval
# 3. Wait for canceler to detect (poll logs)
# 4. Verify cancel transaction submitted automatically
# 5. Verify approval marked as cancelled
# 6. Stop canceler
```

#### 1.2 Add Canceler Health Check

The canceler should expose a `/health` endpoint for monitoring.

```rust
// packages/canceler/src/api.rs (new file)
async fn health_check() -> impl Responder {
    let stats = watcher.get_stats();
    HttpResponse::Ok().json({
        "status": "healthy",
        "verified_count": stats.verified,
        "cancelled_count": stats.cancelled,
        "last_block_evm": stats.last_evm_block,
        "last_block_terra": stats.last_terra_block,
    })
}
```

**Acceptance Criteria:**
- [ ] Canceler daemon starts and is monitored in E2E
- [ ] E2E test creates fraud, canceler detects within 30s
- [ ] E2E test verifies cancel tx submitted by canceler (not manual)
- [ ] Canceler exposes /health endpoint

### Priority 2: Multi-Canceler Network

For production security, need multiple independent cancelers.

#### 2.1 Document Multi-Canceler Setup

```markdown
# Minimum Viable Canceler Network

| Environment | Cancelers Required | Locations |
|-------------|-------------------|-----------|
| Local/Dev   | 1                 | Same machine |
| Testnet     | 2                 | Different regions |
| Mainnet     | 3+                | Different operators |

## Independence Requirements

Each canceler should have:
- Independent RPC endpoints (not same provider)
- Independent hosting (not same cloud region)
- Independent operator (not same organization)
```

#### 2.2 Add Canceler Instance ID

The canceler should identify itself in logs for multi-instance debugging.

```rust
// packages/canceler/src/config.rs
pub canceler_id: String, // e.g., "canceler-us-east-1"
```

**Acceptance Criteria:**
- [ ] Canceler has configurable instance ID
- [ ] Multiple cancelers can run simultaneously
- [ ] Documentation for multi-canceler deployment

### Priority 3: Resilience Testing

Test failure scenarios that could allow fraud.

#### 3.1 RPC Failure Test

```bash
# Test: What happens when Terra LCD is down during verification?
# Expected: Approval stays Pending, verified on retry
# Not: Approval passes as Valid
```

#### 3.2 Delayed Detection Test

```bash
# Test: Create approval, delay canceler start by N seconds
# Expected: Canceler still cancels within remaining delay window
# Risk: If delay < detection time, fraud succeeds
```

#### 3.3 Concurrent Approval Test

```bash
# Test: Create 10 fraudulent approvals rapidly
# Expected: All 10 cancelled
# Risk: Race conditions, gas exhaustion
```

**Acceptance Criteria:**
- [ ] RPC failure doesn't cause false Valid
- [ ] Delayed start still cancels if within window
- [ ] Multiple approvals all cancelled

### Priority 4: Production Preparation

#### 4.1 Testnet Deployment

Deploy to BSC testnet for real-world validation.

```bash
# 1. Deploy EVM contracts to BSC testnet
./scripts/deploy-evm-testnet.sh bsc

# 2. Deploy Terra contracts to pisco testnet
./scripts/deploy-terra-testnet.sh

# 3. Run operator against testnet
# 4. Run canceler against testnet
# 5. Execute test transfer
```

#### 4.2 Monitoring Stack

Add Prometheus metrics for:
- `canceler_approvals_verified_total` (counter)
- `canceler_approvals_cancelled_total` (counter)
- `canceler_verification_latency_seconds` (histogram)
- `canceler_last_block_processed` (gauge per chain)

#### 4.3 Alerting Rules

```yaml
# Example Prometheus alert
- alert: CancelerNotProcessingBlocks
  expr: rate(canceler_last_block_processed[5m]) == 0
  for: 5m
  labels:
    severity: critical
  annotations:
    summary: "Canceler not processing blocks"
```

**Acceptance Criteria:**
- [ ] Testnet deployment working
- [ ] Prometheus metrics exposed
- [ ] Alert rules defined for critical failures

---

## Technical Debt

### Code Quality Issues

| Issue | Location | Priority |
|-------|----------|----------|
| Unused code warnings in operator | `cargo build` | Low |
| Unused code warnings in canceler | `cargo build` | Low |
| TODO comments in test files | Various | Low |
| Missing error handling in some paths | `verifier.rs` | Medium |

### Architecture Improvements

| Improvement | Benefit | Effort |
|-------------|---------|--------|
| Operator as Docker container | Better lifecycle management | Medium |
| Canceler as Docker container | Better lifecycle management | Medium |
| Shared Rust library for hash computation | Single source of truth | High |
| GraphQL API for bridge status | Better frontend integration | High |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| CI workflow doesn't work | High | Medium | Verify before merge |
| Canceler has undiscovered bugs | Medium | Critical | More E2E coverage |
| RPC outages cause missed cancellation | Medium | Critical | Multi-provider, retries |
| Single canceler compromised | Low | Critical | Multi-canceler network |
| Testnet differs from mainnet | Low | Medium | Use mainnet forks |
| Gas price spikes block cancellation | Medium | High | Gas buffering, priority |

---

## Questions for Next Agent

1. **Should the CI workflow use the correct action name?**
   The `dtolnay/rust-action@stable` may not exist. Check if it should be `dtolnay/rust-toolchain@stable` or `actions-rust-lang/setup-rust-toolchain`.

2. **What's the minimum viable canceler network for mainnet launch?**
   Currently single canceler. Need to define decentralization requirements.

3. **Should canceler run as Docker service in docker-compose?**
   Currently runs as background process. Docker would provide better lifecycle management.

4. **How should canceler keys be managed in production?**
   Currently env vars. Need HSM/KMS for real security.

5. **What's the acceptable detection latency?**
   Current target is "within delay window" (~60s local, ~300s prod). Need SLA definition.

---

## Quick Start for Sprint 13

### Step 1: Verify CI Works

```bash
# Push a small change and watch GitHub Actions
git push origin main

# Check: https://github.com/[repo]/actions
# Both e2e.yml and test.yml should run
```

### Step 2: Fix CI If Needed

```yaml
# If dtolnay/rust-action doesn't exist, change to:
- uses: dtolnay/rust-toolchain@stable
  with:
    toolchain: stable
    components: rustfmt, clippy
```

### Step 3: Add Canceler Daemon E2E

```bash
# Update e2e-test.sh to include:
test_canceler_autonomous_detection() {
    log_step "=== TEST: Canceler Autonomous Detection ==="
    
    # Start canceler
    ./scripts/canceler-ctl.sh start 1
    sleep 5
    
    # Create fraudulent approval
    # ... (already exists in test_canceler_fraudulent_detection)
    
    # Wait for canceler to detect (poll logs)
    for i in {1..30}; do
        if grep -qi "cancellation submitted" .canceler.1.log; then
            log_pass "Canceler detected and cancelled fraud"
            break
        fi
        sleep 2
    done
    
    # Verify approval cancelled
    # ...
    
    # Stop canceler
    ./scripts/canceler-ctl.sh stop 1
}
```

### Step 4: Add Canceler Health Endpoint

```rust
// packages/canceler/src/main.rs
// Add axum server for health endpoint
```

---

## Definition of Done for Sprint 13

### CI/CD Verification
- [ ] e2e.yml workflow runs and passes
- [ ] test.yml workflow runs and passes
- [ ] Branch protection requires passing CI

### Canceler E2E
- [ ] Canceler daemon started in E2E tests
- [ ] Fraudulent approval detected by daemon (not manual cancel)
- [ ] Canceler /health endpoint implemented
- [ ] Health endpoint tested in E2E

### Resilience
- [ ] RPC failure doesn't cause false Valid
- [ ] Multiple concurrent approvals all cancelled
- [ ] Delayed canceler start still works

### Production Prep
- [ ] Testnet deployment documented
- [ ] Monitoring metrics implemented
- [ ] Alert rules defined

---

## Appendix: File Changes in Sprint 12

### Modified Files

| File | Changes |
|------|---------|
| `scripts/e2e-test.sh` | +302 lines - Added real canceler E2E tests |
| `.github/workflows/e2e.yml` | Rewrote - Full E2E pipeline with operator/canceler |
| `.github/workflows/test.yml` | +158 lines - Added Rust package tests |
| `SPRINT12.md` | Updated completion markers |

### Files Created

None (all modifications to existing files)

### Files That Should Be Created in Sprint 13

| File | Purpose |
|------|---------|
| `packages/canceler/src/api.rs` | Health endpoint |
| `docs/runbook-canceler-operations.md` | Ops runbook |
| `monitoring/alerts.yml` | Prometheus alert rules |
| `docker-compose.canceler.yml` | Multi-canceler local setup |

---

*Created: 2026-02-03*
*Previous Sprint: SPRINT12.md - Operator Fix & Production Readiness*
