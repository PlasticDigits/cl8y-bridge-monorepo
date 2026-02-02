# Sprint 8: Integration Validation & Production Hardening

**Previous Sprint:** [SPRINT7.md](./SPRINT7.md) - Testing, Polish & Production Readiness

---

## Sprint 7 Retrospective

### What Went Well

1. **Pure Unit Tests Work Great** - Testing `format.ts` and `constants.ts` with Vitest is fast and reliable. These are pure functions with no blockchain dependencies, so no infrastructure needed.

2. **Integration Test Structure** - The pattern of using `describe.skipIf()` and environment variables to conditionally run tests works well:
   ```typescript
   describe.skipIf(skipIntegration)('Infrastructure Connectivity', () => {
     // Tests that need real LocalTerra/Anvil
   })
   ```

3. **Bundle Splitting** - Code splitting reduced initial load from 6MB to 35KB (10KB gzip). Wallet chunks lazy-load only when user connects.

4. **Operator/Canceler Scripts** - The `*-ctl.sh` scripts with start/stop/status/logs provide good developer experience and enable E2E automation.

5. **Makefile Integration** - New `make test-frontend`, `make test-canceler`, `make operator-start` etc. commands integrate well.

### What Went Wrong

1. **Initial Mock-Based Approach Failed**
   - Started Sprint 7 by creating mock-based component tests
   - User explicitly rejected mocks for blockchain/wallet interactions
   - Had to delete `BridgeForm.test.tsx`, `wallet.test.ts` with mocks
   - Lesson: **This project tests against real infrastructure, not mocks**

2. **useBridgeDeposit Hook Untested**
   - The EVM deposit hook was implemented but not integration-tested
   - Waiting for transaction receipts has a hacky polling approach
   - Token addresses are placeholder values
   - Hook needs testing against deployed contracts

3. **Component Tests Missing**
   - After deleting mock-based tests, no component tests remain
   - BridgeForm rendering and interactions not tested
   - Could use React Testing Library for pure UI tests (no blockchain mocking)

4. **Terra Wallet Chunk Still Huge**
   - `wallet-terra` chunk is 5.3MB (604KB gzipped)
   - `@goblinhunt/cosmes` includes too much
   - May need to evaluate alternatives or tree-shaking

5. **E2E Tests Don't Execute Transfers**
   - Current E2E tests verify connectivity only
   - No actual Terra → EVM or EVM → Terra transfers in test suite
   - `--full` mode stubs out transfer logic with TODOs

### Key Discovery: No Mocks Philosophy

The project has a strict no-mocks policy for blockchain interactions:

```
DO test with mocks:
- Pure functions (format, parse, hash)
- Configuration validation
- React component rendering/props

DO NOT mock:
- RPC/LCD responses
- Wallet signing
- Contract calls
- Event polling
- Transaction submission
```

This is documented in README.md under "Testing Philosophy: No Mocks for Blockchain".

---

## Gaps Remaining

### HIGH Priority

| Gap | Impact | Sprint 7 Status |
|-----|--------|-----------------|
| **useBridgeDeposit untested** | EVM deposits may fail in production | Hook exists but no integration test |
| **No transfer E2E tests** | Can't verify cross-chain actually works | Scripts prepared but transfers not executed |
| **Token config is placeholder** | EVM deposits won't work without real addresses | Hardcoded `0x000...` addresses |
| **BridgeForm EVM flow untested** | UI may break when user attempts EVM deposit | No component or integration tests |

### MEDIUM Priority

| Gap | Impact | Sprint 7 Status |
|-----|--------|-----------------|
| **Transaction receipt waiting** | Deposits may timeout or not detect completion | Hacky polling in hook |
| **No real-time status updates** | User doesn't see transfer progress | Not implemented |
| **No transaction history** | User can't track past transfers | UI exists but no persistence |
| **Terra chunk too large** | Slow load on mobile/weak connections | 604KB gzipped |

### LOW Priority

| Gap | Impact | Sprint 7 Status |
|-----|--------|-----------------|
| **No Playwright E2E** | Full browser automation not available | Not started |
| **Component test coverage low** | UI regressions not caught | Only unit tests |
| **Canceler doesn't submit cancels** | Watchtower is observe-only | Logs but doesn't cancel |

---

## Sprint 8 Objectives

### Priority 1: Integration Test EVM Deposit Flow (CRITICAL)

The `useBridgeDeposit` hook needs testing against real deployed contracts.

**Tasks:**

1. **Deploy test token on Anvil**
   ```bash
   # Deploy a test ERC20 with known address
   forge script script/DeployTestToken.s.sol --broadcast
   ```

2. **Create integration test for deposit hook**
   ```typescript
   // packages/frontend/src/hooks/useBridgeDeposit.integration.test.ts
   // Test against real Anvil with deployed contracts
   ```

3. **Test approve → deposit flow end-to-end**
   - Check allowance before
   - Execute approve
   - Verify allowance after
   - Execute deposit
   - Verify DepositRequest event emitted

4. **Update token config with real addresses**
   - Set via environment variables
   - Document in .env.example

**Acceptance Criteria:**
- [x] EVM deposit integration test passes on devnet
- [x] Token addresses configurable via env
- [x] Approve + deposit works in UI

### Priority 2: E2E Transfer Execution

Complete the E2E test suite to actually execute transfers.

**Tasks:**

1. **Implement `test_terra_to_evm_transfer()` fully**
   - Execute lock on Terra via terrad CLI
   - Wait for operator to detect and approve
   - Time-skip on Anvil
   - Verify withdrawal executed

2. **Implement `test_evm_to_terra_transfer()` fully**
   - Execute deposit via cast
   - Wait for operator to detect and approve on Terra
   - Verify release executed

3. **Add transfer verification helpers**
   ```bash
   # scripts/e2e-helpers/wait-for-event.sh
   wait_for_evm_event "DepositRequest" "$EVM_BRIDGE"
   wait_for_terra_event "lock" "$TERRA_BRIDGE"
   ```

**Acceptance Criteria:**
- [x] `./scripts/e2e-test.sh --with-operator --full` executes real transfers
- [x] Both directions (Terra→EVM, EVM→Terra) tested
- [x] Tests complete in under 5 minutes

### Priority 3: Component Tests Without Mocks

Add component tests that test UI without mocking blockchain.

**Tasks:**

1. **Test BridgeForm rendering**
   - Test form elements render
   - Test input validation
   - Test chain swap button
   - No wallet/blockchain mocking

2. **Test with disconnected state**
   - Verify "Connect Wallet" button appears
   - Verify disabled states

3. **Test loading/error UI states**
   - Use React Testing Library
   - Don't mock blockchain responses

**Acceptance Criteria:**
- [x] 24 component tests for BridgeForm
- [x] Tests don't mock RPC/LCD/wallets (only mock wallet connection state)
- [x] Tests run without infrastructure

### Priority 4: Transaction Receipt Handling

Fix the polling approach for waiting on transaction receipts.

**Tasks:**

1. **Use wagmi's `useWaitForTransactionReceipt` properly**
   ```typescript
   const { isLoading, isSuccess } = useWaitForTransactionReceipt({
     hash: txHash,
   })
   ```

2. **Add timeout handling**
   - Configurable timeout (default 2 minutes)
   - Clear error message on timeout

3. **Add retry logic**
   - Retry failed transactions with user confirmation
   - Don't retry user rejections

**Acceptance Criteria:**
- [x] Deposit hook uses proper receipt waiting
- [x] Timeout shows clear error (2 minute timeout)
- [x] Retry available for failed txs

### Priority 5: Bundle Size Reduction

Reduce the Terra wallet chunk size.

**Tasks:**

1. **Analyze what's in cosmes**
   ```bash
   npx vite-bundle-analyzer
   ```

2. **Evaluate alternatives**
   - Could use cosmos-kit instead?
   - Tree-shake unused wallet types?

3. **Lazy load wallet connectors**
   ```typescript
   const StationConnector = lazy(() => import('./connectors/station'))
   ```

**Acceptance Criteria:**
- [x] Understand what's causing 5.3MB chunk (cosmes protobufs: 57MB source)
- [x] Reduce to under 2MB if possible (not feasible - all Cosmos protos bundled together)
- [x] Document findings (see `packages/frontend/BUNDLE_ANALYSIS.md`)

---

## Technical Notes for Next Agent

### Frontend Test Setup

Tests are in `packages/frontend/src/**/*.test.ts`. Run with:
```bash
cd packages/frontend
npm run test:unit      # Skip integration tests
npm run test:run       # All tests (needs devnet)
npm run test:integration # Integration only
```

The test setup file is `src/test/setup.ts`. It does NOT mock fetch or wallet libraries.

### useBridgeDeposit Hook

Located at `src/hooks/useBridgeDeposit.ts`. Key functions:

- `computeTerraChainKey(chainId)` - Computes keccak256 chain key
- `encodeTerraAddress(terraAddress)` - Encodes bech32 to bytes32
- `deposit(amount, destChainId, destAddress, decimals)` - Main entry point

The hook requires `tokenAddress` and `lockUnlockAddress` to be configured.

### Operator/Canceler Scripts

```bash
./scripts/operator-ctl.sh {start|stop|status|logs}
./scripts/canceler-ctl.sh {start|stop|status|logs} [instance_id]
```

PID files: `.operator.pid`, `.canceler-N.pid`
Log files: `.operator.log`, `.canceler-N.log`

### E2E Test Flags

```bash
./scripts/e2e-test.sh --quick          # Connectivity only
./scripts/e2e-test.sh --full           # Include transfer tests
./scripts/e2e-test.sh --with-operator  # Auto-start operator
./scripts/e2e-test.sh --with-canceler  # Auto-start canceler
./scripts/e2e-test.sh --with-all       # Both
./scripts/e2e-test.sh --skip-terra     # Skip Terra tests
```

### Known Issues

1. **Cosmes removeNull()** - The cosmes library strips null values after signing which can cause signature mismatches. See `services/wallet.ts` comments.

2. **LocalTerra Gas** - Terra Classic LCD doesn't support simulation endpoint, so we use fixed gas limits.

3. **Token Addresses Placeholder** - `VITE_BRIDGE_TOKEN_ADDRESS` and `VITE_LOCK_UNLOCK_ADDRESS` are not set. Need real deployed token.

---

## Definition of Done

### Sprint 8 Completion Status: ✅ COMPLETE

1. **Integration Tests**
   - [x] EVM deposit flow tested on devnet
   - [x] useBridgeDeposit hook passes integration tests
   - [x] Token configuration documented (`.env.example`, `make deploy-test-token`)

2. **E2E Transfers**
   - [x] Terra → EVM transfer executes in E2E (implemented in e2e-test.sh)
   - [x] EVM → Terra transfer executes in E2E (implemented in e2e-test.sh)
   - [x] Both complete in automated test (`make e2e-test-full`)

3. **UI Quality**
   - [x] 24 component tests for BridgeForm
   - [x] Transaction receipt handling improved (timeout, retry support)
   - [x] Loading states work correctly

4. **Performance**
   - [x] Bundle analysis documented (`BUNDLE_ANALYSIS.md`)
   - [x] Size reduction evaluated (not feasible - documented why)
   - [x] Initial load under 150KB gzipped (47KB achieved)

---

## Quick Start for Next Agent

```bash
# Start infrastructure (uses official classic-terra/localterra-core:0.5.18 image)
docker compose up -d anvil localterra postgres

# Deploy contracts
make deploy

# Run all tests
make test

# Start frontend dev
cd packages/frontend && npm run dev

# Run E2E
./scripts/e2e-test.sh --with-all --full
```

---

*Created: 2026-02-02*
*Previous Sprint: SPRINT7.md - Testing, Polish & Production Readiness*
