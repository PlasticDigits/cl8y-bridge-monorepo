# Frontend

The CL8Y Bridge frontend is a React-based web application that provides a user interface for cross-chain transfers between Terra Classic and EVM-compatible chains, hash verification, and system settings.

**Source:** [packages/frontend/](../packages/frontend/)

## Overview

```mermaid
flowchart TB
    subgraph Browser[User Browser]
        UI[React App]
        Router[React Router]
        Wagmi[wagmi/viem]
        Cosmes[@goblinhunt/cosmes]
    end

    subgraph Backend[Backend Services]
        EVM[EVM RPC]
        Terra[Terra LCD]
    end

    UI --> Router
    UI --> Wagmi
    UI --> Cosmes
    Wagmi --> EVM
    Cosmes --> Terra
```

## Tech Stack

| Technology | Version | Purpose |
|------------|---------|---------|
| React | 18 | UI framework |
| TypeScript | 5.x | Type safety |
| Vite | 5.x | Build tool (static output) |
| TailwindCSS | 3.x | Styling |
| React Router | 6.x | Client-side routing |
| wagmi | 2.x | EVM wallet connection |
| viem | 2.x | EVM interactions |
| @goblinhunt/cosmes | latest | Terra Classic wallet integration |
| zustand | 5.x | State management |
| @tanstack/react-query | 5.x | Data fetching and caching |
| Vitest | 4.x | Testing framework |

## Project Structure

```
packages/frontend/
├── src/
│   ├── main.tsx              # Entry point with providers (WagmiProvider, QueryClientProvider, BrowserRouter)
│   ├── App.tsx               # Root component with Routes
│   ├── index.css             # Global styles (Tailwind)
│   │
│   ├── pages/                # Page-level components
│   │   ├── TransferPage.tsx       # Crosschain transfers
│   │   ├── HistoryPage.tsx         # Transaction history
│   │   ├── HashVerificationPage.tsx # Hash verification & matching
│   │   └── SettingsPage.tsx       # System settings (read-only)
│   │
│   ├── components/
│   │   ├── Layout.tsx        # App shell (header, nav, footer)
│   │   ├── NavBar.tsx        # Navigation links + wallet buttons
│   │   ├── ConnectWallet.tsx # EVM wallet connection (EIP-6963)
│   │   ├── WalletButton.tsx # Terra wallet connection
│   │   │
│   │   ├── wallet/           # Wallet connection UI
│   │   │   ├── EvmWalletModal.tsx
│   │   │   ├── EvmWalletOption.tsx
│   │   │   ├── TerraWalletModal.tsx
│   │   │   ├── TerraWalletOption.tsx
│   │   │   └── WalletIcons.tsx
│   │   │
│   │   ├── transfer/         # Crosschain transfer components
│   │   │   ├── TransferForm.tsx
│   │   │   ├── SourceChainSelector.tsx
│   │   │   ├── DestChainSelector.tsx
│   │   │   ├── AmountInput.tsx
│   │   │   ├── RecipientInput.tsx
│   │   │   ├── FeeBreakdown.tsx
│   │   │   ├── SwapDirectionButton.tsx
│   │   │   ├── ActiveTransferCard.tsx
│   │   │   ├── RecentTransfers.tsx
│   │   │   ├── TransferStatusBadge.tsx
│   │   │   └── WalletStatusBar.tsx
│   │   │
│   │   ├── verify/           # Hash verification components
│   │   │   ├── HashSearchBar.tsx
│   │   │   ├── HashComparisonPanel.tsx
│   │   │   ├── SourceHashCard.tsx
│   │   │   ├── DestHashCard.tsx
│   │   │   ├── ComparisonIndicator.tsx
│   │   │   ├── StatusBadge.tsx
│   │   │   ├── FraudAlert.tsx
│   │   │   ├── CancelInfo.tsx
│   │   │   ├── HashFieldsTable.tsx
│   │   │   └── RecentVerifications.tsx
│   │   │
│   │   ├── settings/         # System settings components
│   │   │   ├── ChainsPanel.tsx
│   │   │   ├── ChainCard.tsx
│   │   │   ├── TokensPanel.tsx
│   │   │   ├── TokenCard.tsx
│   │   │   ├── BridgeConfigPanel.tsx
│   │   │   └── ConnectionStatus.tsx
│   │   │
│   │   └── ui/               # Shared UI primitives
│   │       ├── Modal.tsx
│   │       ├── Badge.tsx
│   │       ├── Spinner.tsx
│   │       ├── Card.tsx
│   │       └── CopyButton.tsx
│   │
│   ├── hooks/
│   │   ├── useWallet.ts              # Terra wallet hook
│   │   ├── useEvmWalletDiscovery.ts  # EIP-6963 discovery
│   │   ├── useBridgeDeposit.ts       # EVM→Terra deposit flow
│   │   ├── useTerraDeposit.ts        # Terra→EVM lock flow
│   │   ├── useTransferHistory.ts      # Transfer persistence
│   │   ├── useHashVerification.ts   # Hash lookup + comparison
│   │   ├── useTransferLookup.ts      # Cross-chain event queries
│   │   ├── useMultiChainLookup.ts    # Multi-chain hash lookup
│   │   ├── useChainStatus.ts         # RPC/LCD health checks
│   │   ├── useBridgeSettings.ts      # On-chain bridge config
│   │   └── useTokenRegistry.ts       # On-chain token registry
│   │
│   ├── services/
│   │   ├── terra/            # Terra wallet services (split from wallet.ts)
│   │   │   ├── controllers.ts
│   │   │   ├── connect.ts
│   │   │   ├── transaction.ts
│   │   │   ├── detect.ts
│   │   │   ├── types.ts
│   │   │   └── index.ts
│   │   ├── wallet.ts         # Re-export barrel (backward compat)
│   │   ├── hashVerification.ts # Hash computation + chain queries
│   │   ├── lcdClient.ts      # Terra LCD client
│   │   ├── evmClient.ts      # EVM RPC client
│   │   ├── chainDiscovery.ts # Chain ID discovery
│   │   ├── terraBridgeQueries.ts
│   │   └── evmBridgeQueries.ts
│   │
│   ├── stores/
│   │   ├── wallet.ts         # Terra wallet state (Zustand)
│   │   ├── transfer.ts       # Active transfer tracking
│   │   └── ui.ts             # UI state (modals, etc.)
│   │
│   ├── types/
│   │   ├── transfer.ts       # Transfer, hash, status types
│   │   ├── chain.ts          # Chain config types
│   │   └── token.ts          # Token config types
│   │
│   ├── lib/
│   │   ├── wagmi.ts          # EVM wallet config (EIP-6963)
│   │   └── chains.ts         # Chain definitions
│   │
│   ├── utils/
│   │   ├── constants.ts      # Network/contract config
│   │   ├── format.ts         # Human-readable amounts, rates, addresses (see below)
│   │   ├── bigintAmount.ts   # Rational base-unit → human (bigint-safe)
│   │   ├── pow10.ts          # pow10BigInt (no unsafe Number 10**n for BigInt)
│   │   ├── scientificDecimal.ts # Expand e-notation strings for exact parsing
│   │   ├── validation.ts     # Address validation
│   │   ├── bridgeChains.ts   # Bridge chain configuration
│   │   └── chainLabel.ts     # Chain label utilities
│   │
│   └── test/
│       ├── setup.ts          # Vitest setup
│       └── helpers.tsx       # Test render helpers with providers
│
├── public/
├── index.html
├── package.json
├── vite.config.ts
├── vitest.config.ts
├── tailwind.config.js
└── tsconfig.json
```

## Token amounts and formatting

On-chain amounts are expressed in **base units** (smallest denomination). The UI converts them using the token’s **decimals** (e.g. 6 for many Cosmos assets, 9 for SOL, 18 for ERC-20).

- **`parseAmount` / `parseAmountAsBigInt`** turn a **human** decimal string (what the user types) into base units, using **bigint-only** math so large values never hit JavaScript double scientific notation (`1e+21`) that `BigInt()` cannot parse.
- **`formatAmount`**, **`formatAmountForNumberInput`**, and **`formatCompact`** take a **micro/base-unit** value (`string`, safe integer `number`, or `bigint`). They first parse the value as a **rational** (integer or decimal string, including forms like `1e+6` after expansion) in `bigintAmount.ts`, then format with exact arithmetic.
- If a value **cannot** be interpreted as a rational micro amount (garbage string, `NaN`, etc.), the helpers **`console.warn`** with a `[cl8y-bridge/format]` prefix and return a **sentinel**: an em dash (`—`) for display and **`formatCompact`**, and an **empty string** for **`formatAmountForNumberInput`** (valid for `type="number"` inputs). That avoids silent `NaN` or misleading float rounding.

Supporting modules: **`pow10.ts`** for \(10^n\) as `bigint`, **`scientificDecimal.ts`** for normalizing scientific notation strings before parsing.

## Routes

| Path | Page | Description |
|------|------|-------------|
| `/` | `TransferPage` | Crosschain transfers (EVM ↔ Terra) |
| `/history` | `HistoryPage` | Transaction history |
| `/verify` | `HashVerificationPage` | Hash verification & matching |
| `/settings` | `SettingsPage` | System settings (read-only) |

All routes are lazy-loaded via `React.lazy()` for code splitting.

## Components

### Pages

#### TransferPage
Main page for initiating crosschain transfers.

**Features:**
- Wallet status bar (EVM + Terra)
- Transfer direction selector (EVM→Terra / Terra→EVM)
- Source/destination chain selection
- Amount input with balance display
- Recipient address input
- Fee breakdown
- Active transfer tracking
- Recent transfers list

#### HistoryPage
Displays user's bridge transaction history (localStorage-backed).

#### HashVerificationPage
Page for operators/users to verify and match transaction hashes across chains.

**Features:**
- Hash search bar (64-char hex)
- Side-by-side source ↔ destination comparison
- Field-by-field diff highlighting
- Status badges (verified, pending, canceled, fraudulent) — dest Approved stays **Pending** until Executed (**INV-FE-VERIFY-1**)
- EVM dest execute blockers (cancel remaining + GL-127 rate-limit banners)
- Fraud indicators
- Cancel information
- Recent verifications list

#### SettingsPage
Read-only system settings page with tabs:

- **Chains**: Registered bridge chains with connection status
- **Tokens**: Registered tokens with bridge mode (LockUnlock/MintBurn)
- **Bridge Config**: Bridge configuration (withdraw delay, fees, operators, cancelers)

### Wallet Components

#### EvmWalletModal
Modal for EVM wallet selection using EIP-6963 multi-provider discovery.

**Supported wallets:**
- MetaMask
- Coinbase Wallet
- WalletConnect
- Any EIP-6963 compatible wallet

#### TerraWalletModal
Modal for Terra Classic wallet selection.

**Supported wallets:**
- Terra Station (extension; WalletConnect on mobile Chrome when not injected)
- Keplr (extension or Keplr-compatible inject; WalletConnect on mobile Chrome when `window.keplr` is absent — **INV-FE-WC-MOBILE-1** / GL-137)
- Leap (desktop extension only; hidden on mobile)
- Cosmostation (extension; WalletConnect on mobile when not injected)
- LUNC Dash (WalletConnect)
- Galaxy Station (WalletConnect)

Header CTA: [`WalletButton.tsx`](../packages/frontend/src/components/WalletButton.tsx) — accessible name **Connect Terra Wallet** (`aria-label`) so mobile is not limited to the visually hidden `CONNECT TC` span. Same-device WalletConnect uses Open / Copy ([`walletConnectPairing.ts`](../packages/frontend/src/utils/walletConnectPairing.ts)); do not auto-redirect from an async WC callback; do not rotate the pairing URI when Chrome returns from the wallet app ([`walletConnectForeground.ts`](../packages/frontend/src/services/terra/walletConnectForeground.ts)). Agent skill: [`skills/agent-frontend-terra-wallet-mobile.md`](../skills/agent-frontend-terra-wallet-mobile.md).

### Transfer Components

#### TransferForm
Main form for initiating transfers. Handles both directions:
- **Terra → EVM**: Lock transaction via Terra bridge contract
- **EVM → Terra**: Approve + deposit via EVM router contract

#### ActiveTransferCard
Real-time transfer progress tracker showing current status.

#### RecentTransfers
Compact list of last 5 completed transfers.

### Verify Components

#### HashComparisonPanel
Side-by-side comparison of source and destination chain data.

#### HashFieldsTable
Field-by-field comparison table with diff highlighting for mismatches.

#### StatusBadge
Color-coded status indicator (verified, pending, canceled, fraudulent).

### Settings Components

#### ChainsPanel
Grid of chain cards showing:
- Chain name, ID, type (EVM/Cosmos)
- RPC/LCD endpoint
- Explorer URL
- Connection status (green/red with latency)

#### TokensPanel
Grid of token cards showing:
- Token symbol, name, decimals
- Bridge mode (LockUnlock / MintBurn)
- EVM contract address (with copy button)
- Registered chains

#### BridgeConfigPanel
Bridge configuration display:
- Terra bridge: withdraw delay, min/max transfer, fee %, admin, operators, cancelers, paused status
- EVM bridge: cancel window

## Configuration

### Environment Variables

Create `.env.local`:

```bash
# Network selection (local, testnet, mainnet)
VITE_NETWORK=local

# Contract addresses (set by deploy scripts)
VITE_TERRA_BRIDGE_ADDRESS=terra1...
VITE_EVM_BRIDGE_ADDRESS=0x...
VITE_EVM_ROUTER_ADDRESS=0x...

# EVM RPC URLs (optional, defaults in constants.ts)
VITE_ETH_RPC_URL=https://ethereum-rpc.publicnode.com
VITE_BSC_RPC_URL=https://bsc-dataseed1.binance.org
VITE_OPBNB_TESTNET_RPC_URL=https://opbnb-testnet-rpc.bnbchain.org

# WalletConnect Project ID (optional, for mobile wallets)
VITE_WC_PROJECT_ID=your_project_id
```

### Legal clickwrap (GL-134)

Production property is always **`bridge.cl8y.com`**. The SPA uses `@plasticdigits/cl8y-clickwrap@0.1.1` (`TermsGate`) on mutative CTAs only — not the app shell. See [FRONTEND_BRIDGE_INVARIANTS.md](./FRONTEND_BRIDGE_INVARIANTS.md) **INV-FE-CLICKWRAP-1** and [`skills/agent-frontend-clickwrap.md`](../skills/agent-frontend-clickwrap.md).

Local Vite (`http://localhost:3000`) needs Legal API `CORS_ORIGINS` and portal redirect allowlisting (or mocked status in unit/Playwright tests). Playwright intercepts `https://api.terms.cl8y.com/**` with `signed_latest: true` by default. Production env overrides may only target `https://api.terms.cl8y.com` / `https://terms.cl8y.com`.

### Network Configuration

Networks are configured in `src/utils/constants.ts`:

```typescript
export const NETWORKS = {
  local: {
    terra: { chainId: 'localterra', lcd: 'http://localhost:1317', ... },
    evm: { chainId: 31337, rpc: 'http://localhost:8545', ... },
  },
  testnet: {
    terra: { chainId: 'rebel-2', lcd: 'https://lcd.luncblaze.com', ... },
    evm: { chainId: 97, rpc: 'https://data-seed-prebsc-1-s1.binance.org:8545', ... },
  },
  mainnet: {
    terra: { chainId: 'columbus-5', lcd: 'https://terra-classic-lcd.publicnode.com', ... },
    evm: { chainId: 56, rpc: 'https://bsc-dataseed1.binance.org', ... },
  },
}
```

## Development

### Setup

```bash
cd packages/frontend

# Install dependencies
npm install

# Start dev server
npm run dev
```

The app runs at http://localhost:5173.

### Building

```bash
# Production build (static output)
npm run build

# Preview production build
npm run preview
```

The build outputs to `dist/` and can be deployed to any static host.

### Testing

```bash
# Run all tests
npm run test

# Run tests in watch mode
npm run test:watch

# Run tests with coverage
npm run test:coverage

# Run integration tests (requires LocalTerra + Anvil)
npm run test:integration
```

**Test Philosophy:**
- No mocks for blockchain calls (uses real infrastructure)
- Mock only UI state for component rendering tests
- Pure functions tested in isolation
- Integration tests require local infrastructure

### Linting

```bash
npm run lint
```

## Wallet Integration Details

### Terra Wallet (cosmes)

The Terra wallet integration uses `@goblinhunt/cosmes`, which provides:

- Multi-wallet support (Station, Keplr, Leap, etc.)
- WalletConnect support for mobile wallets
- Transaction signing and broadcasting
- Automatic sequence management with retry logic

Key files:
- `services/terra/` - Split wallet services (`walletConnectPairingHook.ts` intercepts cosmes QR on mobile)
- `stores/wallet.ts` - Zustand state management (`connecting` is not persisted; Cancel aborts in-flight connect via connect epoch; stale WC success must not `disconnectTerraWallet` while Retry's newer `connect()` is in flight — **INV-FE-WC-MOBILE-1**)
- `hooks/useWallet.ts` - React hook for components (`resumeWalletConnectAfterForeground` on visibility)
- `hooks/useDismissOnNavigate.ts` - close connected-wallet `fixed inset-0` backdrops on route change
- `utils/walletConnectPairing.ts` / `utils/terraConnectWalletOptions.ts` - GL-137 mobile pairing + Keplr WC rows
- `services/terra/walletConnectForeground.ts` - do not rotate pairing URI when returning from the wallet app

See **INV-FE-WC-MOBILE-1** in [FRONTEND_BRIDGE_INVARIANTS.md](./FRONTEND_BRIDGE_INVARIANTS.md).

### EVM Wallet (wagmi)

The EVM wallet integration uses wagmi with:

- EIP-6963 multi-provider discovery
- Injected wallet detection (MetaMask, etc.)
- WalletConnect and Coinbase Wallet connectors
- Chain switching support
- Transaction hooks

Key files:
- `lib/wagmi.ts` - wagmi configuration
- `lib/chains.ts` - Chain definitions
- `components/ConnectWallet.tsx` - Connection UI
- `hooks/useEvmWalletDiscovery.ts` - EIP-6963 discovery hook

## Styling

The app uses TailwindCSS with a dark theme:

```css
/* Key design tokens */
:root {
  --background: slate-900
  --foreground: slate-50
  --primary: blue-600
  --accent: purple-600
}
```

### Theme Colors

| Element | Color | Tailwind Class |
|---------|-------|----------------|
| Background | Dark slate | `bg-slate-900` / `bg-gray-900` |
| Cards | Slightly lighter | `bg-gray-800` / `bg-gray-900/50` |
| Primary button | Blue gradient | `bg-gradient-to-r from-blue-600 to-purple-600` |
| Text primary | White | `text-white` |
| Text secondary | Gray | `text-gray-400` |

## Hooks

### useBridgeDeposit

Hook for EVM → Terra deposits with approve → deposit flow.

```typescript
import { useBridgeDeposit } from '../hooks/useBridgeDeposit'

function MyComponent() {
  const {
    status,           // 'idle' | 'checking-allowance' | 'approving' | ...
    approvalTxHash,   // Transaction hash for approval
    depositTxHash,    // Transaction hash for deposit
    error,            // Error message if failed
    isLoading,        // True during transaction
    currentAllowance, // Current token allowance
    tokenBalance,     // User's token balance
    deposit,          // Function to execute deposit
    reset,            // Function to reset state
  } = useBridgeDeposit({
    tokenAddress: '0x...',
    lockUnlockAddress: '0x...',
  })

  const handleDeposit = () => {
    deposit('100', 'localterra', 'terra1...', 18)
  }
}
```

### useTerraDeposit

Hook for Terra → EVM lock transactions.

```typescript
import { useTerraDeposit } from '../hooks/useTerraDeposit'

function MyComponent() {
  const {
    lock,
    isLoading,
    txHash,
    error,
  } = useTerraDeposit()

  const handleLock = () => {
    lock({
      destChainId: '0x00000001', // Ethereum
      recipient: 'terra1...',
      amount: '1000000', // micro units
    })
  }
}
```

### useHashVerification

Hook for verifying transfer hashes across chains.

```typescript
import { useHashVerification } from '../hooks/useHashVerification'

function MyComponent() {
  const {
    data,
    isLoading,
    error,
    verify,
  } = useHashVerification()

  useEffect(() => {
    verify('0x...') // 64-char hex hash
  }, [])
}
```

## Current Status

### Implemented
- [x] Project setup (Vite, React, TypeScript)
- [x] TailwindCSS configuration
- [x] React Router (BrowserRouter)
- [x] wagmi EVM wallet configuration (EIP-6963)
- [x] cosmes Terra wallet integration
- [x] Zustand state management
- [x] Chain definitions
- [x] ConnectWallet component (EVM, EIP-6963)
- [x] WalletButton component (Terra)
- [x] TransferPage with TransferForm
- [x] HistoryPage
- [x] HashVerificationPage
- [x] SettingsPage (read-only)
- [x] Terra → EVM lock transaction
- [x] EVM → Terra deposit transaction
- [x] Approve → Deposit two-step flow
- [x] Hash verification & matching
- [x] Multi-chain hash lookup
- [x] System settings display
- [x] Responsive design
- [x] Dark theme
- [x] Vitest unit tests (361+ tests)
- [x] Integration tests with real infrastructure
- [x] Bundle optimization (code splitting, lazy loading)

### TODO
- [ ] Real-time transaction status updates (WebSocket)
- [ ] Transaction history backend sync
- [ ] Error recovery and retry
- [ ] Mobile optimization
- [ ] E2E tests with Playwright
- [ ] Settings page edit functionality (admin)

## Related Documentation

- [Frontend bridge UI invariants](./FRONTEND_BRIDGE_INVARIANTS.md) — transfer status destination rate-limit UX (**INV-UX2**, GL-127), Terra vs EVM decimal parity for **`queryTerraRateLimitStatus`** (**INV-UX2-TERRA1**, GL-130), Hash Verification EVM dest execute blockers (**INV-FE-VERIFY-1**, GL-170; skill [`agent-frontend-hash-verify.md`](../skills/agent-frontend-hash-verify.md)), symbol-only token logos (**INV-FE-TOKEN-LOGO-1**, GL-133; skill [`agent-frontend-token-logos.md`](../skills/agent-frontend-token-logos.md)), Transfer picker economic-then-test ranking (**INV-FE-TOKEN-RANK-1**, GL-136; skill [`agent-frontend-token-rank.md`](../skills/agent-frontend-token-rank.md)), Legal clickwrap (**INV-FE-CLICKWRAP-1**, GL-134; skill [`agent-frontend-clickwrap.md`](../skills/agent-frontend-clickwrap.md)), Android Chrome Terra connect (**INV-FE-WC-MOBILE-1**, GL-137; skill [`agent-frontend-terra-wallet-mobile.md`](../skills/agent-frontend-terra-wallet-mobile.md)), and Terra hash list vs active-withdrawal index (**INV-FE-TC-AW1**, GL-139; skill [`agent-terraclassic-active-withdrawals.md`](../skills/agent-terraclassic-active-withdrawals.md))
- [System Architecture](./architecture.md) - Overall system design
- [Local Development](./local-development.md) - Development environment setup
- [EVM Contracts](./contracts-evm.md) - Smart contract documentation
- [Terra Contracts](./contracts-terraclassic.md) - CosmWasm documentation
- [MetaMask / Blockaid (EVM)](./METAMASK_BLOCKAID_EVM.md) - Security alerts on BSC/opBNB bridge txs (GL-118); not a substitute for on-chain verification
- [Operator](./operator.md) - Backend API documentation
