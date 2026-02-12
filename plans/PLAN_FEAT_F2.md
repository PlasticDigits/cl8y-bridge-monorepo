# PLAN_FEAT_F2: Full Cross-Chain Hash Verification

**Created:** 2026-02-12
**Scope:** `packages/frontend/` (repo-wide paths used throughout)
**Status:** Draft — ready for interactive review
**Depends on:** PLAN_FEAT_F1 (Phase 0–2 complete)

---

## Table of Contents

1. [Problem Statement](#1-problem-statement)
2. [Current State Analysis](#2-current-state-analysis)
3. [Multi-Chain Configuration (Phase 0)](#3-multi-chain-configuration-phase-0)
4. [Multi-Chain Transfer Lookup (Phase 1)](#4-multi-chain-transfer-lookup-phase-1)
5. [Terra LCD Lookup Integration (Phase 2)](#5-terra-lcd-lookup-integration-phase-2)
6. [UI Updates for Multi-Chain Verification (Phase 3)](#6-ui-updates-for-multi-chain-verification-phase-3)
7. [Code Organization Constraints](#7-code-organization-constraints)
8. [Testing Plan](#8-testing-plan)
9. [Phase / Sprint Breakdown](#9-phase--sprint-breakdown)
10. [Dependency Order](#10-dependency-order)
11. [Risk & Open Questions](#11-risk--open-questions)

---

## 1. Problem Statement

### The Limitation

`useTransferLookup` (from F1 Phase 2) queries only **one** EVM bridge contract — the one on `DEFAULT_NETWORK`. It calls `getDeposit(hash)` and `getPendingWithdraw(hash)` on the same contract and returns whichever side exists.

This works in a narrow scenario where the default chain happens to be the source OR destination. But for a general cross-chain transfer:

| Transfer | Source Bridge | Dest Bridge | Current Lookup |
|----------|--------------|-------------|----------------|
| BSC → opBNB | BSC bridge has `DepositRecord` | opBNB bridge has `PendingWithdraw` | Only queries default (e.g. BSC) → finds deposit only |
| ETH → BSC | ETH bridge has `DepositRecord` | BSC bridge has `PendingWithdraw` | If default is BSC → finds withdraw only |
| Terra → BSC | Terra contract has deposit record | BSC bridge has `PendingWithdraw` | Only queries EVM → misses Terra deposit |
| BSC → Terra | BSC bridge has `DepositRecord` | Terra contract has `PendingWithdraw` | Finds deposit only, misses Terra withdraw |

### What Full Verification Requires

For any transfer hash `H`:

1. **Identify** which chain pair the transfer belongs to (source chain + dest chain).
2. **Query the source chain** bridge for `getDeposit(H)` (EVM) or `deposit_hash` query (Terra).
3. **Query the destination chain** bridge for `getPendingWithdraw(H)` (EVM) or `pending_withdraw` query (Terra).
4. **Compare** the 7 hash fields from both sides to detect data consistency or fraud.
5. **Display** unified results with both sides populated.

### Goal

Replace the single-chain lookup with a multi-chain orchestrator that:
- Queries **all configured bridge contracts** (multiple EVM chains + Terra) in parallel.
- Resolves source/dest chain pairs from the data itself (the `srcChain`/`destChain` bytes4 fields in deposit/withdraw records tell us which chains are involved).
- Falls back gracefully when a chain is unreachable.
- Shows partial results immediately (e.g., source found, dest still loading).

---

## 2. Current State Analysis

### Files Being Replaced or Extended

| File | Current LOC | Role | Change Needed |
|------|-------------|------|---------------|
| `src/hooks/useTransferLookup.ts` | 222 | Single-chain EVM lookup | Replace with multi-chain orchestrator |
| `src/hooks/useHashVerification.ts` | 116 | Orchestrates lookup + comparison | Extend for multi-chain state |
| `src/services/hashVerification.ts` | 126 | Hash computation (pure functions) | Add Terra address↔bytes32 encoding |
| `src/utils/constants.ts` | 156 | Network config (single EVM per tier) | Extend with per-chain bridge addresses + RPCs |
| `src/lib/chains.ts` | 118 | Chain registry | Add `bridgeAddress` to `ChainInfo` |
| `src/types/chain.ts` | 14 | `ChainInfo` type | Add `bridgeAddress?` field |
| `src/hooks/useContract.ts` | 125 | Terra LCD helpers | Extract generic LCD fetch for reuse |
| `src/components/verify/HashComparisonPanel.tsx` | 80 | Display panel | Update for per-chain loading state |
| `src/components/verify/SourceHashCard.tsx` | 50 | Source details | Add chain name resolution from bytes4 |
| `src/components/verify/DestHashCard.tsx` | 72 | Dest details | Add chain name resolution from bytes4 |
| `src/components/verify/HashFieldsTable.tsx` | 74 | Diff table | Already supports null sides (no change) |
| `src/pages/HashVerificationPage.tsx` | 48 | Page shell | Add chain selector for manual override |

### Contract Query Interfaces

**EVM Bridge** (`IBridge.sol`):

| Function | Returns | Use |
|----------|---------|-----|
| `getDeposit(bytes32 hash)` | `DepositRecord` (8 fields) | Source chain deposit lookup |
| `getPendingWithdraw(bytes32 hash)` | `PendingWithdraw` (15 fields) | Dest chain withdraw lookup |
| `getThisChainId()` | `bytes4` | Confirm which chain we're querying |

**Terra Bridge** (CosmWasm):

| Query | Returns | Use |
|-------|---------|-----|
| `{ deposit_hash: { deposit_hash: "<base64>" } }` | `DepositInfoResponse` (8 fields) | Source chain deposit lookup |
| `{ pending_withdraw: { withdraw_hash: "<base64>" } }` | `PendingWithdrawResponse` (16 fields) | Dest chain withdraw lookup |
| `{ this_chain_id: {} }` | `ThisChainIdResponse` (4-byte base64) | Confirm chain identity |

### Chain ID Mapping

Bridges use `bytes4` chain IDs assigned during registration. The frontend needs a mapping from bytes4 → chain metadata (RPC URL, bridge address, type). This mapping is currently implicit:

| Chain | Numeric ID | bytes4 (hex) | Type |
|-------|-----------|--------------|------|
| Ethereum | 1 | `0x00000001` | EVM |
| BSC | 56 | `0x00000038` | EVM |
| opBNB | 204 | `0x000000cc` | EVM |
| Terra Classic | TBD (assigned at registration) | Varies | Cosmos |
| Anvil (local) | 31337 | `0x00007a69` | EVM |

**Note:** Terra's bytes4 chain ID is not its numeric chain ID — it is assigned by the bridge's `registerChain` call. The frontend must discover it by calling `getThisChainId()` (EVM) or `{ this_chain_id: {} }` (Terra) on each bridge, or it must be configured explicitly.

---

## 3. Multi-Chain Configuration (Phase 0)

### 3a. Extend Chain Registry

**Why:** Currently `NETWORKS` has a single `evm` object per tier. Multi-chain verification needs per-chain RPC URLs and bridge contract addresses for every supported chain.

**New config shape:**

```typescript
// src/utils/constants.ts — new BRIDGE_CHAINS addition

export interface BridgeChainConfig {
  chainId: number | string       // EVM numeric or Cosmos string
  type: 'evm' | 'cosmos'
  name: string
  rpcUrl: string                 // EVM RPC or Cosmos RPC
  lcdUrl?: string                // Cosmos LCD (if cosmos type)
  lcdFallbacks?: string[]        // LCD fallbacks
  bridgeAddress: string          // Bridge contract address
  bytes4ChainId?: string         // Hex string (e.g. "0x00000001") — discovered or configured
}

export const BRIDGE_CHAINS: Record<string, BridgeChainConfig> = {
  local: {
    anvil: {
      chainId: 31337,
      type: 'evm',
      name: 'Anvil',
      rpcUrl: 'http://localhost:8545',
      bridgeAddress: import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
    },
    localterra: {
      chainId: 'localterra',
      type: 'cosmos',
      name: 'LocalTerra',
      rpcUrl: 'http://localhost:26657',
      lcdUrl: 'http://localhost:1317',
      lcdFallbacks: ['http://localhost:1317'],
      bridgeAddress: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
    },
  },
  // testnet, mainnet variants with all 4 chains...
}
```

**New/Updated Files:**

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/types/chain.ts` | Update | ~30 | Add `bridgeAddress?: string`, `lcdUrl?: string` to `ChainInfo` |
| `src/utils/bridgeChains.ts` | New | ~120 | `BRIDGE_CHAINS` config, `getBridgeChainByBytes4()`, `getAllBridgeChains()` |
| `src/utils/constants.ts` | Update | ~165 | Add env vars for per-chain bridge addresses and RPCs |

### 3b. Chain ID Discovery Service

The bridge's `getThisChainId()` returns the bytes4 chain ID that was assigned during `registerChain`. We need a reliable mapping from bytes4 → chain config.

**Strategy — two-tier resolution:**

1. **Static map:** For well-known chains (ETH=`0x00000001`, BSC=`0x00000038`, etc.), a hardcoded bytes4 → chain ID lookup handles the common case with zero latency.
2. **Discovery (optional):** For unknown bytes4 values, query each configured bridge's `getThisChainId()` and cache the result. This handles custom/dynamic chain registrations.

```typescript
// src/services/chainDiscovery.ts

export async function discoverChainIds(
  chains: BridgeChainConfig[]
): Promise<Map<string, BridgeChainConfig>>
// Returns Map<bytes4Hex, BridgeChainConfig>
```

**New Files:**

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/services/chainDiscovery.ts` | New | ~150 | Discover bytes4 chain IDs from bridge contracts, build resolution map |
| `src/hooks/useChainRegistry.ts` | New | ~100 | React hook wrapping chainDiscovery with caching via React Query |

---

## 4. Multi-Chain Transfer Lookup (Phase 1)

### 4a. Architecture

The core change: instead of querying one bridge, query **all configured bridges** in parallel with `Promise.allSettled`, then assemble results.

```
User enters hash H
         │
         ▼
┌──────────────────────┐
│  useMultiChainLookup │
│                      │
│  For each bridge B   │──► createPublicClient(B.rpcUrl)
│  in BRIDGE_CHAINS:   │      ├── readContract(getDeposit(H))
│                      │      └── readContract(getPendingWithdraw(H))
│                      │
│  For each Terra LCD: │──► fetchLcd(deposit_hash query)
│                      │──► fetchLcd(pending_withdraw query)
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  Assemble Results    │
│                      │
│  source = first      │
│    non-empty deposit │
│                      │
│  dest = first        │
│    non-empty withdraw│
│                      │
│  Attach chain meta   │
│  to each result      │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ useHashVerification  │
│                      │
│ Compare source ↔ dest│
│ Compute expected hash│
│ Determine status     │
└──────────────────────┘
```

### 4b. New Lookup Hook

```typescript
// src/hooks/useMultiChainLookup.ts

export interface MultiChainLookupResult {
  source: DepositData | null
  sourceChain: BridgeChainConfig | null
  dest: PendingWithdrawData | null
  destChain: BridgeChainConfig | null
  queriedChains: string[]        // Which chains were queried
  failedChains: string[]         // Which chains failed (RPC error)
  loading: boolean
  error: string | null
}
```

**Key design decisions:**

1. **Parallel queries:** Use `Promise.allSettled` across all chains. A failing RPC should not block results from other chains.
2. **Early termination:** Once both source (deposit) and dest (withdraw) are found, remaining queries can be ignored via `AbortController`.
3. **Deduplication:** If the same hash yields a deposit on chain A AND a deposit on chain B (shouldn't happen but be defensive), prefer the one where `srcChain` bytes4 matches the queried chain's own ID.
4. **Partial results:** Emit intermediate state so the UI can show "Source found on BSC, querying opBNB for withdraw..." before all queries complete.

### 4c. EVM Client Factory

Currently `useTransferLookup` creates a viem `PublicClient` inline. Extract into a cacheable factory:

```typescript
// src/services/evmClient.ts

const clientCache = new Map<string, PublicClient>()

export function getEvmClient(chain: BridgeChainConfig): PublicClient
```

### 4d. New/Updated Files

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/services/evmClient.ts` | New | ~60 | Cached viem PublicClient factory per chain |
| `src/services/evmBridgeQueries.ts` | New | ~180 | `queryEvmDeposit(client, bridge, hash)`, `queryEvmWithdraw(client, bridge, hash)`, ABI definition |
| `src/hooks/useMultiChainLookup.ts` | New | ~250 | Orchestrates parallel lookups across all chains |
| `src/hooks/useTransferLookup.ts` | Deprecate | — | Keep for backward compat, delegate to `useMultiChainLookup` |
| `src/hooks/useHashVerification.ts` | Update | ~140 | Use `useMultiChainLookup`, expose `sourceChain`/`destChain` metadata |

### 4e. Bridge ABI Extraction

The bridge ABI is currently hardcoded inside `useTransferLookup.ts`. Extract to a shared constant:

```typescript
// src/services/evmBridgeQueries.ts

export const BRIDGE_VIEW_ABI = [
  // getDeposit, getPendingWithdraw, getThisChainId
] as const
```

---

## 5. Terra LCD Lookup Integration (Phase 2)

### 5a. Overview

Terra bridge stores deposits and pending withdrawals in CosmWasm contract state, queryable via LCD REST API. The query interface differs from EVM:

**EVM:** `readContract({ functionName: 'getDeposit', args: [hashBytes32] })`
**Terra LCD:** `GET /cosmwasm/wasm/v1/contract/{addr}/smart/{base64(JSON)}`

The JSON query payload for deposits:
```json
{ "deposit_hash": { "deposit_hash": "<base64 of 32-byte hash>" } }
```

For pending withdrawals:
```json
{ "pending_withdraw": { "withdraw_hash": "<base64 of 32-byte hash>" } }
```

### 5b. Response Mapping

The Terra contract returns different field names and formats than the EVM bridge. We need a normalizer.

**Terra `DepositInfoResponse`:**

| Field | Type | Notes |
|-------|------|-------|
| `deposit_hash` | Binary (base64) | The hash itself |
| `dest_chain_key` | Binary (base64) | 4-byte chain ID (note: named "key" but contains bytes4 ID in V2) |
| `src_account` | Binary (base64) | 32 bytes |
| `dest_token_address` | Binary (base64) | 32 bytes |
| `dest_account` | Binary (base64) | 32 bytes |
| `amount` | Uint128 (string) | Micro amount |
| `nonce` | u64 (number) | Deposit nonce |
| `deposited_at` | u64 (seconds) | Block timestamp |

**Terra `PendingWithdrawResponse`:**

| Field | Type | Notes |
|-------|------|-------|
| `exists` | bool | Whether the record was found |
| `src_chain` | Binary (base64) | 4-byte chain ID |
| `src_account` | Binary (base64) | 32 bytes |
| `dest_account` | Binary (base64) | 32 bytes |
| `token` | String | Terra token denom or CW20 address |
| `recipient` | String | Terra bech32 address |
| `amount` | Uint128 (string) | Micro amount |
| `nonce` | u64 | Source chain nonce |
| `src_decimals` | u8 | |
| `dest_decimals` | u8 | |
| `submitted_at` | u64 | |
| `approved_at` | u64 | |
| `approved` | bool | |
| `cancelled` | bool | |
| `executed` | bool | |
| `cancel_window_remaining` | u64 | Seconds remaining |

### 5c. Normalizer

```typescript
// src/services/terraBridgeQueries.ts

export function normalizeTerraPendingWithdraw(
  raw: TerraPendingWithdrawResponse,
  terraChainConfig: BridgeChainConfig
): PendingWithdrawData | null

export function normalizeTerraDeposit(
  raw: TerraDepositInfoResponse,
  terraChainConfig: BridgeChainConfig
): DepositData | null
```

The normalizer converts base64 Binary fields to `0x`-prefixed Hex, string amounts to `bigint`, and attaches the chain's numeric ID (or a sentinel for Cosmos chains).

### 5d. Hash-to-Hex Conversion

Terra returns `Binary` fields as base64. The frontend works with `0x`-prefixed hex. A utility is needed:

```typescript
// src/services/hashVerification.ts (addition)

export function base64ToHex(b64: string): Hex
export function hexToBase64(hex: Hex): string
export function terraAddressToBytes32(bech32Address: string): Hex
```

The `terraAddressToBytes32` function decodes the bech32 address to its raw 20-byte pubkey hash, then left-pads to 32 bytes — matching the Rust `address_to_bytes32` logic.

### 5e. New/Updated Files

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/services/terraBridgeQueries.ts` | New | ~200 | LCD queries for deposit + pending_withdraw, response normalizers |
| `src/services/hashVerification.ts` | Update | ~170 | Add `base64ToHex`, `hexToBase64`, `terraAddressToBytes32` |
| `src/hooks/useMultiChainLookup.ts` | Update | ~300 | Integrate Terra LCD queries alongside EVM |

### 5f. LCD Fallback Strategy

Reuse the existing `LCD_CONFIG` and fallback logic from `useContract.ts`. Extract the generic `fetchLcd` function into a shared utility:

```typescript
// src/services/lcdClient.ts (extracted from useContract.ts)

export async function fetchLcd<T>(
  lcdUrls: string[],
  path: string,
  timeout?: number
): Promise<T>
```

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/services/lcdClient.ts` | New | ~60 | Generic LCD fetch with fallbacks (extracted from `useContract.ts`) |
| `src/hooks/useContract.ts` | Update | ~100 | Import from `lcdClient.ts` instead of inline impl |

---

## 6. UI Updates for Multi-Chain Verification (Phase 3)

### 6a. Enhanced Verification State

The UI needs to show richer information now that both chains are queried:

```
HashVerificationPage
├── HashSearchBar                    # (unchanged)
├── ChainQueryStatus                 # NEW: Shows which chains were queried, per-chain status
│   ├── ChainQueryRow (per chain)    #   "BSC ✓ deposit found" / "opBNB ⏳ querying..." / "ETH ✗ RPC error"
├── HashComparisonPanel              # (updated props)
│   ├── SourceHashCard               #   Now shows source CHAIN NAME resolved from bytes4
│   ├── ComparisonIndicator          #   (unchanged)
│   └── DestHashCard                 #   Now shows dest CHAIN NAME resolved from bytes4
├── StatusPanel                      # (unchanged)
├── HashFieldsTable                  # (unchanged — already supports null sides)
└── RecentVerifications              # (unchanged)
```

### 6b. Chain Query Status Component

A new component showing the parallel query progress:

```typescript
// src/components/verify/ChainQueryStatus.tsx

interface ChainQueryStatusProps {
  queriedChains: string[]
  failedChains: string[]
  sourceChain: string | null   // Name of chain where deposit was found
  destChain: string | null     // Name of chain where withdraw was found
  loading: boolean
}
```

Visual:
```
┌─────────────────────────────────────────────────┐
│ Queried Chains                                  │
│                                                 │
│  BSC         ● Deposit found (source)           │
│  opBNB       ● Withdraw found (destination)     │
│  Ethereum    ○ No records                       │
│  Terra       ✗ LCD timeout (retrying...)        │
└─────────────────────────────────────────────────┘
```

### 6c. Updated HashComparisonPanel Props

```typescript
export interface HashComparisonPanelProps {
  source: DepositData | null
  sourceChainName: string | null     // NEW
  dest: PendingWithdrawData | null
  destChainName: string | null       // NEW
  status: HashStatus
  matches: boolean | null
  loading: boolean
  error: string | null
  queriedChains: string[]            // NEW
  failedChains: string[]             // NEW
}
```

### 6d. SourceHashCard / DestHashCard Updates

These cards currently use a local `chainIdToLabel(numericId)` function that maps EVM chain IDs. With multi-chain:

- The source chain name is now provided by the lookup hook (resolved from the chain where the deposit was found).
- The dest chain name is resolved from the `destChain` bytes4 in the deposit record (or vice versa for withdraw).
- For Terra-originated transfers, the chain name should say "Terra Classic" even though the bytes4 isn't a numeric EVM ID.

The `evmChainIdToLabel` utility in `utils/chainLabel.ts` should be extended to handle Cosmos chain IDs.

### 6e. New/Updated Files

| File | Action | LOC Est. | Description |
|------|--------|----------|-------------|
| `src/components/verify/ChainQueryStatus.tsx` | New | ~80 | Per-chain query progress indicator |
| `src/components/verify/ChainQueryStatus.test.tsx` | New | ~60 | Unit tests |
| `src/components/verify/HashComparisonPanel.tsx` | Update | ~90 | Accept + pass chain name props |
| `src/components/verify/SourceHashCard.tsx` | Update | ~55 | Accept `chainName` prop instead of local resolution |
| `src/components/verify/DestHashCard.tsx` | Update | ~75 | Accept `chainName` prop |
| `src/pages/HashVerificationPage.tsx` | Update | ~60 | Wire new props from `useHashVerification` |
| `src/utils/chainLabel.ts` | Update | ~35 | Handle both EVM numeric and Cosmos bytes4 chain IDs |
| `src/components/verify/index.ts` | Update | ~20 | Add `ChainQueryStatus` export |

---

## 7. Code Organization Constraints

### 7a. 900 LOC Per File Rule

All files remain under 900 LOC. Key split decisions:

| File | Expected LOC | Strategy if Growth |
|------|-------------|-------------------|
| `useMultiChainLookup.ts` | ~300 | Split EVM + Terra query logic into `evmBridgeQueries.ts` and `terraBridgeQueries.ts` services |
| `terraBridgeQueries.ts` | ~200 | Split normalizers into separate file if > 300 |
| `evmBridgeQueries.ts` | ~180 | Already scoped tightly |
| `bridgeChains.ts` | ~120 | Static config + helpers |
| `chainDiscovery.ts` | ~150 | Could merge into `bridgeChains.ts` if < 250 combined |

### 7b. Module Boundaries

```
src/services/
  ├── evmClient.ts           # Viem PublicClient cache/factory
  ├── evmBridgeQueries.ts    # EVM bridge getDeposit/getPendingWithdraw + ABI
  ├── terraBridgeQueries.ts  # Terra LCD deposit/withdraw queries + normalizers
  ├── lcdClient.ts           # Generic LCD fetch with fallbacks
  ├── chainDiscovery.ts      # bytes4 → chain config resolution
  └── hashVerification.ts    # Hash computation (existing, extended)

src/hooks/
  ├── useMultiChainLookup.ts # Parallel multi-chain lookup orchestrator
  ├── useHashVerification.ts # Comparison + status (updated)
  ├── useChainRegistry.ts    # Chain discovery with React Query caching
  └── useTransferLookup.ts   # Deprecated, delegates to useMultiChainLookup

src/utils/
  ├── bridgeChains.ts        # Per-chain bridge config, getBridgeChain helpers
  ├── chainLabel.ts          # bytes4/numeric → human label (updated)
  └── constants.ts           # Env vars for per-chain addresses (updated)
```

---

## 8. Testing Plan

### 8a. Testing Philosophy (Unchanged)

Per the project's existing philosophy from F1:
- **No mocks for blockchain.** All RPC/LCD calls use real infrastructure (Anvil, LocalTerra).
- **Mock only UI state** — connection status, form state for component rendering tests.
- **Pure functions** are tested in isolation (encoding, normalization, chain resolution).

### 8b. Unit Tests

| Area | Files | Coverage Target | What to Test |
|------|-------|----------------|--------------|
| **Hash encoding** | `hashVerification.test.ts` | ≥95% | `base64ToHex`, `hexToBase64`, `terraAddressToBytes32`, existing functions |
| **EVM queries** | `evmBridgeQueries.test.ts` | ≥85% | ABI shape, response mapping to `DepositData`/`PendingWithdrawData`, empty-record detection |
| **Terra queries** | `terraBridgeQueries.test.ts` | ≥85% | LCD URL construction, base64 encoding of query, response normalization, fallback on missing `exists` |
| **Chain config** | `bridgeChains.test.ts` | ≥90% | `getBridgeChainByBytes4`, `getAllBridgeChains`, unknown chain fallback |
| **Chain discovery** | `chainDiscovery.test.ts` | ≥80% | Static map hits, RPC discovery mock, caching |
| **LCD client** | `lcdClient.test.ts` | ≥85% | Fallback between endpoints, timeout handling |
| **EVM client** | `evmClient.test.ts` | ≥85% | Client caching, creation with custom chain |
| **Chain label** | `chainLabel.test.ts` | ≥95% | EVM numeric, Cosmos bytes4, unknown fallback |
| **UI components** | `ChainQueryStatus.test.tsx` | ≥80% | Loading state, per-chain indicators, error display |
| **Updated cards** | `SourceHashCard.test.tsx`, `DestHashCard.test.tsx` | ≥80% | Chain name from props, Terra data rendering |

### 8c. Integration Tests (Requires Infra)

| Test | File | Infra Required | What to Test |
|------|------|----------------|--------------|
| Multi-chain EVM lookup | `useMultiChainLookup.integration.test.ts` | Anvil (2 instances or 1 with deposits+withdraws) | Query deposit on chain A, withdraw on chain B |
| Terra LCD lookup | `terraBridgeQueries.integration.test.ts` | LocalTerra + bridge deployed | Query deposit by hash, pending withdraw by hash |
| Chain discovery | `chainDiscovery.integration.test.ts` | Anvil + LocalTerra | Discover bytes4 IDs from running bridges |
| End-to-end verification | `useHashVerification.integration.test.ts` | Anvil + LocalTerra | Full flow: deposit on source → submit withdraw on dest → verify hash matches |

Integration tests use the `.integration.test.ts` suffix and are skipped when `SKIP_INTEGRATION=true`.

### 8d. Coverage Expectations

| Directory | Statement Coverage |
|-----------|-------------------|
| `src/services/evmBridgeQueries.ts` | ≥85% |
| `src/services/terraBridgeQueries.ts` | ≥85% |
| `src/services/chainDiscovery.ts` | ≥80% |
| `src/services/lcdClient.ts` | ≥85% |
| `src/services/evmClient.ts` | ≥85% |
| `src/services/hashVerification.ts` | ≥95% |
| `src/hooks/useMultiChainLookup.ts` | ≥80% |
| `src/utils/bridgeChains.ts` | ≥90% |
| `src/components/verify/ChainQueryStatus.tsx` | ≥80% |

---

## 9. Phase / Sprint Breakdown

### Phase 0: Multi-Chain Configuration (Sprint F2-A)

**Duration:** ~2 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 0.1 | Define `BridgeChainConfig` type, extend `ChainInfo` | `src/types/chain.ts` | F1 0.5 |
| 0.2 | Create `bridgeChains.ts` with per-network chain configs | `src/utils/bridgeChains.ts` | 0.1 |
| 0.3 | Add env vars for per-chain bridge addresses | `src/utils/constants.ts` | 0.2 |
| 0.4 | Create `chainDiscovery.ts` (static map + RPC discovery) | `src/services/chainDiscovery.ts` | 0.2 |
| 0.5 | Create `useChainRegistry.ts` hook | `src/hooks/useChainRegistry.ts` | 0.4 |
| 0.6 | Update `chainLabel.ts` for Cosmos chain IDs | `src/utils/chainLabel.ts` | 0.2 |
| 0.7 | Unit tests: bridgeChains, chainDiscovery, chainLabel | `*.test.ts` | 0.2, 0.4, 0.6 |

### Phase 1: Multi-Chain EVM Lookup (Sprint F2-B)

**Duration:** ~3 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 1.1 | Extract bridge ABI + EVM query functions | `src/services/evmBridgeQueries.ts` | F1 2.1 |
| 1.2 | Create cached EVM client factory | `src/services/evmClient.ts` | — |
| 1.3 | Create `useMultiChainLookup.ts` (EVM-only first) | `src/hooks/useMultiChainLookup.ts` | 0.2, 1.1, 1.2 |
| 1.4 | Update `useHashVerification.ts` to use multi-chain lookup | `src/hooks/useHashVerification.ts` | 1.3 |
| 1.5 | Deprecate `useTransferLookup.ts` (thin wrapper) | `src/hooks/useTransferLookup.ts` | 1.3 |
| 1.6 | Unit tests: evmBridgeQueries, evmClient | `*.test.ts` | 1.1, 1.2 |
| 1.7 | Unit tests: useMultiChainLookup (mocked clients) | `*.test.ts` | 1.3 |

### Phase 2: Terra LCD Lookup (Sprint F2-C)

**Duration:** ~3 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 2.1 | Extract generic LCD fetch to `lcdClient.ts` | `src/services/lcdClient.ts` | — |
| 2.2 | Update `useContract.ts` to import from `lcdClient.ts` | `src/hooks/useContract.ts` | 2.1 |
| 2.3 | Create `terraBridgeQueries.ts` (query + normalize) | `src/services/terraBridgeQueries.ts` | 2.1 |
| 2.4 | Add `base64ToHex`, `hexToBase64`, `terraAddressToBytes32` to hashVerification | `src/services/hashVerification.ts` | — |
| 2.5 | Integrate Terra queries into `useMultiChainLookup.ts` | `src/hooks/useMultiChainLookup.ts` | 2.3, 1.3 |
| 2.6 | Unit tests: lcdClient, terraBridgeQueries, new hash utils | `*.test.ts` | 2.1, 2.3, 2.4 |
| 2.7 | Integration test: Terra LCD deposit/withdraw lookup | `*.integration.test.ts` | 2.3 |

### Phase 3: UI Updates (Sprint F2-D)

**Duration:** ~2 days
**Deliverables:**

| # | Task | Files | Depends On |
|---|------|-------|------------|
| 3.1 | Create `ChainQueryStatus` component | `src/components/verify/ChainQueryStatus.tsx` | 0.2 |
| 3.2 | Update `HashComparisonPanel` for chain name props | `src/components/verify/HashComparisonPanel.tsx` | 1.4 |
| 3.3 | Update `SourceHashCard` to accept chain name prop | `src/components/verify/SourceHashCard.tsx` | 1.4 |
| 3.4 | Update `DestHashCard` to accept chain name prop | `src/components/verify/DestHashCard.tsx` | 1.4 |
| 3.5 | Update `HashVerificationPage` to wire new state | `src/pages/HashVerificationPage.tsx` | 3.1–3.4 |
| 3.6 | Update barrel exports | `src/components/verify/index.ts` | 3.1 |
| 3.7 | Unit tests: ChainQueryStatus, updated cards | `*.test.tsx` | 3.1–3.4 |
| 3.8 | Integration test: end-to-end multi-chain verification | `*.integration.test.ts` | 3.5 |

### Phase 4: Polish & Migration (Sprint F2-E)

**Duration:** ~1 day

| # | Task | Description |
|---|------|-------------|
| 4.1 | Remove old inline ABI from `useTransferLookup.ts` |
| 4.2 | Verify all files are ≤900 LOC |
| 4.3 | Full test run (`npm run test:run`), fix any failures |
| 4.4 | Build verification (`npm run build`), check bundle sizes |
| 4.5 | Update `RecentVerifications` to store which chains were matched |
| 4.6 | Add ARIA labels to `ChainQueryStatus` indicators |
| 4.7 | Document multi-chain config in README or `docs/frontend.md` |

---

## 10. Dependency Order

```
Phase 0 (Config)
  ├── 0.1 BridgeChainConfig type ─────────────────────────────┐
  ├── 0.2 bridgeChains.ts ────────────────────────────────────┐│
  ├── 0.4 chainDiscovery.ts ──────────────── (0.2) ──────────┤│
  └── 0.6 chainLabel.ts update ──────────── (0.2) ───────────┤│
                                                              ││
Phase 1 (EVM Multi-Chain)                                     ││
  ├── 1.1 evmBridgeQueries.ts ──── (F1 2.1) ─────────────────┤│
  ├── 1.2 evmClient.ts ───────────────────────────────────────┤│
  ├── 1.3 useMultiChainLookup ──── (0.2, 1.1, 1.2) ──────────┤│
  └── 1.4 useHashVerification ──── (1.3) ─────────────────────┤│
                                                              ││
Phase 2 (Terra LCD)                                           ││
  ├── 2.1 lcdClient.ts ───────────────────────────────────────┤│
  ├── 2.3 terraBridgeQueries.ts ── (2.1) ─────────────────────┤│
  ├── 2.4 hashVerification.ts ─── (additions) ────────────────┤│
  └── 2.5 useMultiChainLookup ──── (2.3, 1.3) ────────────────┤│
                                                               ││
Phase 3 (UI)                                                   ││
  ├── 3.1 ChainQueryStatus ────── (0.2) ──────────────────────┤│
  ├── 3.2 HashComparisonPanel ──── (1.4) ──────────────────────┤│
  └── 3.5 HashVerificationPage ── (3.1–3.4) ──────────────────┘│
                                                                │
Phase 4 (Polish) ────────────────────────────────── (all) ──────┘
```

**Critical path:** Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4

Phases 1 and 2 share the `useMultiChainLookup` hook. Phase 1 delivers EVM-only multi-chain. Phase 2 adds Terra support to the same hook. Phase 3 updates UI and can start once Phase 1 is complete (Terra UI additions merge in when Phase 2 finishes).

---

## 11. Risk & Open Questions

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Public EVM RPCs rate-limit parallel queries (4+ chains × 2 calls each = 8+ RPC calls) | Lookup fails or is very slow | Use `Promise.allSettled` not `Promise.all`; show partial results; add configurable RPC URLs via env vars; respect rate limits with per-chain throttle |
| Terra LCD endpoints are unreliable or slow | Terra side never resolves | LCD fallback array (3 endpoints); configurable timeout; show "Terra unavailable" gracefully |
| bytes4 chain IDs differ between environments (local vs testnet vs mainnet) | Chain resolution fails, deposit/withdraw attributed to wrong chain | Static map per environment; verify at startup via `getThisChainId()` calls; warn in console if mismatch |
| Terra bridge deposit query returns base64 Binary fields that differ from EVM bytes32 encoding | Hash comparison always fails for Terra-involved transfers | Thorough unit tests comparing known Solidity/Rust test vectors; `base64ToHex` round-trip tests |
| Bundle size increase from multiple viem clients | Slower page load | viem clients share the same bundle; only `http` transport is used (no websocket); lazy-load verification page |
| `useTransferLookup` is imported by existing code | Breaking change if removed | Keep as deprecated thin wrapper that delegates to `useMultiChainLookup`; export same interface |

### Open Questions

1. **Per-chain bridge addresses for mainnet:** Are bridge contract addresses for all 4 chains (ETH, BSC, opBNB, Terra) already deployed and known? If not, which subset is available?

2. **Terra bytes4 chain ID:** What bytes4 value was assigned to Terra Classic during bridge registration? Is it consistent across environments, or does it vary (local=X, testnet=Y, mainnet=Z)?

3. **EVM-to-EVM transfers:** Do all 4 EVM chains have independent bridge deployments, or do some share a bridge? (e.g., does opBNB use the same bridge as BSC?)

4. **Rate limiting:** Should the frontend respect a global RPC call budget, or is it acceptable to fire 8+ parallel calls per verification lookup?

5. **Caching strategy:** Should multi-chain lookup results be cached with React Query (with TTL), or is each lookup fresh? For frequently-checked hashes (e.g., operator monitoring), caching would reduce RPC load significantly.

---

## Appendix A: New Dependencies

No new runtime dependencies required. All functionality uses existing packages:
- `viem` (EVM client, ABI encoding)
- `@tanstack/react-query` (caching for chain discovery)
- `fetch` (Terra LCD queries)

## Appendix B: Files Created / Modified Summary

| Action | Count | Category |
|--------|-------|----------|
| New services | 5 | `src/services/{evmClient,evmBridgeQueries,terraBridgeQueries,lcdClient,chainDiscovery}.ts` |
| New hooks | 2 | `src/hooks/{useMultiChainLookup,useChainRegistry}.ts` |
| New utils | 1 | `src/utils/bridgeChains.ts` |
| New components | 1 | `src/components/verify/ChainQueryStatus.tsx` |
| Updated files | 10 | `useHashVerification.ts`, `useTransferLookup.ts`, `useContract.ts`, `hashVerification.ts`, `constants.ts`, `chainLabel.ts`, `types/chain.ts`, `HashComparisonPanel.tsx`, `SourceHashCard.tsx`, `DestHashCard.tsx`, `HashVerificationPage.tsx`, `verify/index.ts` |
| New test files | ~12 | Co-located `*.test.ts` / `*.test.tsx` / `*.integration.test.ts` |
| **Total new files** | **~20** | |

## Appendix C: Estimated LOC Per File

| File | Est. LOC | Max LOC |
|------|----------|---------|
| `src/services/evmClient.ts` | 60 | 100 |
| `src/services/evmBridgeQueries.ts` | 180 | 300 |
| `src/services/terraBridgeQueries.ts` | 200 | 350 |
| `src/services/lcdClient.ts` | 60 | 100 |
| `src/services/chainDiscovery.ts` | 150 | 250 |
| `src/hooks/useMultiChainLookup.ts` | 300 | 400 |
| `src/hooks/useChainRegistry.ts` | 100 | 150 |
| `src/utils/bridgeChains.ts` | 120 | 200 |
| `src/components/verify/ChainQueryStatus.tsx` | 80 | 120 |
| `src/services/hashVerification.ts` (updated) | 170 | 250 |
| `src/hooks/useHashVerification.ts` (updated) | 140 | 200 |
