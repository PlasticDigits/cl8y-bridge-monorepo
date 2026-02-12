# PLAN_FEAT_F1: Frontend Improvement — Crosschain Transfers, Hash Verification, System Settings

**Created:** 2026-02-12
**Scope:** `packages/frontend/` (repo-wide paths used throughout)
**Status:** Draft — ready for interactive review

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Shared Foundation (Phase 0)](#2-shared-foundation-phase-0)
3. [Crosschain Transfers Page (Phase 1)](#3-crosschain-transfers-page-phase-1)
4. [Hash Verification & Matching Page (Phase 2)](#4-hash-verification--matching-page-phase-2)
5. [System Settings Page (Phase 3)](#5-system-settings-page-phase-3)
6. [Code Organization Constraints](#6-code-organization-constraints)
7. [Testing Plan](#7-testing-plan)
8. [Phase / Sprint Breakdown](#8-phase--sprint-breakdown)
9. [Dependency Order](#9-dependency-order)
10. [Risk & Resolved Decisions](#10-risk--resolved-decisions)

---

## 1. Current State Analysis

### Existing Frontend Architecture

| Layer | File | LOC | Purpose |
|-------|------|-----|---------|
| Entry | `packages/frontend/src/main.tsx` | 19 | WagmiProvider + QueryClientProvider |
| App Shell | `packages/frontend/src/App.tsx` | 70 | Header, tab nav (bridge / history), footer |
| Component | `src/components/BridgeForm.tsx` | 432 | Bridge form (both directions) |
| Component | `src/components/ConnectWallet.tsx` | 92 | EVM wallet button (injected only) |
| Component | `src/components/WalletButton.tsx` | 395 | Terra wallet modal (6 wallet types) |
| Component | `src/components/TransactionHistory.tsx` | 132 | localStorage-based tx list |
| Hook | `src/hooks/useWallet.ts` | 141 | Terra wallet state wrapper |
| Hook | `src/hooks/useBridgeDeposit.ts` | 480 | EVM→Terra approve+deposit flow |
| Hook | `src/hooks/useContract.ts` | 124 | LCD query helpers |
| Service | `src/services/wallet.ts` | 533 | cosmes wallet controllers, tx signing |
| Store | `src/stores/wallet.ts` | 163 | Zustand Terra wallet state |
| Lib | `src/lib/wagmi.ts` | 36 | wagmi config (injected connector only) |
| Lib | `src/lib/chains.ts` | 128 | ChainInfo definitions |
| Util | `src/utils/constants.ts` | 149 | Networks, contracts, config |
| Util | `src/utils/format.ts` | 151 | formatAmount, formatAddress, etc. |

### Key Observations

1. **No router** — App.tsx uses `useState` tabs; adding pages requires a router.
2. **EVM wallet is injected-only** — `ConnectWallet.tsx` uses `injected()` connector; no EIP-6963 multi-provider discovery.
3. **Terra wallet is mature** — `WalletButton.tsx` already supports 6 wallets (Station, Keplr, Leap, Cosmostation, LUNC Dash, Galaxy Station) via `@goblinhunt/cosmes`.
4. **`services/wallet.ts` is 533 LOC** — exceeds the 900-LOC budget today but will grow; needs splitting.
5. **No hash verification UI** — transfer hashes are computed in backend (operator/canceler) but not exposed to the frontend.
6. **No system settings page** — chain/token config lives in `constants.ts` with no admin UI.
7. **Testing** — 62 unit tests exist; integration tests hit real infra. Vitest + RTL + jsdom. No Playwright yet.

### Tech Stack (Unchanged)

- React 18, TypeScript 5, Vite 5, TailwindCSS 3
- wagmi 2 / viem 2 (EVM), @goblinhunt/cosmes (Terra)
- zustand 5 (state), @tanstack/react-query 5 (data fetching)
- Vitest 4 (tests), @testing-library/react 16

---

## 2. Shared Foundation (Phase 0)

Before building new pages, establish shared infrastructure that all three features depend on.

### 2a. Add Client-Side Router

**Why:** The app currently has only two "tabs" via `useState`. Three new pages require proper routing, URL persistence, and lazy-loading.

**Library:** `react-router-dom@6` (lightweight, standard). Use **BrowserRouter** — Render deployment requires `render.yaml` rewrite to serve SPA (fallback to `index.html` for client-side routes).

**New/Updated Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/main.tsx` | Update | Wrap `<App />` with `<BrowserRouter>` |
| `src/App.tsx` | Update | Replace tab state with `<Routes>` / `<Route>` / `<NavLink>` |
| `src/pages/TransferPage.tsx` | New | Lazy-loaded wrapper for crosschain transfers |
| `src/pages/HistoryPage.tsx` | New | Existing TransactionHistory, extracted |
| `src/pages/HashVerificationPage.tsx` | New | Hash verification & matching |
| `src/pages/SettingsPage.tsx` | New | System settings |
| `src/components/Layout.tsx` | New | Shared header, nav, footer (extracted from App.tsx) |
| `src/components/NavBar.tsx` | New | Top nav with links and wallet buttons |

**Route Map:**

| Path | Page | Lazy |
|------|------|------|
| `/` | `TransferPage` | Yes |
| `/history` | `HistoryPage` | Yes |
| `/verify` | `HashVerificationPage` | Yes |
| `/settings` | `SettingsPage` | Yes |

### 2b. EIP-6963 Multi-Provider Discovery

**Why:** Current `ConnectWallet.tsx` uses `injected()` only — this only finds a single injected provider (typically MetaMask). EIP-6963 lets the app discover **all** installed EVM wallets (MetaMask, Rabby, Coinbase Wallet, Trust Wallet, etc.) and present them in a modal.

**Approach:** wagmi 2 natively supports EIP-6963 via the `mipd` (Multi Injected Provider Discovery) store. We need to:

1. Enable the `multiInjectedProviderDiscovery` option on the wagmi config (it is `true` by default in wagmi 2, but we override the connector list).
2. Use `useConnectors()` from wagmi to enumerate discovered providers.
3. Build a wallet selection modal similar to `WalletButton.tsx` for EVM wallets.

**New/Updated Files:**

| File | Action | Description |
|------|--------|-------------|
| `src/lib/wagmi.ts` | Update | Remove hardcoded `injected()`, let wagmi auto-discover EIP-6963 providers; add `walletConnect` and `coinbaseWallet` connectors as fallbacks |
| `src/components/ConnectWallet.tsx` | Rewrite | Modal-based UI listing discovered EVM wallets with icons/names from EIP-6963 `rdns` |
| `src/components/wallet/EvmWalletModal.tsx` | New | Modal for EVM wallet selection |
| `src/components/wallet/EvmWalletOption.tsx` | New | Single wallet row (icon, name, connect button) |
| `src/hooks/useEvmWalletDiscovery.ts` | New | Wraps wagmi's `useConnectors()` + sorts by EIP-6963 priority |

### 2c. Split `services/wallet.ts`

The file is already 533 LOC and will grow when we add new signing flows. Split into focused modules:

| New File | Extracted From | Contents |
|----------|---------------|----------|
| `src/services/terra/controllers.ts` | `wallet.ts` lines 1–56 | Controller instantiation, CONTROLLERS map |
| `src/services/terra/connect.ts` | `wallet.ts` lines 107–202 | `connectTerraWallet`, `disconnectTerraWallet` |
| `src/services/terra/transaction.ts` | `wallet.ts` lines 230–450 | `executeContractWithCoins`, `executeCw20Send`, fee estimation |
| `src/services/terra/detect.ts` | `wallet.ts` lines 79–102 | `isStationInstalled`, `isKeplrInstalled`, etc. |
| `src/services/terra/types.ts` | `wallet.ts` lines 62–63 | `TerraWalletType`, window type augmentations |
| `src/services/terra/index.ts` | New | Barrel re-export |

`services/wallet.ts` becomes a thin re-export or is deleted.

### 2d. Shared Types & Constants

| File | Action | Description |
|------|--------|-------------|
| `src/types/transfer.ts` | New | `TransferHash`, `TransferRecord`, `TransferStatus`, `TransferDirection` |
| `src/types/chain.ts` | New | Extend `ChainInfo` with `bridgeContractAddress`, `explorerTxUrl` helper |
| `src/types/token.ts` | New | `TokenConfig`, `TokenRegistryEntry` |
| `src/types/index.ts` | New | Barrel re-export |
| `src/utils/constants.ts` | Update | Import types from `types/`, keep runtime values |

---

## 3. Crosschain Transfers Page (Phase 1)

### Overview

A dedicated page for initiating crosschain transfers with clear separation of EVM and Terra Classic flows.

### 3a. Page Structure

```
TransferPage
├── WalletStatusBar            # Shows connected EVM + Terra wallets at a glance
├── TransferDirectionSelector  # EVM→Terra or Terra→EVM toggle
├── TransferForm
│   ├── SourceChainSelector
│   ├── AmountInput
│   ├── SwapDirectionButton
│   ├── DestChainSelector
│   ├── RecipientInput
│   ├── FeeBreakdown
│   └── SubmitButton
├── ActiveTransferCard         # Shows in-progress transfer status
└── RecentTransfers            # Last 5 completed transfers (quick glance)
```

### 3b. New/Updated Files

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/pages/TransferPage.tsx` | New | ~120 | Page layout, orchestrates sub-components |
| `src/components/transfer/TransferForm.tsx` | New | ~350 | Main form logic (refactored from `BridgeForm.tsx`) |
| `src/components/transfer/SourceChainSelector.tsx` | New | ~80 | Chain dropdown with wallet status indicator |
| `src/components/transfer/DestChainSelector.tsx` | New | ~80 | Destination chain dropdown |
| `src/components/transfer/AmountInput.tsx` | New | ~100 | Amount input with balance display, max button |
| `src/components/transfer/RecipientInput.tsx` | New | ~80 | Address input with validation (0x… / terra1…) |
| `src/components/transfer/FeeBreakdown.tsx` | New | ~60 | Fee %, estimated time, receive amount |
| `src/components/transfer/SwapDirectionButton.tsx` | New | ~30 | Animated swap icon button |
| `src/components/transfer/ActiveTransferCard.tsx` | New | ~150 | Real-time transfer progress tracker |
| `src/components/transfer/RecentTransfers.tsx` | New | ~120 | Compact list of last 5 transfers |
| `src/components/transfer/TransferStatusBadge.tsx` | New | ~40 | Status pill (pending, confirmed, failed, canceled) |
| `src/components/transfer/WalletStatusBar.tsx` | New | ~80 | Horizontal bar showing both wallet connections |
| `src/components/transfer/index.ts` | New | ~15 | Barrel export |
| `src/hooks/useTerraDeposit.ts` | New | ~200 | Terra→EVM lock flow (extracted from BridgeForm inline logic) |
| `src/hooks/useTransferHistory.ts` | New | ~120 | Persist + query transfers from localStorage only (no backend) |
| `src/stores/transfer.ts` | New | ~100 | Zustand store for active transfer state |
| `src/components/BridgeForm.tsx` | Delete | — | Replaced by `transfer/TransferForm.tsx` |

### 3c. EVM Wallet Flow (EIP-6963)

1. User clicks "Connect EVM Wallet" → `EvmWalletModal` opens.
2. Modal lists all discovered EIP-6963 providers (MetaMask, Rabby, etc.) + WalletConnect fallback.
3. User selects provider → wagmi `connect()` with that connector.
4. On success, `WalletStatusBar` shows connected EVM address + chain.
5. If source chain requires EVM wallet and user hasn't connected, `TransferForm` shows inline prompt.

### 3d. Terra Wallet Flow

Already implemented via `WalletButton.tsx` → `useWallet` → `stores/wallet.ts` → `services/wallet.ts`.

Changes:
- Extract Terra connection logic from header into `WalletStatusBar` for contextual display.
- Keep `WalletButton.tsx` in the `NavBar` for global access.

### 3e. Transfer Flow Separation

| Direction | Source Wallet | Hook | Contract Call |
|-----------|--------------|------|---------------|
| Terra → EVM | Terra (cosmes) | `useTerraDeposit` | `lock { dest_chain_id, recipient }` + native coin |
| EVM → Terra | EVM (wagmi) | `useBridgeDeposit` | `approve()` → `router.deposit(token, amount, chainKey, destAccount)` |

Both hooks write to `stores/transfer.ts` so `ActiveTransferCard` can track progress regardless of direction. Transfer history uses **localStorage only** — the dapp works without any backend.

### 3f. Supported Chains

**opBNB, BSC, ETH, Terra Classic.** Update `chains.ts`, `constants.ts`, and `lib/wagmi.ts` to include all four. Ensure `NETWORKS` (local/testnet/mainnet) and wagmi `chains` array cover opBNB where deployed.

---

## 4. Hash Verification & Matching Page (Phase 2)

### Overview

A page for operators/users to verify and match transaction hashes across chains, flag fraudulent hashes, and view canceled transactions.

### 4a. Data Model

```typescript
// src/types/transfer.ts

export type HashStatus = 'verified' | 'pending' | 'canceled' | 'fraudulent' | 'unknown';

export interface TransferHash {
  hash: string;               // keccak256 transfer hash
  srcChain: string;           // Source chain ID
  destChain: string;          // Destination chain ID
  srcTxHash: string;          // Source chain transaction hash
  destTxHash: string | null;  // Destination chain tx hash (null if pending)
  srcAccount: string;         // Sender address
  destAccount: string;        // Recipient address
  token: string;              // Destination token (encoded)
  amount: string;             // Net amount (post-fee)
  nonce: string;              // Transfer nonce
  status: HashStatus;
  canceledAt?: number;        // Timestamp if canceled
  cancelReason?: string;      // Reason if canceled
  fraudIndicators?: string[]; // Why flagged as fraudulent
  createdAt: number;
  updatedAt: number;
}
```

### 4b. Data Source

**RPC/LCD only.** No operator API. On-chain data is the source of truth. `useTransferLookup` and `useHashVerification` query EVM RPC and Terra LCD directly for deposit/lock events and approval state.

### 4c. Page Structure

```
HashVerificationPage
├── HashSearchBar                # Input field for transfer hash lookup
├── HashComparisonPanel
│   ├── SourceHashCard           # Source chain deposit/lock details
│   ├── ComparisonIndicator      # Visual match/mismatch indicator
│   └── DestHashCard             # Destination chain approval/withdrawal details
├── StatusPanel
│   ├── StatusBadge              # verified / pending / canceled / fraudulent
│   ├── FraudAlert               # Red banner if fraudulent
│   └── CancelInfo               # Gray banner if canceled
├── HashFieldsTable              # Side-by-side field comparison
│   ├── srcChain row
│   ├── destChain row
│   ├── srcAccount row
│   ├── destAccount row
│   ├── token row
│   ├── amount row
│   └── nonce row
└── RecentVerifications          # List of recently verified hashes
```

### 4d. New Files

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/pages/HashVerificationPage.tsx` | New | ~150 | Page layout + search orchestration |
| `src/components/verify/HashSearchBar.tsx` | New | ~80 | Search input with hash validation (64-char hex) |
| `src/components/verify/HashComparisonPanel.tsx` | New | ~200 | Side-by-side source ↔ dest comparison |
| `src/components/verify/SourceHashCard.tsx` | New | ~150 | Source chain details card |
| `src/components/verify/DestHashCard.tsx` | New | ~150 | Destination chain details card |
| `src/components/verify/ComparisonIndicator.tsx` | New | ~60 | Match ✓ / Mismatch ✗ / Pending ⏳ visual |
| `src/components/verify/StatusBadge.tsx` | New | ~50 | Color-coded status pill |
| `src/components/verify/FraudAlert.tsx` | New | ~60 | Red alert banner with fraud indicators |
| `src/components/verify/CancelInfo.tsx` | New | ~50 | Gray banner with cancel timestamp + reason |
| `src/components/verify/HashFieldsTable.tsx` | New | ~180 | 7-row comparison table with diff highlighting |
| `src/components/verify/RecentVerifications.tsx` | New | ~100 | List of recent lookups (localStorage) |
| `src/components/verify/index.ts` | New | ~12 | Barrel export |
| `src/hooks/useHashVerification.ts` | New | ~200 | Fetch + compare hashes across chains |
| `src/hooks/useTransferLookup.ts` | New | ~150 | Query source chain events + dest chain approvals |
| `src/services/hashVerification.ts` | New | ~180 | Hash computation (matching `HashLib.computeTransferHash`), LCD/RPC queries for transfer data |

### 4e. Hash Computation (Frontend)

The frontend must compute transfer hashes identically to the smart contracts and backend. Reference implementation from `README.md`:

```
keccak256(abi.encode(srcChain, destChain, srcAccount, destAccount, token, amount, nonce))
```

Using `viem`:

```typescript
import { keccak256, encodeAbiParameters, parseAbiParameters } from 'viem';

export function computeTransferHash(
  srcChain: `0x${string}`,
  destChain: `0x${string}`,
  srcAccount: `0x${string}`,
  destAccount: `0x${string}`,
  token: `0x${string}`,
  amount: bigint,
  nonce: bigint
): `0x${string}` {
  const encoded = encodeAbiParameters(
    parseAbiParameters('bytes32, bytes32, bytes32, bytes32, bytes32, uint256, uint256'),
    [srcChain, destChain, srcAccount, destAccount, token, amount, nonce]
  );
  return keccak256(encoded);
}
```

This goes in `src/services/hashVerification.ts`.

### 4f. Status Visuals

| Status | Color | Icon | Background |
|--------|-------|------|------------|
| `verified` | Green | Checkmark | `bg-green-900/30 border-green-700` |
| `pending` | Yellow | Hourglass | `bg-yellow-900/30 border-yellow-700` |
| `canceled` | Gray | X-circle | `bg-gray-700/30 border-gray-500` |
| `fraudulent` | Red | Shield-exclamation | `bg-red-900/40 border-red-600` + pulsing glow |

### 4g. Diff-Style Comparison

`HashFieldsTable` renders each of the 7 transfer hash fields side by side:

```
┌─────────────┬──────────────────┬──────────────────┐
│ Field       │ Source (Deposit) │ Dest (Approval)  │
├─────────────┼──────────────────┼──────────────────┤
│ srcChain    │ 0x00…0038 ✓     │ 0x00…0038 ✓      │
│ destChain   │ 0x00…col5 ✓     │ 0x00…col5 ✓      │
│ srcAccount  │ 0xf39F…2266 ✓   │ 0xf39F…2266 ✓    │
│ destAccount │ 0xa1b2…c3d4 ✓   │ 0xa1b2…c3d4 ✓    │
│ token       │ 0x00…aabb ✓     │ 0x00…aabb ✓      │
│ amount      │ 999700 ✓        │ 999700 ✓         │
│ nonce       │ 42 ✓            │ 42 ✓             │
└─────────────┴──────────────────┴──────────────────┘
```

Mismatched fields get a red background + strikethrough on the differing value. This is the core of fraud detection visibility.

---

## 5. System Settings Page (Phase 3)

### Overview

A read-only settings page that displays registered chains, tokens, and bridge configuration.

### 5a. Page Structure

```
SettingsPage
├── Tabs: [Chains | Tokens | Bridge Config]
├── ChainsPanel
│   ├── ChainCard (per chain)
│   │   ├── Chain name, ID, type (EVM/Cosmos)
│   │   ├── RPC/LCD endpoint
│   │   ├── Explorer URL
│   │   └── Connection status indicator (green/red)
├── TokensPanel
│   └── TokenCard (per token)
│       ├── Symbol, name, decimals
│       ├── Contract address (with copy button)
│       ├── Bridge mode (MintBurn / LockUnlock)
│       └── Registered chains
└── BridgeConfigPanel
    ├── Withdraw delay
    ├── Fee percentage
    ├── Min transfer amount
    ├── Operator address
    └── Canceler addresses
```

### 5b. New Files

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/pages/SettingsPage.tsx` | New | ~120 | Page layout with sub-tabs |
| `src/components/settings/ChainsPanel.tsx` | New | ~200 | Chain list with status indicators (read-only) |
| `src/components/settings/ChainCard.tsx` | New | ~150 | Single chain detail card (no edit) |
| `src/components/settings/TokensPanel.tsx` | New | ~200 | Token list |
| `src/components/settings/TokenCard.tsx` | New | ~130 | Single token detail card |
| `src/components/settings/BridgeConfigPanel.tsx` | New | ~180 | Bridge config display |
| `src/components/settings/ConnectionStatus.tsx` | New | ~60 | Green/red indicator with latency |
| `src/components/settings/CopyButton.tsx` | New | ~40 | Click-to-copy with tooltip |
| `src/components/settings/index.ts` | New | ~10 | Barrel export |
| `src/hooks/useChainStatus.ts` | New | ~120 | Ping RPC/LCD endpoints, report status |
| `src/hooks/useBridgeSettings.ts` | New | ~150 | Query on-chain bridge config (withdraw delay, fee, etc.) |
| `src/hooks/useTokenRegistry.ts` | New | ~130 | Query EVM TokenRegistry contract for registered tokens |

### 5c. Data Sources

| Data | Source | Method |
|------|--------|--------|
| Registered EVM chains | `ChainRegistry.sol` | `useReadContract` (wagmi) |
| Registered tokens | `TokenRegistry.sol` | `useReadContract` (wagmi) |
| Terra bridge config | Terra bridge contract | `useContract` LCD query |
| Endpoint health | RPC/LCD URLs from `constants.ts` | `fetch()` with timeout |
| EVM bridge config | `CL8YBridge.sol` | `useReadContract` (wagmi) |

### 5d. Admin Edit

**Deferred to future.** Settings page is read-only. No edit buttons, no forms.

---

## 6. Code Organization Constraints

### 6a. 900 LOC Per File Rule

**Current violations to fix:**

| File | Current LOC | Action |
|------|-------------|--------|
| `src/services/wallet.ts` | 533 | Split per Phase 0 §2c (5 files, ~100 LOC each) |

**Prevention strategy:**

- All new components target **≤300 LOC** (including JSX).
- Pages are thin orchestrators (≤150 LOC) that compose components.
- Hooks contain business logic (≤250 LOC).
- If a component approaches 400 LOC, split into sub-components immediately.

### 6b. Module Structure

```
packages/frontend/src/
├── main.tsx
├── App.tsx
├── index.css
├── vite-env.d.ts
│
├── pages/                      # Page-level components (thin)
│   ├── TransferPage.tsx
│   ├── HistoryPage.tsx
│   ├── HashVerificationPage.tsx
│   └── SettingsPage.tsx
│
├── components/
│   ├── Layout.tsx              # Shell: header + main + footer
│   ├── NavBar.tsx              # Navigation links + wallet buttons
│   │
│   ├── wallet/                 # Wallet connection UI
│   │   ├── EvmWalletModal.tsx
│   │   ├── EvmWalletOption.tsx
│   │   ├── TerraWalletModal.tsx  (refactored from WalletButton.tsx)
│   │   ├── TerraWalletOption.tsx (extracted from WalletButton.tsx)
│   │   ├── WalletIcons.tsx       (extracted from WalletButton.tsx)
│   │   └── index.ts
│   │
│   ├── transfer/               # Crosschain transfer components
│   │   ├── TransferForm.tsx
│   │   ├── SourceChainSelector.tsx
│   │   ├── DestChainSelector.tsx
│   │   ├── AmountInput.tsx
│   │   ├── RecipientInput.tsx
│   │   ├── FeeBreakdown.tsx
│   │   ├── SwapDirectionButton.tsx
│   │   ├── ActiveTransferCard.tsx
│   │   ├── RecentTransfers.tsx
│   │   ├── TransferStatusBadge.tsx
│   │   ├── WalletStatusBar.tsx
│   │   └── index.ts
│   │
│   ├── verify/                 # Hash verification components
│   │   ├── HashSearchBar.tsx
│   │   ├── HashComparisonPanel.tsx
│   │   ├── SourceHashCard.tsx
│   │   ├── DestHashCard.tsx
│   │   ├── ComparisonIndicator.tsx
│   │   ├── StatusBadge.tsx
│   │   ├── FraudAlert.tsx
│   │   ├── CancelInfo.tsx
│   │   ├── HashFieldsTable.tsx
│   │   ├── RecentVerifications.tsx
│   │   └── index.ts
│   │
│   ├── settings/               # System settings components
│   │   ├── ChainsPanel.tsx
│   │   ├── ChainCard.tsx
│   │   ├── TokensPanel.tsx
│   │   ├── TokenCard.tsx
│   │   ├── BridgeConfigPanel.tsx
│   │   ├── ConnectionStatus.tsx
│   │   ├── CopyButton.tsx
│   │   └── index.ts
│   │
│   └── ui/                     # Shared UI primitives
│       ├── Modal.tsx           # Reusable modal (used by wallet modals)
│       ├── Badge.tsx           # Color-coded badge/pill
│       ├── Spinner.tsx         # Loading spinner
│       ├── Card.tsx            # Base card wrapper
│       └── index.ts
│
├── hooks/
│   ├── useWallet.ts            # Terra wallet (existing)
│   ├── useEvmWalletDiscovery.ts # EIP-6963 discovery
│   ├── useBridgeDeposit.ts     # EVM→Terra deposit (existing)
│   ├── useTerraDeposit.ts      # Terra→EVM lock (new)
│   ├── useTransferHistory.ts   # Transfer history persistence
│   ├── useContract.ts          # LCD queries (existing)
│   ├── useHashVerification.ts  # Hash lookup + comparison
│   ├── useTransferLookup.ts    # Cross-chain event queries
│   ├── useChainStatus.ts       # RPC/LCD health checks
│   ├── useBridgeSettings.ts    # On-chain bridge config
│   └── useTokenRegistry.ts     # On-chain token registry
│
├── services/
│   ├── terra/                  # Split from wallet.ts
│   │   ├── controllers.ts
│   │   ├── connect.ts
│   │   ├── transaction.ts
│   │   ├── detect.ts
│   │   ├── types.ts
│   │   └── index.ts
│   └── hashVerification.ts     # Hash computation + chain queries
│
├── stores/
│   ├── wallet.ts               # Terra wallet state (existing)
│   └── transfer.ts             # Active transfer tracking (new)
│
├── types/
│   ├── transfer.ts             # Transfer, hash, status types
│   ├── chain.ts                # Chain config types
│   ├── token.ts                # Token config types
│   └── index.ts
│
├── lib/
│   ├── wagmi.ts                # EVM wallet config (updated)
│   └── chains.ts               # Chain definitions (existing)
│
├── utils/
│   ├── constants.ts            # Runtime config (existing)
│   ├── format.ts               # Formatting (existing)
│   └── validation.ts           # Address validation helpers (new)
│
└── test/
    ├── setup.ts                # Vitest setup (existing)
    └── helpers.ts              # Test render helpers with providers (new)
```

### 6c. Composition Patterns

**Shared hooks to avoid duplication:**

| Hook | Used By | Purpose |
|------|---------|---------|
| `useWallet` | TransferForm, WalletStatusBar, NavBar | Terra wallet state |
| `useEvmWalletDiscovery` | EvmWalletModal, WalletStatusBar | EVM provider list |
| `useTransferHistory` | RecentTransfers, HistoryPage | Transfer persistence |
| `useChainStatus` | ChainCard, WalletStatusBar | RPC/LCD ping |

**Shared UI components:**

| Component | Used By |
|-----------|---------|
| `Modal` | EvmWalletModal, TerraWalletModal |
| `Badge` | TransferStatusBadge, StatusBadge, ConnectionStatus |
| `Spinner` | ActiveTransferCard, WalletButtons |
| `Card` | ChainCard, TokenCard, SourceHashCard, DestHashCard |
| `CopyButton` | TokenCard, HashFieldsTable, SourceHashCard |

---

## 7. Testing Plan

### 7a. Testing Philosophy (Unchanged)

Per the project's existing philosophy:
- **No mocks for blockchain.** All RPC/LCD calls use real infrastructure (Anvil, LocalTerra).
- **Mock only UI state** — connection status, form state for component rendering tests.
- **Pure functions** are tested in isolation (formatting, hashing, validation).

### 7b. Test File Placement

Every source file `foo.ts` or `Foo.tsx` has its test file **co-located**:

```
src/components/transfer/AmountInput.tsx
src/components/transfer/AmountInput.test.tsx    ← unit test

src/hooks/useHashVerification.ts
src/hooks/useHashVerification.test.ts           ← unit test
src/hooks/useHashVerification.integration.test.ts ← integration test

src/services/hashVerification.ts
src/services/hashVerification.test.ts           ← unit test
```

Integration tests use the `.integration.test.ts` suffix and are skipped when `SKIP_INTEGRATION=true`.

### 7c. Unit Tests

| Area | Files | Coverage Target | What to Test |
|------|-------|----------------|--------------|
| **Transfer components** | `TransferForm.test.tsx`, `AmountInput.test.tsx`, `RecipientInput.test.tsx`, `FeeBreakdown.test.tsx`, `SourceChainSelector.test.tsx`, `DestChainSelector.test.tsx` | ≥80% lines | Rendering, input validation, disabled states, direction swap |
| **Verify components** | `HashSearchBar.test.tsx`, `HashComparisonPanel.test.tsx`, `HashFieldsTable.test.tsx`, `StatusBadge.test.tsx`, `FraudAlert.test.tsx` | ≥80% lines | Search input validation, diff highlighting, status colors |
| **Settings components** | `ChainCard.test.tsx`, `TokenCard.test.tsx`, `BridgeConfigPanel.test.tsx`, `ConnectionStatus.test.tsx` | ≥80% lines | Data display, copy button, status indicator |
| **Wallet components** | `EvmWalletModal.test.tsx`, `EvmWalletOption.test.tsx` | ≥80% lines | Modal open/close, connector list rendering |
| **UI primitives** | `Modal.test.tsx`, `Badge.test.tsx`, `CopyButton.test.tsx` | ≥90% lines | Accessibility, keyboard nav, visual states |
| **Hooks** | `useEvmWalletDiscovery.test.ts`, `useTransferHistory.test.ts`, `useChainStatus.test.ts`, `useBridgeSettings.test.ts` | ≥80% lines | State transitions, localStorage persistence |
| **Services** | `hashVerification.test.ts`, `terra/connect.test.ts`, `terra/detect.test.ts` | ≥90% lines | Hash computation parity, wallet detection |
| **Utils** | `validation.test.ts` | ≥95% lines | Address regex, amount validation |
| **Types** | N/A | N/A | Types are checked at compile time |

### 7d. Integration Tests (Requires Infra)

| Test | File | Infra Required | What to Test |
|------|------|----------------|--------------|
| Terra wallet connect | `useWallet.integration.test.ts` | LocalTerra | Connect/disconnect, balance fetch |
| Terra lock tx | `useTerraDeposit.integration.test.ts` | LocalTerra + Anvil | Lock flow with real contract |
| EVM deposit tx | `useBridgeDeposit.integration.test.ts` | Anvil | Approve + deposit with real contracts |
| Hash verification | `useHashVerification.integration.test.ts` | Anvil + LocalTerra | Compute hash, query events, compare |
| Chain status | `useChainStatus.integration.test.ts` | Anvil + LocalTerra | Ping endpoints, verify latency |
| Bridge config query | `useBridgeSettings.integration.test.ts` | Anvil + LocalTerra | Query on-chain config |

### 7e. E2E Tests (Playwright — Future)

Playwright tests are listed in the existing TODO. When implemented:

- Use **20+ workers** per user rule.
- Test wallet connection flows (mocked wallet extension via Playwright fixture).
- Test full transfer initiation flow.
- Test hash verification page with known test hashes.
- Test settings page data display.

### 7f. Coverage Expectations

| Directory | Statement Coverage |
|-----------|-------------------|
| `src/utils/` | ≥95% |
| `src/services/` | ≥85% |
| `src/hooks/` | ≥80% |
| `src/components/ui/` | ≥90% |
| `src/components/transfer/` | ≥80% |
| `src/components/verify/` | ≥80% |
| `src/components/settings/` | ≥75% |
| `src/components/wallet/` | ≥80% |
| `src/pages/` | ≥70% |

---

## 8. Phase / Sprint Breakdown

### Phase 0: Shared Foundation (Sprint F1-A)

**Duration:** ~3 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 0.1 | Add `react-router-dom`, create `Layout.tsx`, `NavBar.tsx` | `main.tsx`, `App.tsx`, `Layout.tsx`, `NavBar.tsx` | — |
| 0.2 | Create page shells (`TransferPage`, `HistoryPage`, `HashVerificationPage`, `SettingsPage`) | `src/pages/*.tsx` | 0.1 |
| 0.3 | Extract shared UI primitives (`Modal`, `Badge`, `Spinner`, `Card`, `CopyButton`) | `src/components/ui/*` | — |
| 0.4 | Split `services/wallet.ts` into `services/terra/*` | `src/services/terra/*` | — |
| 0.5 | Create shared types (`types/transfer.ts`, `types/chain.ts`, `types/token.ts`) | `src/types/*` | — |
| 0.6 | Create `utils/validation.ts` (address validation, hex validation) | `src/utils/validation.ts` | 0.5 |
| 0.7 | Create `test/helpers.ts` (render with providers) | `src/test/helpers.ts` | 0.1 |
| 0.8 | Unit tests for UI primitives + validation utils | `*.test.tsx` / `*.test.ts` | 0.3, 0.6 |
| 0.9 | Update supported chains: opBNB, BSC, ETH, Terra Classic in `chains.ts`, `constants.ts`, `lib/wagmi.ts` | `chains.ts`, `constants.ts`, `wagmi.ts` | — |
| 0.10 | Add Render yaml SPA fallback for BrowserRouter | `render.yaml` (repo root or `.render/`) | 0.1 |

### Phase 1: Crosschain Transfers Page (Sprint F1-B)

**Duration:** ~5 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 1.1 | EIP-6963 wagmi config + `useEvmWalletDiscovery` hook | `src/lib/wagmi.ts`, `src/hooks/useEvmWalletDiscovery.ts` | 0.1 |
| 1.2 | `EvmWalletModal` + `EvmWalletOption` components | `src/components/wallet/EvmWalletModal.tsx`, `EvmWalletOption.tsx` | 1.1, 0.3 |
| 1.3 | Refactor `WalletButton.tsx` → `wallet/TerraWalletModal.tsx` + `TerraWalletOption.tsx` + `WalletIcons.tsx` | `src/components/wallet/*` | 0.3, 0.4 |
| 1.4 | `WalletStatusBar` component | `src/components/transfer/WalletStatusBar.tsx` | 1.1, 1.3 |
| 1.5 | Transfer form sub-components (`SourceChainSelector`, `DestChainSelector`, `AmountInput`, `RecipientInput`, `FeeBreakdown`, `SwapDirectionButton`) | `src/components/transfer/*` | 0.5, 0.6 |
| 1.6 | `TransferForm` composition (replaces `BridgeForm`) | `src/components/transfer/TransferForm.tsx` | 1.5 |
| 1.7 | `useTerraDeposit` hook (extract Terra lock logic) | `src/hooks/useTerraDeposit.ts` | 0.4 |
| 1.8 | `stores/transfer.ts` (active transfer state) | `src/stores/transfer.ts` | 0.5 |
| 1.9 | `ActiveTransferCard` + `TransferStatusBadge` | `src/components/transfer/ActiveTransferCard.tsx` | 1.8, 0.3 |
| 1.10 | `useTransferHistory` hook + `RecentTransfers` component | `src/hooks/useTransferHistory.ts`, `src/components/transfer/RecentTransfers.tsx` | 0.5 |
| 1.11 | `TransferPage` assembly | `src/pages/TransferPage.tsx` | 1.4–1.10 |
| 1.12 | Update `ConnectWallet.tsx` to use `EvmWalletModal` | `src/components/ConnectWallet.tsx` | 1.2 |
| 1.13 | Remove old `BridgeForm.tsx` | Delete `src/components/BridgeForm.tsx` | 1.11 |
| 1.14 | Unit tests for all Phase 1 components + hooks | `*.test.tsx` / `*.test.ts` | 1.1–1.13 |
| 1.15 | Integration test: Terra deposit flow | `useTerraDeposit.integration.test.ts` | 1.7 |
| 1.16 | Integration test: EVM deposit flow (update existing) | `useBridgeDeposit.integration.test.ts` | 1.6 |

### Phase 2: Hash Verification & Matching Page (Sprint F1-C)

**Duration:** ~4 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 2.1 | `hashVerification.ts` service (compute hash, query events) | `src/services/hashVerification.ts` | 0.5 |
| 2.2 | `useTransferLookup` hook (fetch source/dest events) | `src/hooks/useTransferLookup.ts` | 2.1 |
| 2.3 | `useHashVerification` hook (compare source ↔ dest) | `src/hooks/useHashVerification.ts` | 2.2 |
| 2.4 | `HashSearchBar` component | `src/components/verify/HashSearchBar.tsx` | 0.6 |
| 2.5 | `SourceHashCard` + `DestHashCard` | `src/components/verify/SourceHashCard.tsx`, `DestHashCard.tsx` | 0.3 |
| 2.6 | `HashFieldsTable` (diff-style comparison) | `src/components/verify/HashFieldsTable.tsx` | 0.3 |
| 2.7 | `StatusBadge`, `FraudAlert`, `CancelInfo`, `ComparisonIndicator` | `src/components/verify/*.tsx` | 0.3 |
| 2.8 | `HashComparisonPanel` composition | `src/components/verify/HashComparisonPanel.tsx` | 2.5–2.7 |
| 2.9 | `RecentVerifications` (localStorage-backed) | `src/components/verify/RecentVerifications.tsx` | — |
| 2.10 | `HashVerificationPage` assembly | `src/pages/HashVerificationPage.tsx` | 2.3, 2.4, 2.8, 2.9 |
| 2.11 | Unit tests: hash computation parity (must match Solidity/Rust) | `hashVerification.test.ts` | 2.1 |
| 2.12 | Unit tests: verify components | `*.test.tsx` | 2.4–2.9 |
| 2.13 | Integration test: end-to-end hash verification | `useHashVerification.integration.test.ts` | 2.3 |

### Phase 3: System Settings Page (Sprint F1-D)

**Duration:** ~3 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 3.1 | `useChainStatus` hook (ping endpoints) | `src/hooks/useChainStatus.ts` | 0.5 |
| 3.2 | `useBridgeSettings` hook (query on-chain config) | `src/hooks/useBridgeSettings.ts` | 0.5 |
| 3.3 | `useTokenRegistry` hook (query TokenRegistry) | `src/hooks/useTokenRegistry.ts` | 0.5 |
| 3.4 | `ConnectionStatus` component | `src/components/settings/ConnectionStatus.tsx` | 0.3 |
| 3.5 | `ChainCard` + `ChainsPanel` | `src/components/settings/ChainCard.tsx`, `ChainsPanel.tsx` | 3.1, 3.4 |
| 3.6 | `TokenCard` + `TokensPanel` | `src/components/settings/TokenCard.tsx`, `TokensPanel.tsx` | 3.3 |
| 3.7 | `BridgeConfigPanel` | `src/components/settings/BridgeConfigPanel.tsx` | 3.2 |
| 3.8 | `SettingsPage` assembly | `src/pages/SettingsPage.tsx` | 3.5–3.7 |
| 3.9 | Unit tests: settings components + hooks | `*.test.tsx` / `*.test.ts` | 3.1–3.8 |
| 3.10 | Integration test: chain status + bridge config | `*.integration.test.ts` | 3.1, 3.2 |

### Phase 4: Polish & Cleanup (Sprint F1-E)

**Duration:** ~2 days

| # | Task | Description |
|---|------|-------------|
| 4.1 | Remove deprecated files (`BridgeForm.tsx`, `services/wallet.ts` if fully migrated) |
| 4.2 | Update `docs/frontend.md` with new structure, route map, component list |
| 4.3 | Update `vitest.config.ts` coverage thresholds for new directories |
| 4.4 | Verify all files are ≤900 LOC; split any that grew past the limit |
| 4.5 | Full test run (`npm run test:run`), fix any failures |
| 4.6 | Build verification (`npm run build`), check bundle sizes |
| 4.7 | Accessibility audit: keyboard nav on modals, ARIA labels on status badges |
| 4.8 | Verify Render deployment with SPA fallback (all routes serve `index.html`) |

---

## 9. Dependency Order

```
Phase 0 (Foundation)
  ├── 0.1 Router ──────────────────────────────────────┐
  ├── 0.3 UI Primitives ──────────────────────────────┐│
  ├── 0.4 Split services/wallet.ts ────────────────┐  ││
  ├── 0.5 Shared types ──────────────────────────┐ │  ││
  ├── 0.6 Validation utils ───────────────────┐  │ │  ││
  └── 0.7 Test helpers ────────────────────┐  │  │ │  ││
                                           │  │  │ │  ││
Phase 1 (Transfers)                        │  │  │ │  ││
  ├── 1.1 EIP-6963 wagmi ─────────────────┤  │  │ │  ││
  ├── 1.2 EvmWalletModal ──────── (1.1, 0.3)─┤  │ │  ││
  ├── 1.3 TerraWallet refactor ── (0.3, 0.4)──┤  │ │  ││
  ├── 1.5 Transfer sub-components (0.5, 0.6)───┤  │ │  ││
  ├── 1.7 useTerraDeposit ─────── (0.4) ───────┤  │ │  ││
  ├── 1.8 transfer store ──────── (0.5) ────────┤  │ │  ││
  └── 1.11 TransferPage ──────── (1.2-1.10) ────┘  │ │  ││
                                                    │ │  ││
Phase 2 (Hash Verification)                         │ │  ││
  ├── 2.1 hashVerification.ts ── (0.5) ─────────────┤ │  ││
  ├── 2.3 useHashVerification ── (2.1, 2.2) ─────────┤ │  ││
  └── 2.10 HashVerificationPage  (2.3-2.9) ───────────┘ │  ││
                                                         │  ││
Phase 3 (Settings)                                       │  ││
  ├── 3.1 useChainStatus ─────── (0.5) ──────────────────┤  ││
  ├── 3.2 useBridgeSettings ──── (0.5) ───────────────────┤  ││
  └── 3.8 SettingsPage ───────── (3.1-3.7) ────────────────┘  ││
                                                               ││
Phase 4 (Polish) ──────────────────────────────────── (all) ───┘│
                                                                │
All phases depend on Phase 0 ───────────────────────────────────┘
```

**Critical path:** Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4

Phases 2 and 3 have no inter-dependencies and **can run in parallel** after Phase 1 completes (Phase 1 validates the routing + wallet infrastructure that 2 and 3 depend on).

---

## 10. Risk & Resolved Decisions

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| EIP-6963 provider discovery may not work on all browsers | Users can't connect some wallets | Keep `injected()` fallback + WalletConnect as escape hatch |
| Hash verification requires querying events from multiple chains | Slow UX, rate limits | Cache aggressively with React Query; show partial results immediately |
| `services/wallet.ts` split may break existing integration tests | CI failures | Run full test suite after split; keep re-export barrel for backward compat |
| Bundle size increase from new pages | Slower initial load | Lazy-load all pages via `React.lazy()` + `Suspense`; keep current chunk strategy |
| Terra LCD endpoints may not expose all transfer events | Hash verification gaps | Support manual hash input as fallback; document which events are queryable |

### Decisions (Resolved)

1. **Operator API:** No operator REST API. Hash verification must use RPC/LCD directly — on-chain data is the source of truth. `useTransferLookup` and `useHashVerification` query chains only.

2. **Admin edit for Settings:** Deferred to a future phase. Settings page is **read-only** — no edit buttons or forms.

3. **Transfer history backend:** **localStorage only.** The dapp must work without any backend. No operator DB, indexer, or The Graph.

4. **Supported chains:** **opBNB, BSC, ETH, Terra Classic.** All four must appear in `chains.ts`, `constants.ts`, and wagmi config. Update `NETWORKS` for each environment (local/testnet/mainnet) accordingly.

5. **Routing:** **BrowserRouter.** The Render deployment `render.yaml` will need a rewrite to serve the SPA (fallback to `index.html` for client-side routes).

---

## Appendix A: New Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `react-router-dom` | `^6.x` | Client-side routing |

No other new runtime dependencies are required. All EIP-6963 support comes from wagmi 2's built-in `mipd` integration.

## Appendix B: Files Created / Modified Summary

| Action | Count | Category |
|--------|-------|----------|
| New pages | 4 | `src/pages/` |
| New components | ~35 | `src/components/{wallet,transfer,verify,settings,ui}/` |
| New hooks | 8 | `src/hooks/` |
| New services | 6 | `src/services/terra/*`, `src/services/hashVerification.ts` |
| New stores | 1 | `src/stores/transfer.ts` |
| New types | 4 | `src/types/` |
| New utils | 1 | `src/utils/validation.ts` |
| New test helpers | 1 | `src/test/helpers.ts` |
| Updated files | 5 | `main.tsx`, `App.tsx`, `lib/wagmi.ts`, `ConnectWallet.tsx`, `vitest.config.ts` |
| Deleted files | 1 | `BridgeForm.tsx` (replaced) |
| **New test files** | ~30 | Co-located `*.test.tsx` / `*.test.ts` |
| **Total new files** | **~90** | |

## Appendix C: Estimated LOC Per File (Audit Checklist)

All files are designed to stay within the 900 LOC limit. Expected ranges:

| File Type | Target LOC | Max LOC |
|-----------|-----------|---------|
| Page | 80–150 | 200 |
| Component | 40–200 | 350 |
| Hook | 80–250 | 400 |
| Service | 80–200 | 400 |
| Store | 60–120 | 200 |
| Type file | 20–60 | 100 |
| Util | 20–80 | 150 |
| Test file | 50–300 | 500 |
