# Sprint 5: E2E Testing Infrastructure & Production Readiness

**Duration**: 2 weeks  
**Theme**: Automated Testing & Operational Excellence

---

## Sprint 4 Retrospective

### What Went Wrong

1. **E2E Testing Not Automated**
   - LocalTerra wasn't running when tests were attempted
   - Port 9090 already in use caused API server to fail
   - No proper setup/teardown scripts exist
   - `terrad` CLI not available in standard environment
   - Contract deployment not integrated into test flow
   - Tests require manual environment setup that wasn't documented

2. **WorkSplit Underutilization - Actual Statistics**
   
   | Status | Count | Jobs |
   |--------|-------|------|
   | **pass** | 2 | `sprint3_001_integration_tests`, `sprint4_007_terra_helpers` |
   | **fail** | 9 | Environment issues (rustup), thinking loops |
   | **created** (never run) | 9 | Most Sprint 4 frontend/API jobs |
   | **pending_work** | 4 | Jobs that generated partial output |
   
   **Root Causes:**
   - Environment issues: 8 jobs failed because "rustup could not choose a version of cargo"
   - Thinking loop: `relayer_012_writers_evm_full` failed with "Model stuck in thinking loop for 120s"
   - Jobs created but never run: Frontend jobs were created but manually implemented instead
   - Did not use `worksplit run` to batch execute - ran individual jobs or did manual work

3. **Incomplete Implementations**
   - `api.rs` - only stub created, full implementation not done
   - `multi_evm.rs` - only stub created
   - Confirmation tracking has placeholder logic
   - Frontend has no actual wallet integration working

### What Worked Well

- Frontend UI renders beautifully with all components
- EVM contracts deploy and all 59 tests pass
- Relayer builds and starts successfully
- Database migrations work correctly
- Unit tests for types pass

---

## Sprint 5 Priorities

### Priority 1: E2E Testing Infrastructure (CRITICAL)

The E2E testing infrastructure must be fully automated with clean setup/teardown.

#### 1.1 Docker Compose E2E Profile

Create a dedicated E2E profile that:
- Starts all required services (Anvil, LocalTerra, PostgreSQL)
- Waits for health checks before proceeding
- Uses isolated ports to avoid conflicts
- Supports clean teardown

**Target File**: `docker-compose.e2e.yml` or update `docker-compose.yml`

```yaml
# E2E services should use:
# - Dedicated ports (e.g., 18545 for Anvil, 19090 for API)
# - Health checks with proper wait conditions
# - Named volumes for isolation
# - Automatic cleanup on shutdown
```

#### 1.2 E2E Setup Script

Create `scripts/e2e-setup.sh`:
- Check/start Docker services with health wait
- Deploy EVM contracts to Anvil
- Deploy Terra contracts to LocalTerra
- Export all addresses to `.env.e2e`
- Verify all services are reachable
- Create test accounts with funded balances

#### 1.3 E2E Teardown Script

Create `scripts/e2e-teardown.sh`:
- Stop all E2E services gracefully
- Clean up test data
- Remove temporary files
- Report any orphaned processes

#### 1.4 E2E Test Runner

Update `scripts/e2e-test.sh`:
- Source setup script automatically
- Run tests in isolated environment
- Capture logs on failure
- Always run teardown (even on failure)
- Exit with proper status codes

#### 1.5 CI/CD Integration

Create `.github/workflows/e2e.yml`:
- Run on PR and push to main
- Use Docker Compose for services
- Cache Docker images
- Upload logs as artifacts on failure

---

### Priority 2: Complete Sprint 4 Gaps

#### 2.1 Full API Server Implementation

Replace the stub `src/api.rs` with full implementation:
- `/health` - Health check with component status
- `/metrics` - Prometheus metrics
- `/status` - Queue counts, uptime, chain sync status
- `/pending` - List of pending transactions with pagination
- `/tx/:hash` - Transaction status lookup

#### 2.2 Multi-EVM Configuration

Replace the stub `src/multi_evm.rs` with full implementation:
- Support multiple EVM chains in config
- Chain-specific RPC URLs and confirmation counts
- Dynamic chain addition/removal
- Per-chain rate limiting

#### 2.3 Confirmation Tracker Completion

The current implementation has placeholder logic. Complete:
- Actual RPC polling for EVM receipts
- Actual LCD polling for Terra transactions
- Proper reorg detection
- Confirmation count verification

---

### Priority 3: Frontend Functionality

#### 3.1 Wallet Connection

- Test MetaMask connection flow
- Handle chain switching
- Display connected address and balance
- Error states for wrong network

#### 3.2 Bridge Transaction Flow

- Form validation
- Transaction submission to contracts
- Loading states during transaction
- Success/failure feedback
- Transaction hash display with explorer links

#### 3.3 Transaction History

- Fetch from API `/pending` endpoint
- Real-time status updates
- Filter by status (pending, confirmed, failed)

---

### Priority 4: Operational Improvements

#### 4.1 Structured Logging

- Add request IDs for tracing
- Log levels per module
- JSON logging for production
- Log rotation configuration

#### 4.2 Error Handling

- Consistent error types across modules
- User-friendly error messages
- Retry logic with circuit breaker
- Dead letter queue for failed transactions

#### 4.3 Configuration Validation

- Validate all config on startup
- Fail fast with clear error messages
- Check RPC connectivity before starting
- Verify contract addresses are valid

---

## WorkSplit Best Practices for This Sprint

**READ THIS BEFORE CREATING ANY WORKSPLIT JOB**

### Mandatory Steps

1. **Read manager instructions first**: `jobs/_managerinstruction.md`
2. **Check README success rates**: The "Success Rate by Job Type" section
3. **Default to REPLACE mode** - Edit mode has ~50% success rate

### Job Sizing Rules

| Lines of Output | Action |
|-----------------|--------|
| < 50 lines | Do it manually, don't use WorkSplit |
| 50-150 lines | Single WorkSplit job |
| 150-300 lines | Consider splitting into 2 jobs |
| > 300 lines | MUST split into smaller jobs |

### Splitting Strategy

When a feature requires >300 lines:

```
# BAD - One large job
feature_001_complete.md  # 500 lines, will timeout

# GOOD - Split by responsibility
feature_001a_types.md      # ~80 lines: structs, enums
feature_001b_helpers.md    # ~100 lines: utility functions  
feature_001c_core.md       # ~120 lines: main logic
feature_001d_tests.md      # ~100 lines: unit tests
```

### Job Dependencies

Use `depends_on` in frontmatter:

```yaml
---
output_dir: src/api/
output_file: handlers.rs
depends_on:
  - api_001_types
  - api_002_db_queries
---
```

### Batch Execution

Create ALL related jobs FIRST, then run:

```bash
worksplit run  # Runs all pending jobs in dependency order
```

Do NOT run individual jobs with `--job` unless debugging.

### On Failure

1. **First failure**: Check if file was generated, may be incomplete
2. **Edit mode fails**: Switch to REPLACE mode immediately
3. **Timeout/thinking loop**: Break into smaller jobs
4. **Never retry edit mode** more than once
5. **Never abandon WorkSplit** without user approval - switch to manual if needed

### Recommended Job Breakdown for This Sprint

#### E2E Infrastructure Jobs (all REPLACE mode)

```
e2e_001_docker_compose.md     # ~80 lines: E2E Docker config
e2e_002_setup_script.md       # ~150 lines: Setup script
e2e_003_teardown_script.md    # ~60 lines: Teardown script
e2e_004_test_runner.md        # ~100 lines: Test runner updates
e2e_005_github_workflow.md    # ~80 lines: CI/CD workflow
```

#### API Server Jobs (all REPLACE mode)

```
api_001_types.md              # ~60 lines: Request/response types
api_002_handlers.md           # ~150 lines: Route handlers
api_003_server.md             # ~100 lines: Server setup
```

#### Multi-EVM Jobs (all REPLACE mode)

```
multi_evm_001_config.md       # ~80 lines: Config structs
multi_evm_002_manager.md      # ~120 lines: Chain manager
multi_evm_003_integration.md  # ~100 lines: Integration with existing code
```

---

## Definition of Done

### E2E Testing
- [ ] `./scripts/e2e-setup.sh` starts all services and deploys contracts
- [ ] `./scripts/e2e-test.sh` runs without manual intervention
- [ ] `./scripts/e2e-teardown.sh` cleans up completely
- [ ] Tests pass in fresh environment (no leftover state)
- [ ] Port conflicts are impossible (dedicated ports or dynamic allocation)
- [ ] CI/CD pipeline runs E2E tests on every PR

### API Server
- [ ] All 5 endpoints implemented and tested
- [ ] Swagger/OpenAPI documentation
- [ ] Integration tests for each endpoint

### Multi-EVM
- [ ] Config supports multiple chains
- [ ] Can add/remove chains without restart
- [ ] Per-chain metrics

### Frontend
- [ ] Wallet connects and shows balance
- [ ] Can submit bridge transaction
- [ ] Transaction appears in history

---

## Technical Debt to Address

1. **Unused imports warning** in `confirmation/mod.rs` - fix with `cargo fix`
2. **Placeholder confirmation logic** - replace with real RPC calls
3. **Hardcoded port 9090** - make configurable via env var
4. **Missing graceful shutdown** for API server
5. **No request timeout** on RPC calls

---

## Success Metrics

| Metric | Target |
|--------|--------|
| E2E test pass rate | 100% on clean environment |
| E2E setup time | < 60 seconds |
| E2E teardown time | < 10 seconds |
| API response time (p95) | < 100ms |
| WorkSplit job success rate | > 90% (by following best practices) |

---

## Notes for Next Agent

1. **Start with E2E infrastructure** - it's the foundation for everything else
2. **Test E2E setup/teardown in isolation** before running full tests
3. **Use dedicated ports** - don't reuse ports that might conflict
4. **Always use REPLACE mode** for WorkSplit jobs
5. **Split jobs < 200 lines** to avoid timeouts
6. **Run `worksplit status`** frequently to track progress
7. **If a job fails twice**, do it manually and move on
8. **Commit working code frequently** - don't let failures accumulate

---

## Appendix: E2E Test Architecture

### Port Allocation

| Service | Current Port | E2E Port | Notes |
|---------|--------------|----------|-------|
| Anvil | 8545 | 18545 | EVM RPC |
| PostgreSQL | 5433 | 15433 | Database |
| LocalTerra RPC | 26657 | 26657 | Keep default |
| LocalTerra LCD | 1317 | 1317 | Keep default |
| Relayer API | 9090 | 19090 | Health/metrics |
| Frontend | 3000 | N/A | Not needed for E2E |

### Docker Compose Profile Structure

```yaml
services:
  # Base services (always run)
  anvil:
    profiles: ["default", "e2e"]
    
  postgres:
    profiles: ["default", "e2e"]
    
  # E2E-only services
  localterra:
    profiles: ["e2e"]
    
  terrad-cli:
    profiles: ["e2e"]
    depends_on:
      localterra:
        condition: service_healthy
```

### E2E Script Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    e2e-setup.sh                             │
├─────────────────────────────────────────────────────────────┤
│ 1. Check prerequisites (docker, forge, terrad)             │
│ 2. Clean up any existing E2E containers                    │
│ 3. Start services: docker compose --profile e2e up -d      │
│ 4. Wait for health: until healthy do sleep 1               │
│ 5. Deploy EVM contracts: forge script DeployLocal          │
│ 6. Deploy Terra contracts: terrad tx wasm store...         │
│ 7. Configure bridge: register chains, tokens               │
│ 8. Export addresses to .env.e2e                            │
│ 9. Fund test accounts                                      │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    e2e-test.sh                              │
├─────────────────────────────────────────────────────────────┤
│ 1. Source .env.e2e                                         │
│ 2. Start relayer in background                             │
│ 3. Wait for relayer healthy                                │
│ 4. Run test scenarios:                                     │
│    - Terra → EVM transfer                                  │
│    - EVM → Terra transfer                                  │
│    - Error cases                                           │
│ 5. Verify final balances                                   │
│ 6. Report results                                          │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    e2e-teardown.sh                          │
├─────────────────────────────────────────────────────────────┤
│ 1. Stop relayer                                            │
│ 2. docker compose --profile e2e down -v                    │
│ 3. Remove .env.e2e                                         │
│ 4. Kill any orphaned processes                             │
└─────────────────────────────────────────────────────────────┘
```

### Test Scenarios

| Scenario | Steps | Verification |
|----------|-------|--------------|
| Terra → EVM | Lock LUNA on Terra | wLUNA minted on EVM |
| EVM → Terra | Deposit wLUNA on EVM | LUNA released on Terra |
| Retry on failure | Simulate RPC timeout | Transaction eventually succeeds |
| Confirmation tracking | Submit, wait for confirms | Status changes: pending → submitted → confirmed |

### Success Criteria

- [ ] `./scripts/e2e-setup.sh` completes in <60s
- [ ] `./scripts/e2e-test.sh` runs without manual intervention
- [ ] `./scripts/e2e-teardown.sh` leaves no orphaned containers/processes
- [ ] All tests pass on fresh `git clone` + `docker compose`
- [ ] CI/CD runs E2E on every PR
