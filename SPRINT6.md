# Sprint 6: Frontend Development & Production Validation

## Sprint 5 Handoff Notes

### What Went Well in Sprint 5

1. **Clean module architecture** - Created reusable modules (`terra_client.rs`, `evm_client.rs`, `retry.rs`)
2. **Type-safe EVM interactions** - Alloy's `sol!` macro provides compile-time contract verification
3. **Production retry system** - Error classification, exponential backoff, gas bumping
4. **Monitoring stack** - Prometheus + Grafana ready with pre-built dashboard
5. **Safety-first deployment** - Mainnet scripts have confirmation prompts

### What Needs Attention

1. **Transaction signing untested** - Terra/EVM signing compiles but hasn't been tested on testnet
2. **E2E tests shallow** - Check connectivity but don't execute actual transfers
3. **LocalTerra image changed** - ~~Switched to `ghcr.io/mint-cash/localterra:latest`~~ **RESOLVED in Sprint 9:** Now uses official `classic-terra/localterra-core:0.5.18` with config from `classic-terra/localterra`
4. **Canceler event polling** - `poll_evm_approvals()` and `poll_terra_approvals()` are stubs
5. **Dead code warnings** - Should clean up or actually use the code

---

## Sprint 6 Objectives

### Priority 1: Devnet E2E Validation (CRITICAL)

**Focus on local devnet only** - no live testnet this sprint. Validate the backend works with automated E2E tests:

1. **Local Infrastructure Setup**
   - Anvil for EVM (already working)
   - LocalTerra for Terra Classic
   - PostgreSQL for operator state
   - Verify all services start correctly with `make start`

2. **Automated E2E Transfer Tests**
   - Start local infrastructure
   - Deploy both contracts automatically
   - Run operator in background
   - Execute EVM→Terra transfer programmatically
   - Execute Terra→EVM transfer programmatically
   - Verify funds arrive correctly (balance checks)
   - All tests should run with a single `make e2e-test` command

3. **Signing Verification (Local)**
   - Verify cosmrs signing works with LocalTerra
   - Verify alloy signing works with Anvil
   - Test full transaction lifecycle on devnet

### Priority 2: Canceler Event Polling

Complete the canceler to actually monitor for fraudulent approvals:

```
Task 6.1: EVM Approval Event Polling
- Query WithdrawApproved events from EVM bridge
- Extract approval parameters
- Queue for verification

Task 6.2: Terra Approval Event Polling
- Query approve_withdraw transactions from Terra
- Use LCD tx search or contract query
- Queue for verification

Task 6.3: Integration Test
- Submit fraudulent approval
- Verify canceler detects and cancels
```

### Priority 3: Frontend Development

Build a user-facing web application for bridge transfers.

---

## Frontend Technical Specifications

### Stack (Static Vite Build)

**IMPORTANT**: This is a static frontend - no server-side rendering. Use Vite for fast development and static builds that can be deployed to any CDN/static host.

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Framework | **Vite + React 18** | Fast builds, static output, simple deployment |
| Styling | Tailwind CSS | Utility-first, fast development |
| Web3 EVM | wagmi + viem | Type-safe, modern, well-maintained |
| Web3 Terra | **@goblinhunt/cosmes** | Battle-tested Terra Classic wallet integration |
| State | **Zustand** + TanStack Query | Simple, performant, persisted wallet state |
| UI Components | Custom components | Tailwind-based, no heavy UI library |

### Why Static Vite (Not Next.js)

1. **Simplicity** - No server to deploy/manage, just static files
2. **Cost** - Free hosting on Vercel/Netlify/Cloudflare Pages
3. **Speed** - Vite's dev server is extremely fast
4. **Decentralization** - Static files can be served from IPFS
5. **Existing code** - Frontend already uses Vite

### Terra Wallet Integration

Use `@goblinhunt/cosmes` following the proven pattern from existing dapps:
- `services/wallet.ts` - Wallet connection, transaction signing
- `stores/wallet.ts` - Zustand store for wallet state
- `hooks/useWallet.ts` - React hook for components
- `services/contract.ts` - Contract queries and executions

Supported wallets: Station, Keplr, Leap, Cosmostation, LUNC Dash, Galaxy Station

### Core Features

1. **Wallet Connection**
   - MetaMask/WalletConnect for EVM
   - Terra Station for Terra Classic
   - Display connected addresses

2. **Bridge Interface**
   - Token selection
   - Amount input with max button
   - Source/destination chain selector
   - Fee display
   - Transfer button

3. **Transaction Status**
   - Deposit confirmation (source chain)
   - Approval pending (watchtower delay)
   - Execution confirmation (destination chain)
   - Error states with retry

4. **Transaction History**
   - Past transfers with status
   - Links to explorers
   - Pending transfers

### Directory Structure (Vite)

```
packages/frontend/
├── src/
│   ├── main.tsx              # Entry point with providers
│   ├── App.tsx               # Main app with routing
│   ├── index.css             # Global styles (Tailwind)
│   ├── components/
│   │   ├── BridgeForm.tsx    # Main bridge interface
│   │   ├── ChainSelector.tsx # Chain dropdown
│   │   ├── TokenInput.tsx    # Token/amount input
│   │   ├── TransferStatus.tsx # Transfer progress
│   │   ├── TransactionHistory.tsx
│   │   └── WalletButton.tsx  # Unified wallet connection
│   ├── hooks/
│   │   ├── useWallet.ts      # Wallet hook (Terra + EVM)
│   │   ├── useContract.ts    # Contract query hooks
│   │   └── useBridge.ts      # Bridge-specific hooks
│   ├── services/
│   │   ├── wallet.ts         # Terra wallet via @goblinhunt/cosmes
│   │   └── contract.ts       # Contract interactions
│   ├── stores/
│   │   └── wallet.ts         # Zustand wallet store
│   ├── lib/
│   │   ├── wagmi.ts          # EVM wallet config
│   │   └── chains.ts         # Chain definitions
│   ├── types/
│   │   └── contracts.ts      # Contract type definitions
│   └── utils/
│       ├── constants.ts      # Network/contract addresses
│       └── format.ts         # Formatting utilities
├── public/
│   └── ... (assets)
├── index.html
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── postcss.config.js
└── tsconfig.json
```

---

## Using WorkSplit for Frontend Development

WorkSplit is a tool for generating code files using LLM assistance. It's ideal for scaffolding the frontend since:
- TypeScript files have clear structure
- Components are self-contained
- We can batch-generate related files

### WorkSplit Setup

```bash
# WorkSplit is already configured in the repo
# Check the configuration
cat worksplit.toml

# Jobs are stored in jobs/
ls jobs/
```

### Creating Frontend Jobs

1. **Read the manager instructions first:**
```bash
cat jobs/_manager_instruction.md
```

2. **Create jobs for each component:**

```bash
# Create a job for a TypeScript component
worksplit new-job frontend_001_bridge_form -t replace \
  -o packages/frontend/src/components/ \
  -f BridgeForm.tsx

# This creates: jobs/frontend_001_bridge_form.md
```

3. **Edit the job file to add requirements:**

```markdown
---
output_dir: packages/frontend/src/components/
output_file: BridgeForm.tsx
---

# BridgeForm Component

Create a React component for the main bridge transfer form.

## Requirements

- Use React 18+ with TypeScript
- Accept props: `onTransfer: (params: TransferParams) => Promise<void>`
- Include fields for:
  - Source chain (dropdown)
  - Destination chain (dropdown)
  - Token selection
  - Amount input
- Display estimated fee
- Submit button with loading state
- Use Tailwind CSS for styling
- Use shadcn/ui Button and Input components

## Types

```typescript
interface TransferParams {
  sourceChain: 'evm' | 'terra';
  destChain: 'evm' | 'terra';
  token: string;
  amount: string;
  recipient: string;
}
```

## Dependencies

- Import Button from '@/components/ui/button'
- Import Input from '@/components/ui/input'
```

### Batching Related Jobs

Create multiple jobs with dependencies:

```bash
# Job 1: Types and interfaces
worksplit new-job frontend_001_types -t replace \
  -o packages/frontend/lib/ \
  -f types.ts

# Job 2: Bridge form (depends on types)
worksplit new-job frontend_002_bridge_form -t replace \
  -o packages/frontend/components/bridge/ \
  -f BridgeForm.tsx
```

Add dependency in job 2:
```yaml
---
output_dir: packages/frontend/components/bridge/
output_file: BridgeForm.tsx
depends_on:
  - frontend_001_types
---
```

### Running Jobs

**IMPORTANT: Always run `worksplit run` without flags for batch execution!**

```bash
# Run ALL pending jobs (correct way)
worksplit run

# Only use --job flag for debugging individual failures
worksplit run --job frontend_001_types
```

### Best Practices for TypeScript Jobs

1. **Keep jobs focused** - One component/file per job
2. **Define types first** - Create type definition jobs before component jobs
3. **Include import hints** - Tell the job what to import from where
4. **Specify styling approach** - Mention Tailwind classes expected
5. **Target 100-300 lines** - Split large components into smaller jobs
6. **Use REPLACE mode** - More reliable than EDIT mode for new files

### Example Job Batch for Frontend

```
packages/frontend/jobs/
├── frontend_001_types.md           # Core types and interfaces  
├── frontend_002_constants.md       # Network/contract addresses
├── frontend_003_wallet_service.md  # Terra wallet via cosmes
├── frontend_004_wallet_store.md    # Zustand wallet store
├── frontend_005_wallet_hook.md     # useWallet hook
├── frontend_006_wagmi_config.md    # EVM wallet config
├── frontend_007_wallet_button.md   # Unified wallet UI
├── frontend_008_chain_selector.md  # Chain dropdown
├── frontend_009_bridge_form.md     # Main form
└── frontend_010_app.md             # App component
```

**Note**: The frontend already has WorkSplit configured. See `packages/frontend/worksplit.toml`.

---

## Task Breakdown

### Week 1: Validation & Canceler

| Task | Priority | Estimated Effort |
|------|----------|------------------|
| 6.1 Test Terra signing on rebel-2 | Critical | 2-4 hours |
| 6.2 Test EVM signing on BSC testnet | Critical | 2-4 hours |
| 6.3 Full E2E with running operator | Critical | 4-6 hours |
| 6.4 Implement EVM event polling | High | 4-6 hours |
| 6.5 Implement Terra event polling | High | 4-6 hours |
| 6.6 Canceler integration test | High | 2-4 hours |

### Week 2: Frontend

| Task | Priority | Estimated Effort |
|------|----------|------------------|
| 6.7 Frontend scaffold (Next.js) | High | 2-4 hours |
| 6.8 Wallet connection (EVM + Terra) | High | 4-6 hours |
| 6.9 Bridge form component | High | 4-6 hours |
| 6.10 Transaction submission | High | 4-6 hours |
| 6.11 Status tracking | Medium | 4-6 hours |
| 6.12 Transaction history | Medium | 4-6 hours |

---

## Acceptance Criteria

### Devnet E2E Validation
- [ ] LocalTerra and Anvil start correctly with `make start`
- [ ] Contracts deploy successfully with `make deploy`
- [ ] Operator runs and detects deposit events
- [ ] Complete EVM→Terra transfer in local environment (automated test)
- [ ] Complete Terra→EVM transfer in local environment (automated test)
- [ ] Canceler event polling detects approvals on both chains
- [ ] Canceler can submit cancellation transactions
- [ ] All E2E tests pass with `make e2e-test`

### Frontend MVP
- [ ] Connect EVM wallets (MetaMask via wagmi)
- [ ] Connect Terra wallets (Station, Keplr, etc. via @goblinhunt/cosmes)
- [ ] Submit bridge transfer from UI (devnet)
- [ ] Display transfer status updates
- [ ] Show transaction history
- [ ] Error handling with user-friendly messages
- [ ] Static build works with `npm run build`

---

## Environment Variables for Devnet Testing

```bash
# LocalTerra (devnet)
TERRA_LCD_URL=http://localhost:1317
TERRA_RPC_URL=http://localhost:26657
TERRA_CHAIN_ID=localterra
# Default LocalTerra test account (pre-funded)
TERRA_MNEMONIC="notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius"

# Anvil (local EVM)
EVM_RPC_URL=http://localhost:8545
EVM_CHAIN_ID=31337
# Default Anvil test account (pre-funded with 10000 ETH)
EVM_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Database
DATABASE_URL=postgres://operator:operator@localhost:5433/operator
```

### Starting Devnet Infrastructure

```bash
# Start all services
make start

# Check status
make status

# Deploy contracts
make deploy

# Run E2E tests
make e2e-test
```

---

## References

- [SPRINT5.md](./SPRINT5.md) - Previous sprint (completed)
- [docs/architecture.md](./docs/architecture.md) - System design
- [docs/crosschain-flows.md](./docs/crosschain-flows.md) - Transfer sequences
- [jobs/_manager_instruction.md](./jobs/_manager_instruction.md) - WorkSplit guide
- [README.md in jobs/](./jobs/README.md) - Job success rates and best practices

---

## Notes for Next Agent

1. **Start with validation** - Don't build frontend until backend signing is tested
2. **Use rebel-2 testnet** - Terra Classic testnet for safe testing
3. **Check LocalTerra** - The docker image was changed; verify it starts correctly
4. **WorkSplit for frontend** - Use it for bulk TypeScript generation
5. **Keep jobs small** - 100-300 lines per job works best
6. **Run `worksplit run`** - Never run individual jobs in production workflow
