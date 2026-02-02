# Frontend

The CL8Y Bridge frontend is a React-based web application that provides a user interface for cross-chain transfers between Terra Classic and EVM-compatible chains.

**Source:** [packages/frontend/](../packages/frontend/)

## Overview

```mermaid
flowchart TB
    subgraph Browser[User Browser]
        UI[React App]
        Wagmi[wagmi/viem]
        Cosmes[@goblinhunt/cosmes]
    end

    subgraph Backend[Backend Services]
        EVM[EVM RPC]
        Terra[Terra LCD]
    end

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
| wagmi | 2.x | EVM wallet connection |
| viem | 2.x | EVM interactions |
| @goblinhunt/cosmes | latest | Terra Classic wallet integration |
| zustand | 4.x | State management for Terra wallet |
| @tanstack/react-query | 5.x | Data fetching and caching |

## Project Structure

```
packages/frontend/
├── src/
│   ├── main.tsx              # Entry point with providers
│   ├── App.tsx               # Main app with tabs
│   ├── index.css             # Global styles (Tailwind)
│   ├── components/
│   │   ├── BridgeForm.tsx    # Main bridge interface
│   │   ├── ConnectWallet.tsx # EVM wallet connection
│   │   ├── WalletButton.tsx  # Terra wallet connection
│   │   └── TransactionHistory.tsx
│   ├── hooks/
│   │   ├── useWallet.ts      # Terra wallet hook
│   │   └── useContract.ts    # Contract query hooks
│   ├── services/
│   │   └── wallet.ts         # Terra wallet via cosmes
│   ├── stores/
│   │   └── wallet.ts         # Zustand wallet store
│   ├── lib/
│   │   ├── wagmi.ts          # EVM wallet config
│   │   └── chains.ts         # Chain definitions
│   └── utils/
│       ├── constants.ts      # Network/contract config
│       └── format.ts         # Formatting utilities
├── public/
├── index.html
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

## Components

### WalletButton (Terra)

Handles Terra Classic wallet connection using `@goblinhunt/cosmes`.

```tsx
<WalletButton />
```

Supported wallets:
- Terra Station (browser extension)
- Keplr (browser extension)
- Leap (browser extension)
- Cosmostation (browser extension)
- LUNC Dash (WalletConnect)
- Galaxy Station (WalletConnect)

### ConnectWallet (EVM)

Handles EVM wallet connection using wagmi.

```tsx
<ConnectWallet />
```

Supported wallets:
- MetaMask (injected)
- Other injected wallets

### BridgeForm

Main bridge interface for initiating transfers.

```tsx
<BridgeForm />
```

Features:
- Source chain selection (Terra, BSC, Anvil)
- Destination chain selection
- Amount input with validation
- Optional recipient address
- Fee breakdown display
- Transaction submission (Terra → EVM via cosmes)
- Loading and error states

### TransactionHistory

Displays user's bridge transactions.

```tsx
<TransactionHistory />
```

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

# WalletConnect Project ID (optional, for mobile wallets)
VITE_WC_PROJECT_ID=your_project_id
```

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
- `services/wallet.ts` - Core wallet functions
- `stores/wallet.ts` - Zustand state management
- `hooks/useWallet.ts` - React hook for components

### EVM Wallet (wagmi)

The EVM wallet integration uses wagmi with:

- Injected wallet detection (MetaMask, etc.)
- Chain switching support
- Transaction hooks

Key files:
- `lib/wagmi.ts` - wagmi configuration
- `lib/chains.ts` - Chain definitions
- `components/ConnectWallet.tsx` - Connection UI

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
| Cards | Slightly lighter | `bg-gray-800` |
| Primary button | Blue gradient | `bg-gradient-to-r from-blue-600 to-purple-600` |
| Text primary | White | `text-white` |
| Text secondary | Gray | `text-gray-400` |

## Current Status

### Implemented
- [x] Project setup (Vite, React, TypeScript)
- [x] TailwindCSS configuration
- [x] wagmi EVM wallet configuration
- [x] cosmes Terra wallet integration
- [x] Zustand state management
- [x] Chain definitions
- [x] ConnectWallet component (EVM)
- [x] WalletButton component (Terra)
- [x] BridgeForm component with transaction logic
- [x] TransactionHistory component (UI)
- [x] Responsive design
- [x] Dark theme
- [x] Terra → EVM lock transaction
- [x] EVM → Terra deposit transaction (useBridgeDeposit hook)
- [x] Approve → Deposit two-step flow
- [x] Vitest unit tests (62 tests)
- [x] Integration tests with real infrastructure
- [x] Bundle optimization (10KB gzipped initial load)

### TODO
- [ ] Real-time transaction status updates
- [ ] Transaction history persistence
- [ ] Error recovery and retry
- [ ] Mobile optimization
- [ ] E2E tests with Playwright

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

## Testing

```bash
# Run unit tests
npm run test:unit

# Run integration tests (requires LocalTerra + Anvil)
npm run test:integration

# Watch mode
npm run test

# Coverage
npm run test:coverage
```

## Related Documentation

- [System Architecture](./architecture.md) - Overall system design
- [Local Development](./local-development.md) - Development environment setup
- [EVM Contracts](./contracts-evm.md) - Smart contract documentation
- [Terra Contracts](./contracts-terraclassic.md) - CosmWasm documentation
- [Operator](./operator.md) - Backend API documentation
