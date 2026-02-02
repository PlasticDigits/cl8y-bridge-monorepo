# Sprint 7: Testing, Polish & Production Readiness

**Previous Sprint:** [SPRINT6.md](./SPRINT6.md) - Frontend & Devnet Validation

---

## Sprint 6 Retrospective

### What Went Right

1. **Terra Wallet Integration** - Successfully integrated `@goblinhunt/cosmes` from the ustr-cmm reference project. Multi-wallet support (Station, Keplr, Leap, etc.) works correctly.

2. **Code Organization** - Clean separation of concerns with:
   - `services/wallet.ts` - Core wallet functions
   - `stores/wallet.ts` - Zustand state management
   - `hooks/useWallet.ts` - React hook for components

3. **Canceler Event Polling** - Implemented actual event polling for both chains:
   - EVM: `WithdrawApproved` events via alloy
   - Terra: `pending_approvals` query via LCD

4. **Operator Tests** - 30 unit tests + 5 integration tests passing

5. **Build Success** - Both frontend and canceler compile successfully

### What Went Wrong

1. **Vitest Not Set Up** - Frontend has no test infrastructure at all:
   - No vitest dependency
   - No test script in package.json
   - No test files exist

2. **Bundle Size** - Frontend bundle is 5.8MB due to wallet dependencies:
   - Needs code splitting
   - Consider lazy loading wallet connectors

3. **Canceler Has No Tests** - The new event polling code has zero test coverage

4. **Unused Code Warnings** - Several unused fields in canceler (approved_at_timestamp, delay_seconds, evm_rpc_url)

### Gaps Remaining

| Area | Gap | Priority |
|------|-----|----------|
| **Testing** | No frontend tests (vitest) | HIGH |
| **Testing** | No canceler tests | HIGH |
| **Testing** | E2E requires manual operator startup | MEDIUM |
| **Frontend** | EVM deposit not implemented in UI | HIGH |
| **Frontend** | No transaction history persistence | MEDIUM |
| **Frontend** | No real-time status updates | MEDIUM |
| **Frontend** | Large bundle size (5.8MB) | LOW |
| **Canceler** | Unused struct fields | LOW |
| **Documentation** | Missing API docs for frontend hooks | LOW |

---

## Sprint 7 Objectives

### Priority 1: Frontend Testing (CRITICAL)

Set up Vitest and create comprehensive tests.

**Tasks:**

1. **Set up Vitest infrastructure**
   ```bash
   cd packages/frontend
   npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom
   ```

2. **Add test scripts to package.json**
   ```json
   {
     "scripts": {
       "test": "vitest",
       "test:ui": "vitest --ui",
       "test:coverage": "vitest --coverage"
     }
   }
   ```

3. **Create vitest.config.ts**
   ```typescript
   import { defineConfig } from 'vitest/config'
   import react from '@vitejs/plugin-react'

   export default defineConfig({
     plugins: [react()],
     test: {
       environment: 'jsdom',
       globals: true,
       setupFiles: './src/test/setup.ts',
     },
   })
   ```

4. **Write tests for:**
   - `utils/format.ts` - Unit tests for formatting functions
   - `utils/constants.ts` - Config validation
   - `stores/wallet.ts` - Zustand store tests
   - `hooks/useWallet.ts` - Hook tests with mocked wallet
   - `components/BridgeForm.tsx` - Component tests

**Acceptance Criteria:**
- [ ] `npm run test` works
- [ ] At least 80% coverage on utils
- [ ] At least 50% coverage on stores/hooks
- [ ] Component tests for BridgeForm

### Priority 2: EVM Deposit Implementation

Complete the EVM → Terra transfer in frontend.

**Tasks:**

1. **Create useBridgeDeposit hook**
   ```typescript
   // hooks/useBridgeDeposit.ts
   import { useWriteContract, useWaitForTransactionReceipt } from 'wagmi'
   
   export function useBridgeDeposit() {
     // Implement EVM deposit via router contract
   }
   ```

2. **Update BridgeForm to use the hook**

3. **Add proper ABI for EVM Router contract**

**Acceptance Criteria:**
- [ ] Can submit EVM deposit from UI
- [ ] Transaction status updates in UI
- [ ] Error handling for failed transactions

### Priority 3: Canceler Tests

Add unit tests for canceler event polling.

**Tasks:**

1. **Create mock providers for testing**
   ```rust
   // src/test_utils.rs
   pub fn mock_evm_provider() -> MockProvider { ... }
   pub fn mock_terra_lcd() -> MockLcd { ... }
   ```

2. **Test event polling logic**
   - Test block range queries
   - Test approval parsing
   - Test deduplication

3. **Test verification logic**
   - Test hash computation
   - Test source chain verification

**Acceptance Criteria:**
- [ ] At least 10 unit tests for canceler
- [ ] Event polling tested with mocks
- [ ] Verification logic tested

### Priority 4: Bundle Optimization

Reduce frontend bundle size.

**Tasks:**

1. **Add code splitting for wallet connectors**
   ```typescript
   // Lazy load wallet components
   const WalletModal = lazy(() => import('./WalletModal'))
   ```

2. **Configure Vite manual chunks**
   ```typescript
   // vite.config.ts
   build: {
     rollupOptions: {
       output: {
         manualChunks: {
           'wallet-terra': ['@goblinhunt/cosmes'],
           'wallet-evm': ['wagmi', 'viem'],
           'vendor': ['react', 'react-dom'],
         }
       }
     }
   }
   ```

3. **Analyze bundle with `vite-bundle-analyzer`**

**Acceptance Criteria:**
- [ ] Main bundle under 500KB
- [ ] Wallet code in separate chunks
- [ ] Total gzipped size under 500KB

### Priority 5: E2E Automation

Make E2E tests fully automated.

**Tasks:**

1. **Create operator start/stop helpers in E2E script**
   ```bash
   start_operator_background() {
     cd packages/operator
     cargo run &
     OPERATOR_PID=$!
   }
   ```

2. **Add wait-for-event helpers**
   - Wait for DepositRequest on EVM
   - Wait for ApproveWithdraw on Terra

3. **Implement actual transfer tests**
   - Terra lock → EVM withdrawal
   - EVM deposit → Terra unlock

**Acceptance Criteria:**
- [ ] `make e2e-test` runs without manual intervention
- [ ] Tests complete in under 5 minutes
- [ ] Clear pass/fail output

---

## Technical Specifications

### Vitest Configuration

```typescript
// packages/frontend/vitest.config.ts
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test/setup.ts',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: ['node_modules/', 'src/test/'],
    },
  },
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
})
```

### Test Setup File

```typescript
// packages/frontend/src/test/setup.ts
import '@testing-library/jest-dom'
import { vi } from 'vitest'

// Mock window.matchMedia for component tests
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation(query => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
})

// Mock wallet extensions
vi.mock('@goblinhunt/cosmes/wallet', () => ({
  StationController: vi.fn(),
  KeplrController: vi.fn(),
  // ... other controllers
}))
```

### Example Test Files

```typescript
// src/utils/format.test.ts
import { describe, it, expect } from 'vitest'
import { formatAmount, parseAmount, formatAddress } from './format'

describe('formatAmount', () => {
  it('formats micro amounts to human readable', () => {
    expect(formatAmount('1000000', 6)).toBe('1.00')
    expect(formatAmount('1500000', 6)).toBe('1.50')
  })

  it('handles zero', () => {
    expect(formatAmount('0', 6)).toBe('0.00')
  })

  it('handles large amounts', () => {
    expect(formatAmount('1000000000000', 6)).toBe('1,000,000.00')
  })
})

describe('formatAddress', () => {
  it('truncates long addresses', () => {
    const addr = 'terra1abcdefghijklmnopqrstuvwxyz123456'
    expect(formatAddress(addr, 6)).toBe('terra1...23456')
  })

  it('returns short addresses unchanged', () => {
    expect(formatAddress('0x1234', 8)).toBe('0x1234')
  })
})
```

```typescript
// src/stores/wallet.test.ts
import { describe, it, expect, beforeEach } from 'vitest'
import { useWalletStore } from './wallet'

describe('useWalletStore', () => {
  beforeEach(() => {
    useWalletStore.setState({
      connected: false,
      address: null,
      luncBalance: '0',
    })
  })

  it('has correct initial state', () => {
    const state = useWalletStore.getState()
    expect(state.connected).toBe(false)
    expect(state.address).toBeNull()
  })

  it('updates balances', () => {
    useWalletStore.getState().setBalances({ lunc: '1000000' })
    expect(useWalletStore.getState().luncBalance).toBe('1000000')
  })
})
```

---

## Process Improvements

Based on Sprint 6 experience, recommended changes:

### 1. Test-First for New Features
- Write test stubs before implementing features
- Use TDD for utility functions
- Prevents accumulating test debt

### 2. Check Build Before Moving On
- Run `npm run build` after each major change
- Run `cargo check` frequently
- Catches issues early

### 3. Use WorkSplit for Repetitive Code
- Test file generation is a good WorkSplit use case
- Creates consistent patterns

### 4. Size Budget for Frontend
- Set bundle size limits in CI
- Alert when approaching limits

### 5. Integration Test Early
- Don't wait until end of sprint
- Test operator + frontend together weekly

---

## Definition of Done

### For Sprint 7 to be complete:

1. **Testing**
   - [ ] Vitest running with `npm run test`
   - [ ] 20+ frontend tests passing
   - [ ] 10+ canceler tests passing
   - [ ] Test coverage report generated

2. **Features**
   - [ ] EVM deposit works from UI
   - [ ] Transaction status shown in UI
   - [ ] Both transfer directions work E2E

3. **Quality**
   - [ ] Main bundle under 500KB
   - [ ] No TypeScript errors
   - [ ] No ESLint warnings
   - [ ] Cargo clippy passes

4. **Documentation**
   - [ ] Test instructions in README
   - [ ] Hook documentation in frontend.md

---

## Notes for Next Agent

### Quick Start
```bash
# Run all tests
make test

# Frontend tests
cd packages/frontend && npm run test

# Operator tests
cd packages/operator && cargo test

# E2E tests
make e2e-test --full
```

### Key Files to Modify

| Task | Files |
|------|-------|
| Vitest setup | `packages/frontend/package.json`, `vitest.config.ts` |
| Frontend tests | `packages/frontend/src/**/*.test.ts` |
| EVM deposit | `packages/frontend/src/hooks/useBridgeDeposit.ts` |
| Canceler tests | `packages/canceler/src/*.rs` |
| Bundle optimization | `packages/frontend/vite.config.ts` |

### Known Issues

1. **Cosmes removeNull()** - The cosmes library strips null values after signing which can cause signature mismatches. See `services/wallet.ts` comments.

2. **LocalTerra Gas** - Terra Classic LCD doesn't support simulation endpoint, so we use fixed gas limits.

3. **Large Dependencies** - wagmi and cosmes add significant bundle size. Consider if all wallets are needed.

### Reference Projects

- `@golbinhunt/cosmes` implementation: `/home/answorld/repos/ustr-cmm/frontend/src`
- Working Terra wallet pattern: ustr-cmm frontend

---

## Sprint Timeline Suggestion

| Phase | Focus | Deliverable |
|-------|-------|-------------|
| Phase 1 | Vitest setup | Tests running |
| Phase 2 | Utility tests | 80% coverage on utils |
| Phase 3 | Component tests | BridgeForm tested |
| Phase 4 | EVM deposit | Feature complete |
| Phase 5 | Bundle optimization | Size reduced |
| Phase 6 | E2E automation | Full automation |

---

*Created: 2026-02-02*
*Previous Sprint: SPRINT6.md - Frontend & Devnet Validation*
