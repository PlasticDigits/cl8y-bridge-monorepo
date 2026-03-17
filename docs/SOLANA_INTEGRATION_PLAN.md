# Solana Integration Plan

**Created:** 2026-03-17
**Scope:** All packages — contracts, operator, canceler, multichain-rs, frontend, e2e
**Status:** Draft — ready for review

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State — What Exists Today](#2-current-state--what-exists-today)
3. [Contract Reusability Assessment](#3-contract-reusability-assessment)
4. [Solana Programs (Phase 1)](#4-solana-programs-phase-1)
5. [Address Codec Adaptation (Phase 2)](#5-address-codec-adaptation-phase-2)
6. [Operator / Indexer (Phase 3)](#6-operator--indexer-phase-3)
7. [Canceler (Phase 4)](#7-canceler-phase-4)
8. [Frontend (Phase 5)](#8-frontend-phase-5)
9. [E2E Tests & Local Dev (Phase 6)](#9-e2e-tests--local-dev-phase-6)
10. [Chain Registration & Deployment (Phase 7)](#10-chain-registration--deployment-phase-7)
11. [Architecture Diagrams](#11-architecture-diagrams)
12. [Dependency Order](#12-dependency-order)
13. [Risks & Open Questions](#13-risks--open-questions)
14. [Appendix A: Solana vs Existing Chains](#appendix-a-solana-vs-existing-chains)

---

## 1. Executive Summary

This plan covers adding Solana as a third chain type to CL8Y Bridge, enabling transfers between Solana ↔ EVM and Solana ↔ Terra Classic.

**Key conclusions from analysis:**

- Terra Classic and EVM contracts **cannot be reused** — Solana requires new programs written in Rust/Anchor
- The **protocol design** (deposit → approve → delay → execute, 7-field hash, watchtower cancelers) is fully reusable
- `CHAIN_TYPE_SOLANA = 3` is already reserved in `AddressCodecLib.sol`, `address_codec.rs`, and `multichain-rs`
- `@solana/kit` v5.5, `@solana-program/token`, and `@solana-program/system` are already installed in the frontend
- The operator/canceler follow a clean EVM/Terra split pattern that extends naturally to a third chain type
- The `UniversalAddress` format uses 20-byte raw addresses — Solana's 32-byte pubkeys require adaptation

**Scope of work:**

| Area | Effort | New Code |
|------|--------|----------|
| Solana programs + tests | Large | `packages/contracts-solana/` |
| Address codec (all codebases) | Medium | Modify `multichain-rs`, `AddressCodecLib.sol`, Terra `address_codec.rs` |
| Operator watcher + writer | Medium | `watchers/solana.rs`, `writers/solana.rs` |
| Canceler | Medium | `watcher.rs` Solana arm |
| Frontend wallets + UI | Medium | Solana wallet modal, chain config, hooks |
| E2E tests + local dev | Medium | `solana-test-validator` in docker-compose |
| Chain registration + deployment | Small | Register on EVM/Terra contracts |

---

## 2. Current State — What Exists Today

### 2a. Solana Chain Type Reserved

All three address codec implementations already define `CHAIN_TYPE_SOLANA = 3`:

| Codebase | File | Constant |
|----------|------|----------|
| Solidity | `contracts-evm/src/lib/AddressCodecLib.sol:35` | `uint32 public constant CHAIN_TYPE_SOLANA = 3` |
| Rust (shared) | `multichain-rs/src/address_codec.rs:35` | `pub const CHAIN_TYPE_SOLANA: u32 = 3` |
| CosmWasm | `contracts-terraclassic/bridge/src/address_codec.rs:39` | `pub const CHAIN_TYPE_SOLANA: u32 = 3` |

The operator EVM watcher already maps chain type 3 to `"solana"` in its display logic (`packages/operator/src/watchers/evm.rs:673`).

### 2b. Frontend Solana Dependencies Already Installed

The frontend `package.json` already includes (via `package-lock.json`):

- `@solana/kit` v5.5.0
- `@solana-program/system` v0.10.0
- `@solana-program/token` v0.9.0
- `@solana/web3.js` v1.98.1

These are not yet used in any source files.

### 2c. Address Format Challenge

The current `UniversalAddress` format allocates 20 bytes for the raw address:

```text
| Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
```

Solana public keys are 32 bytes (Ed25519). This is the most significant cross-cutting change required — see [Phase 2](#5-address-codec-adaptation-phase-2).

---

## 3. Contract Reusability Assessment

### Can Terra Classic contracts run on Solana?

**No.** They are fundamentally incompatible:

| Aspect | Terra Classic (CosmWasm) | Solana |
|--------|--------------------------|--------|
| VM | CosmWasm (Wasm) | Sealevel (BPF/SBF) |
| State model | Contract-owned storage | Account-based (PDAs) |
| Token standard | CW20 | SPL Token |
| Address format | Bech32 (20-byte canonical) | Base58 (32-byte Ed25519 pubkey) |
| Entry point style | `execute`, `query`, `instantiate` | Instruction handlers |
| Language | Rust (cosmwasm-std) | Rust (solana-program or Anchor) |

While both compile Rust, the SDK, state model, and runtime are entirely different. The CosmWasm contract cannot be compiled for Solana's BPF target.

### Can EVM contracts run on Solana?

**No.** Solidity targets the EVM. Solana does not run EVM bytecode (Neon EVM exists but is a separate ecosystem and would not provide native SPL token integration).

### What IS Reusable

| Reusable Element | Location | How It Applies to Solana |
|------------------|----------|--------------------------|
| Protocol flow | All contracts | deposit → approve → delay → execute → cancel |
| 7-field hash | `HashLib.sol`, `hash.rs` | Same `keccak256(abi.encode(...))` — implement in Solana program |
| Chain ID system | `ChainRegistry.sol` | Register Solana with a `bytes4` chain ID |
| Address encoding rules | `AddressCodecLib.sol` | Extend for 32-byte Solana pubkeys |
| Fee model | Bridge contracts | Same fee deduction before hash computation |
| Nonce tracking | Bridge contracts | Same global outgoing nonce pattern |
| Watchtower pattern | Security model | Same approve/delay/cancel with cancelers |
| Operator logic | `multichain-rs` | Hash computation, address codec, shared types |

---

## 4. Solana Programs (Phase 1)

### 4a. Package Structure

```
packages/contracts-solana/
├── Anchor.toml
├── Cargo.toml
├── programs/
│   └── cl8y-bridge/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                 # Program entry, declare_id!
│           ├── instructions/
│           │   ├── mod.rs
│           │   ├── initialize.rs      # Admin: init bridge state
│           │   ├── deposit_native.rs  # User: deposit SOL
│           │   ├── deposit_spl.rs     # User: deposit SPL token (lock or burn)
│           │   ├── withdraw_submit.rs # User: submit pending withdrawal
│           │   ├── withdraw_approve.rs# Operator: approve pending withdrawal
│           │   ├── withdraw_execute.rs# User: execute after delay
│           │   ├── withdraw_cancel.rs # Canceler: cancel fraudulent approval
│           │   ├── withdraw_reenable.rs# Admin: reenable cancelled withdrawal
│           │   ├── register_chain.rs  # Admin: register chain ID
│           │   ├── register_token.rs  # Admin: register token mapping
│           │   ├── set_config.rs      # Admin: update fee, delay, operator
│           │   └── add_canceler.rs    # Admin: add/remove canceler
│           ├── state/
│           │   ├── mod.rs
│           │   ├── bridge.rs          # Bridge config PDA
│           │   ├── deposit.rs         # Deposit record PDA
│           │   ├── pending_withdraw.rs# Pending withdrawal PDA
│           │   ├── chain_registry.rs  # Registered chains PDA
│           │   └── token_registry.rs  # Token mapping PDA
│           ├── hash.rs               # computeTransferHash — must match HashLib.sol
│           ├── address_codec.rs      # Solana ↔ bytes32 encoding
│           └── error.rs             # Custom error codes
├── tests/
│   ├── bridge.test.ts               # Anchor integration tests (TypeScript)
│   ├── hash_parity.test.ts          # Hash parity with EVM/Terra reference values
│   ├── deposit_withdraw.test.ts     # Full deposit → approve → execute flow
│   ├── cancel_flow.test.ts          # Cancel and reenable flows
│   └── helpers/
│       └── setup.ts                 # Test validator setup, airdrop, deploy
└── migrations/
    └── deploy.ts
```

### 4b. Program Design

#### State Accounts (PDAs)

| PDA | Seeds | Fields |
|-----|-------|--------|
| `BridgeConfig` | `["bridge"]` | `admin`, `operator`, `fee_bps`, `withdraw_delay`, `deposit_nonce`, `paused` |
| `DepositRecord` | `["deposit", nonce.to_le_bytes()]` | `transfer_hash`, `src_account`, `dest_chain`, `dest_account`, `token`, `amount`, `nonce`, `timestamp` |
| `PendingWithdraw` | `["withdraw", transfer_hash]` | `transfer_hash`, `src_chain`, `src_account`, `dest_account`, `token`, `amount`, `nonce`, `approved_at`, `cancelled` |
| `ChainEntry` | `["chain", chain_id]` | `chain_id: [u8; 4]`, `identifier: String` |
| `TokenMapping` | `["token", dest_chain, dest_token]` | `local_mint`, `dest_chain`, `dest_token`, `mode` (LockUnlock / MintBurn), `decimals` |
| `CancelerEntry` | `["canceler", pubkey]` | `pubkey`, `active` |

#### Instruction Flow

**Deposit (Solana → other chain):**

```
User calls deposit_native or deposit_spl:
  1. Transfer SOL/SPL to bridge PDA (lock) or burn (if mintable)
  2. Deduct fee → compute net amount
  3. Increment deposit_nonce
  4. Compute transfer_hash = keccak256(srcChain, destChain, srcAccount, destAccount, destToken, netAmount, nonce)
  5. Create DepositRecord PDA with hash and parameters
  6. Emit DepositEvent { transfer_hash, dest_chain, dest_account, token, amount, nonce, fee }
```

**Withdrawal (other chain → Solana):**

```
Step 1 — User calls withdraw_submit:
  1. Compute transfer_hash from provided parameters
  2. Create PendingWithdraw PDA (approved = false)

Step 2 — Operator calls withdraw_approve:
  1. Verify caller == bridge.operator
  2. Set approved = true, approved_at = Clock::get()

Step 3 — (5 min delay, cancelers verify)

Step 4 — User calls withdraw_execute:
  1. Verify approved == true, cancelled == false
  2. Verify Clock::get() >= approved_at + withdraw_delay
  3. Transfer SOL/SPL from bridge PDA to user (unlock) or mint to user
  4. Close PendingWithdraw PDA (reclaim rent)
```

**Cancel:**

```
Canceler calls withdraw_cancel:
  1. Verify caller is in CancelerEntry PDAs
  2. Set PendingWithdraw.cancelled = true
```

### 4c. Hash Parity

The Solana program must compute the same 7-field keccak256 hash as all other codebases. Implementation in `programs/cl8y-bridge/src/hash.rs`:

```rust
use solana_program::keccak;

pub fn compute_transfer_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    let mut buf = [0u8; 224]; // 7 x 32 bytes

    // srcChain: 4 bytes left-aligned in 32-byte slot
    buf[0..4].copy_from_slice(src_chain);

    // destChain: 4 bytes left-aligned in 32-byte slot
    buf[32..36].copy_from_slice(dest_chain);

    // srcAccount: 32 bytes
    buf[64..96].copy_from_slice(src_account);

    // destAccount: 32 bytes
    buf[96..128].copy_from_slice(dest_account);

    // token: 32 bytes (destination token)
    buf[128..160].copy_from_slice(token);

    // amount: uint256 big-endian (u128 in upper 16 bytes of slot)
    buf[176..192].copy_from_slice(&amount.to_be_bytes());

    // nonce: uint256 big-endian (u64 in upper 8 bytes of slot)
    buf[216..224].copy_from_slice(&nonce.to_be_bytes());

    keccak::hash(&buf).to_bytes()
}
```

**Critical**: This must pass parity tests against the hardcoded reference hashes used in `contracts-evm/test/HashLib.t.sol` and `multichain-rs/src/hash.rs` tests.

### 4d. SPL Token Integration

| Mode | Mechanism |
|------|-----------|
| **Lock/Unlock** | Transfer SPL tokens to/from a bridge-owned Associated Token Account (ATA) |
| **Mint/Burn** | Bridge PDA is mint authority for bridged SPL tokens; burns on deposit, mints on withdraw |
| **Native SOL** | Wrap to WSOL (native mint) or handle directly via system transfer |

### 4e. Testing

| Test Category | Framework | What It Validates |
|---------------|-----------|-------------------|
| Hash parity | Anchor + TypeScript | Same hashes as Solidity/Rust reference values |
| Deposit flow | Anchor bankrun | SOL + SPL deposit, nonce increment, event emission |
| Withdraw flow | Anchor bankrun | submit → approve → delay → execute lifecycle |
| Cancel flow | Anchor bankrun | Canceler cancels, user cannot execute, admin reenables |
| Access control | Anchor bankrun | Only operator can approve, only cancelers can cancel |
| Fee math | Anchor bankrun | Fee deduction, net amount in hash |
| Edge cases | Anchor bankrun | Double-deposit, replay, zero amount, wrong signer |

---

## 5. Address Codec Adaptation (Phase 2)

### 5a. The Problem

Solana pubkeys are 32 bytes. The current `UniversalAddress` format allocates only 20 bytes for the raw address:

```text
Current: | Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
```

This works for EVM (20-byte addresses) and Cosmos (20-byte canonical addresses from bech32). It does not fit Solana's 32-byte Ed25519 pubkeys.

### 5b. Options

| Option | Description | Impact |
|--------|-------------|--------|
| **A: Extend into reserved** | Use reserved 8 bytes for Solana, giving 28 bytes — still not enough | Insufficient |
| **B: Variable-length raw address** | Chain type determines whether raw is 20 or 32 bytes, pack 32-byte into address+reserved | Breaking change to all codebases |
| **C: Separate encoding path** | For the `bytes32` hash fields (srcAccount, destAccount, token), Solana uses full 32-byte pubkey directly; `UniversalAddress` remains 20-byte for EVM/Cosmos display/routing | Least disruptive |

**Recommended: Option C** — separate the hash encoding (which already uses `bytes32` slots) from the `UniversalAddress` struct used for display/routing.

### 5c. Design: Option C in Detail

The 7-field transfer hash already uses `bytes32` for all address fields. For Solana:

- **Hash computation**: `srcAccount` / `destAccount` / `token` are the raw 32-byte Solana pubkey, occupying the full `bytes32` slot
- **UniversalAddress (display/routing)**: Either expand the struct or use a `SolanaAddress` newtype that wraps `[u8; 32]` directly

This means the hash computation path doesn't need `UniversalAddress` at all — it works with raw `[u8; 32]` slices, which is already the case.

The routing/display path needs:
- EVM `ChainRegistry.sol` and Terra bridge must accept 32-byte Solana addresses in `destAccount` without left-pad truncation
- Frontend must display base58-encoded 32-byte addresses for Solana

### 5d. Changes Per Codebase

| Codebase | File | Change |
|----------|------|--------|
| `multichain-rs` | `address_codec.rs` | Add `SolanaAddress` newtype wrapping `[u8; 32]`; add `parse_solana_address(base58) -> [u8; 32]` and `encode_solana_address([u8; 32]) -> String`; add `fn to_hash_bytes(&self) -> [u8; 32]` for Solana that returns the full pubkey |
| `contracts-evm` | `AddressCodecLib.sol` | Add `encodeSolana(bytes32 pubkey) -> bytes32` that stores the full 32-byte key; update `isValidChainType` |
| `contracts-terraclassic` | `address_codec.rs` | Add `encode_solana` / `decode_solana` functions; ensure deposit/withdraw handlers accept 32-byte dest accounts for Solana chain type |
| `contracts-solana` | `address_codec.rs` | Native — pubkeys are already 32 bytes; encode EVM/Cosmos addresses as left-padded 32-byte for hashing |
| Frontend | New service | `services/solana/address.ts` — base58 ↔ bytes32 conversion |

### 5e. Hash Encoding Rules (Updated for Solana)

| Address Type | In Hash `bytes32` | Encoding |
|-------------|-------------------|----------|
| EVM (20-byte) | `0x000000000000000000000000{20 bytes}` | Left-pad with 12 zero bytes |
| Cosmos (20-byte bech32) | `0x000000000000000000000000{20 bytes}` | Bech32 decode → left-pad |
| Solana (32-byte pubkey) | `{32 bytes}` | Full pubkey, no padding |
| Native denom (Terra) | `keccak256(denom)` | Full 32-byte hash |
| SPL Mint (Solana) | `{32 bytes}` | Full mint pubkey |

---

## 6. Operator / Indexer (Phase 3)

### 6a. Current Architecture

The operator uses a watcher/writer pattern per chain type:

```
WatcherManager
├── EvmWatcher (per EVM chain) — polls eth_getLogs for Deposit events
├── TerraWatcher — polls LCD tx_search for deposit txs
└── [NEW] SolanaWatcher — polls Solana RPC for bridge instructions

WriterManager
├── EvmWriter (per EVM chain) — submits approveWithdraw txs
├── TerraWriter — submits ApproveWithdraw msgs
└── [NEW] SolanaWriter — submits approve_withdraw instructions
```

### 6b. New Files

| File | Purpose |
|------|---------|
| `packages/operator/src/config.rs` | Add `SolanaConfig` struct and env loading |
| `packages/operator/src/watchers/solana.rs` | `SolanaWatcher` — poll for deposit instructions |
| `packages/operator/src/writers/solana.rs` | `SolanaWriter` — submit approval instructions |
| `packages/operator/src/watchers/mod.rs` | Register Solana watcher in `WatcherManager` |
| `packages/operator/src/writers/mod.rs` | Register Solana writer in `WriterManager` |
| DB migration | `solana_deposits` and `solana_blocks` tables |

### 6c. SolanaWatcher Design

```rust
pub struct SolanaWatcher {
    rpc_client: RpcClient,
    program_id: Pubkey,
    db: PgPool,
    last_signature: Option<Signature>,
    poll_interval: Duration,
}

impl SolanaWatcher {
    pub async fn run(mut self) -> Result<()> {
        loop {
            // 1. getSignaturesForAddress(program_id, { until: last_signature })
            // 2. For each signature, getTransaction to parse instruction data
            // 3. Filter for deposit_native / deposit_spl instructions
            // 4. Extract: nonce, sender, dest_chain, dest_account, token, amount, fee
            // 5. Compute transfer_hash
            // 6. INSERT INTO solana_deposits (idempotent on nonce)
            // 7. Update last_signature
            tokio::time::sleep(self.poll_interval).await;
        }
    }
}
```

**RPC methods used:**

| Method | Purpose |
|--------|---------|
| `getSignaturesForAddress` | Find transactions involving the bridge program |
| `getTransaction` | Get full transaction data with parsed instructions |
| `getSlot` | Current slot height (for block tracking) |
| `getAccountInfo` | Read PDA state (deposit records, pending withdrawals) |

### 6d. SolanaWriter Design

```rust
pub struct SolanaWriter {
    rpc_client: RpcClient,
    program_id: Pubkey,
    keypair: Keypair, // Operator keypair
    db: PgPool,
}

impl SolanaWriter {
    pub async fn run(mut self) -> Result<()> {
        loop {
            // 1. Query DB for unprocessed deposits destined for Solana
            // 2. For each: build withdraw_approve instruction
            // 3. Submit transaction
            // 4. Mark deposit as processed in DB
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
```

### 6e. Configuration

```bash
# New env vars for operator
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_WS_URL=wss://api.mainnet-beta.solana.com
SOLANA_PROGRAM_ID=<bridge program pubkey>
SOLANA_KEYPAIR_PATH=/path/to/operator-keypair.json
SOLANA_POLL_INTERVAL_MS=2000
SOLANA_BYTES4_CHAIN_ID=0x00000005  # or whatever ID is chosen
```

### 6f. Database Migration

```sql
CREATE TABLE solana_deposits (
    id BIGSERIAL PRIMARY KEY,
    nonce BIGINT NOT NULL UNIQUE,
    transfer_hash BYTEA NOT NULL,
    src_account BYTEA NOT NULL,       -- 32-byte Solana pubkey
    dest_chain BYTEA NOT NULL,        -- 4-byte chain ID
    dest_account BYTEA NOT NULL,      -- 32-byte universal address
    token BYTEA NOT NULL,             -- 32-byte dest token
    amount NUMERIC NOT NULL,
    fee NUMERIC NOT NULL,
    slot BIGINT NOT NULL,
    signature TEXT NOT NULL,          -- Solana tx signature
    processed BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE solana_blocks (
    slot BIGINT PRIMARY KEY,
    block_hash TEXT NOT NULL,
    processed_at TIMESTAMPTZ DEFAULT NOW()
);
```

### 6g. Shared Library Updates (`multichain-rs`)

| File | Change |
|------|--------|
| `src/lib.rs` | Add `pub mod solana;` |
| `src/solana/mod.rs` | New: Solana RPC client wrapper |
| `src/solana/watcher.rs` | New: Shared Solana event parsing (used by operator + canceler) |
| `src/solana/types.rs` | New: Solana-specific types (instruction data, event structs) |
| `src/address_codec.rs` | Add Solana address parsing (base58 ↔ `[u8; 32]`) |
| `src/hash.rs` | No change (already chain-agnostic) |
| `Cargo.toml` | Add `solana-sdk`, `solana-client`, `bs58` dependencies |

---

## 7. Canceler (Phase 4)

### 7a. Current Architecture

The canceler watches for approvals on destination chains and verifies them against source chain deposits. It currently handles:

- **EVM → Terra**: Watch EVM `WithdrawApprove` events, verify against Terra deposits
- **Terra → EVM**: Watch Terra approval txs, verify against EVM deposits

### 7b. New Verification Routes

Adding Solana creates six new routes:

| Source | Destination | Canceler Watches | Canceler Verifies Against |
|--------|-------------|------------------|---------------------------|
| EVM | Solana | Solana `withdraw_approve` instructions | EVM `deposits[hash]` mapping |
| Solana | EVM | EVM `WithdrawApprove` events | Solana `DepositRecord` PDAs |
| Terra | Solana | Solana `withdraw_approve` instructions | Terra `deposit_hash` query |
| Solana | Terra | Terra approval txs | Solana `DepositRecord` PDAs |
| Solana | Solana | Solana `withdraw_approve` instructions | Solana `DepositRecord` PDAs |
| EVM | EVM | (existing) | (existing) |

### 7c. Changes

| File | Change |
|------|--------|
| `packages/canceler/src/watcher.rs` | Add Solana approval monitoring arm |
| `packages/canceler/src/verifier.rs` | Add Solana deposit verification (read `DepositRecord` PDAs) |
| `packages/canceler/src/config.rs` | Add `SolanaConfig` |
| `packages/canceler/src/solana.rs` | New: Solana-specific cancel instruction submission |

### 7d. Solana Canceler Flow

```
1. Poll Solana for withdraw_approve instructions (via getSignaturesForAddress)
2. For each approval:
   a. Read PendingWithdraw PDA to get full parameters
   b. Recompute transfer_hash
   c. Verify computed hash == stored hash
   d. Query source chain (EVM/Terra/Solana) for matching deposit
   e. If no matching deposit: submit withdraw_cancel instruction
3. Also verify approvals on EVM/Terra that target Solana:
   a. Read Solana DepositRecord PDA by hash
   b. If not found: cancel on EVM/Terra
```

---

## 8. Frontend (Phase 5)

### 8a. Wallet Integration

The frontend needs a Solana wallet adapter alongside the existing EVM (wagmi) and Terra (cosmes) wallets.

**Recommended library:** `@solana/wallet-adapter-react` + `@solana/wallet-adapter-wallets`

This provides adapters for all major Solana wallets:

| Wallet | Adapter |
|--------|---------|
| Phantom | `PhantomWalletAdapter` |
| Solflare | `SolflareWalletAdapter` |
| Backpack | `BackpackWalletAdapter` |
| Coinbase | `CoinbaseWalletAdapter` |
| Ledger | `LedgerWalletAdapter` |
| WalletConnect | Via Solana WalletConnect adapter |

### 8b. New / Updated Files

| File | Action | Description |
|------|--------|-------------|
| `src/lib/solana.ts` | New | Solana connection config, cluster URLs per environment |
| `src/services/solana/connect.ts` | New | Wallet connection, disconnect, reconnect |
| `src/services/solana/transaction.ts` | New | Build + sign deposit/withdraw instructions |
| `src/services/solana/detect.ts` | New | Detect installed Solana wallets |
| `src/services/solana/address.ts` | New | Base58 ↔ bytes32 encoding |
| `src/services/solana/index.ts` | New | Barrel export |
| `src/stores/solanaWallet.ts` | New | Zustand store for Solana wallet state (mirrors `stores/wallet.ts` pattern) |
| `src/hooks/useSolanaWallet.ts` | New | Solana wallet state wrapper |
| `src/hooks/useSolanaDeposit.ts` | New | Solana → other chain deposit flow |
| `src/components/wallet/SolanaWalletModal.tsx` | New | Wallet selection modal |
| `src/components/wallet/SolanaWalletOption.tsx` | New | Single wallet row |
| `src/components/transfer/WalletStatusBar.tsx` | Update | Show Solana connection status |
| `src/utils/bridgeChains.ts` | Update | Add Solana to `BRIDGE_CHAINS` per network tier |
| `src/utils/chainlist.ts` | Update | Add Solana to chainlist mapping |
| `src/hooks/useBridgeDeposit.ts` | Update | Handle `solana` source chain type |
| `src/hooks/useWithdrawSubmit.ts` | Update | Handle `solana` destination chain type |
| `src/hooks/useTransferRouteValidation.ts` | Update | Validate solana ↔ evm/terra/solana routes |
| `src/pages/TransferPage.tsx` | Update | Include Solana wallet connect prompt |

### 8c. Chain Config

Add Solana entries to `BRIDGE_CHAINS`:

```typescript
// src/utils/bridgeChains.ts

// local
{ id: 'solana-localnet', type: 'solana', name: 'Solana Localnet',
  rpcUrl: 'http://localhost:8899', programId: '<deployed-program-id>',
  bytes4ChainId: '0x00000005' },

// testnet
{ id: 'solana-devnet', type: 'solana', name: 'Solana Devnet',
  rpcUrl: 'https://api.devnet.solana.com', programId: '<devnet-program-id>',
  bytes4ChainId: '0x00000005' },

// mainnet
{ id: 'solana', type: 'solana', name: 'Solana',
  rpcUrl: 'https://api.mainnet-beta.solana.com', programId: '<mainnet-program-id>',
  bytes4ChainId: '0x00000005' },
```

### 8d. Transfer Direction Matrix (Updated)

| Source | Destination | Source Wallet | Deposit Hook | Notes |
|--------|-------------|---------------|-------------|-------|
| EVM → Terra | EVM (wagmi) | `useBridgeDeposit` | Existing |
| Terra → EVM | Terra (cosmes) | `useTerraDeposit` | Existing |
| EVM → Solana | EVM (wagmi) | `useBridgeDeposit` | New dest chain type |
| Solana → EVM | Solana (adapter) | `useSolanaDeposit` | New |
| Terra → Solana | Terra (cosmes) | `useTerraDeposit` | New dest chain type |
| Solana → Terra | Solana (adapter) | `useSolanaDeposit` | New |
| EVM → EVM | EVM (wagmi) | `useBridgeDeposit` | Existing |
| Solana → Solana | Solana (adapter) | `useSolanaDeposit` | Possible but unlikely |

### 8e. Withdraw on Solana

When a user withdraws on Solana (receiving tokens from another chain), the frontend must:

1. Build a `withdraw_submit` instruction with the transfer parameters
2. Sign and send via the connected Solana wallet
3. Wait for operator approval (poll `PendingWithdraw` PDA)
4. After delay, build and send `withdraw_execute` instruction

This differs from EVM (wagmi `writeContract`) and Terra (cosmes `executeMsg`) — it uses Solana's instruction-based model via `@solana/web3.js` or `@solana/kit`.

---

## 9. E2E Tests & Local Dev (Phase 6)

### 9a. Docker Compose Addition

Add `solana-test-validator` to `docker-compose.yml`:

```yaml
solana:
  image: solanalabs/solana:v2.2
  command: >
    solana-test-validator
    --reset
    --bind-address 0.0.0.0
    --rpc-port 8899
    --faucet-port 9900
    --limit-ledger-size 50000000
  ports:
    - "8899:8899"
    - "8900:8900"
    - "9900:9900"
  healthcheck:
    test: ["CMD", "solana", "cluster-version", "--url", "http://localhost:8899"]
    interval: 5s
    timeout: 5s
    retries: 10
```

### 9b. E2E Test Scenarios

| Test | Route | Validates |
|------|-------|-----------|
| Solana → EVM deposit + withdraw | Solana → Anvil | Full flow: SOL deposit, operator approval, EVM withdraw |
| EVM → Solana deposit + withdraw | Anvil → Solana | Full flow: EVM deposit, operator approval, Solana withdraw |
| Solana → Terra deposit + withdraw | Solana → LocalTerra | Full flow with Cosmos destination |
| Terra → Solana deposit + withdraw | LocalTerra → Solana | Full flow with Cosmos source |
| Canceler verifies Solana deposits | Any → Solana | Canceler reads Solana PDAs, verifies or cancels |
| Canceler verifies against Solana | Solana → Any | Canceler reads Solana DepositRecord, verifies |
| SPL token lock/unlock | Solana ↔ EVM | SPL token bridging (non-mintable) |
| SPL token mint/burn | Solana ↔ EVM | Bridged SPL token (mintable) |
| Hash parity cross-chain | All pairs | Same hash computed on Solana, EVM, Terra |

### 9c. Makefile Targets

```makefile
# Solana targets
solana-build:
	cd packages/contracts-solana && anchor build

solana-test:
	cd packages/contracts-solana && anchor test

solana-deploy-local:
	cd packages/contracts-solana && anchor deploy --provider.cluster localnet

solana-validator:
	solana-test-validator --reset
```

---

## 10. Chain Registration & Deployment (Phase 7)

### 10a. Assign Solana bytes4 Chain ID

Choose a `bytes4` chain ID for Solana. Options:

| Option | Value | Rationale |
|--------|-------|-----------|
| Solana's genesis hash prefix | Variable | Non-standard |
| Sequential | `0x00000005` | Next available after LocalTerra (`0x00000002`), Anvil1 (`0x00000003`) |
| **Solana convention** | `0x01399e79` (mainnet cluster ID) | Unique but arbitrary |

**Recommendation:** Use a simple, memorable value like `0x000001a1` (mainnet) and `0x000001a2` (devnet), or just `0x00000005` for simplicity. This should be decided and documented before deployment.

### 10b. Register Solana on Existing Chains

**On each EVM chain (BSC, opBNB):**

```solidity
chainRegistry.registerChain("solana_mainnet-beta", bytes4(0x00000005));
```

**On Terra Classic:**

```json
{
  "register_chain": {
    "chain_id": "AAAAAQU=",
    "identifier": "solana_mainnet-beta"
  }
}
```

### 10c. Register Tokens

For each token that bridges to/from Solana, register the mapping on all chains:

**EVM TokenRegistry:**

```solidity
tokenRegistry.setDestToken(localERC20, solanaChainId, solanaSPLMintBytes32);
```

**Terra bridge:**

```json
{
  "set_token_destination": {
    "token": "uluna",
    "dest_chain": "AAAAAQU=",
    "dest_token_address": "<base64-encoded-solana-mint-pubkey>"
  }
}
```

**Solana program:**

```
register_token instruction with:
  local_mint: <SPL mint pubkey>
  dest_chain: <bytes4 chain ID>
  dest_token: <bytes32 encoded dest token>
  mode: LockUnlock or MintBurn
```

### 10d. Deployment Sequence

```
1. Deploy Solana program to devnet
2. Run hash parity tests (Solana ↔ EVM ↔ Terra)
3. Register Solana on EVM testnet ChainRegistry
4. Register Solana on Terra testnet bridge
5. Configure operator with Solana RPC + keypair
6. Configure cancelers with Solana RPC
7. Run E2E tests on testnet
8. Deploy Solana program to mainnet-beta
9. Register Solana on BSC + opBNB mainnet
10. Register Solana on Terra Classic mainnet
11. Enable in frontend config
```

---

## 11. Architecture Diagrams

### 11a. Updated System Architecture

```mermaid
flowchart TB
    subgraph EVM[EVM Chains]
        EVMUser[User Wallet]
        Bridge[CL8YBridge]
        TokenReg[TokenRegistry]
        ChainReg[ChainRegistry]
    end

    subgraph Terra[Terra Classic]
        TerraUser[User Wallet]
        TerraBridge[Bridge Contract]
    end

    subgraph Solana[Solana]
        SolanaUser[User Wallet]
        SolanaBridge[Bridge Program]
        SolanaTokenReg[Token Registry PDA]
    end

    subgraph Infra[Infrastructure]
        Operator[Operator Service]
        Canceler[Canceler Network]
        DB[(PostgreSQL)]
    end

    EVMUser -->|deposit| Bridge
    TerraUser -->|lock| TerraBridge
    SolanaUser -->|deposit| SolanaBridge

    Bridge -.->|Deposit event| Operator
    TerraBridge -.->|Lock tx| Operator
    SolanaBridge -.->|Deposit instruction| Operator

    Operator -->|approveWithdraw| Bridge
    Operator -->|ApproveWithdraw| TerraBridge
    Operator -->|withdraw_approve| SolanaBridge
    Operator --> DB

    Canceler -.->|verify & cancel| Bridge
    Canceler -.->|verify & cancel| TerraBridge
    Canceler -.->|verify & cancel| SolanaBridge
```

### 11b. Solana Transfer Flow

```mermaid
sequenceDiagram
    participant User
    participant Solana as Solana Bridge Program
    participant Operator
    participant Canceler
    participant EVM as EVM Bridge

    User->>Solana: deposit_spl(dest_chain, dest_account, amount)
    Solana->>Solana: Lock SPL tokens in PDA
    Solana->>Solana: Create DepositRecord PDA
    Solana-->>Operator: Deposit instruction detected

    Operator->>EVM: approveWithdraw(hash, params)
    EVM-->>Canceler: WithdrawApproved event

    par Verification Window (5 min)
        Canceler->>Solana: Read DepositRecord PDA
        Solana-->>Canceler: Deposit exists, hash matches
        Note over Canceler: Valid — no action
    end

    User->>EVM: withdraw(hash)
    EVM->>User: Unlock ERC20 tokens
```

---

## 12. Dependency Order

```
Phase 1: Solana Programs
  ├── Bridge program (state, instructions, hash)
  ├── Unit tests (hash parity, instruction logic)
  └── Local deploy scripts

Phase 2: Address Codec (can start parallel to Phase 1)
  ├── multichain-rs: Solana address functions
  ├── AddressCodecLib.sol: encodeSolana
  ├── Terra address_codec.rs: encode_solana
  └── Parity tests across all four codebases

Phase 3: Operator (depends on Phase 1 + 2)
  ├── SolanaConfig
  ├── SolanaWatcher
  ├── SolanaWriter
  ├── DB migration
  └── Integration tests

Phase 4: Canceler (depends on Phase 1 + 2)
  ├── Solana approval monitoring
  ├── Solana deposit verification
  ├── Cancel instruction submission
  └── Integration tests

Phase 5: Frontend (depends on Phase 1, can parallel with 3+4)
  ├── Solana wallet adapter integration
  ├── Chain config + type updates
  ├── Deposit/withdraw hooks
  ├── Wallet modal UI
  └── Unit + integration tests

Phase 6: E2E Tests (depends on Phase 1-5)
  ├── Docker compose with solana-test-validator
  ├── Cross-chain flow tests
  └── Hash parity E2E validation

Phase 7: Deployment (depends on Phase 1-6)
  ├── Devnet/testnet deployment + registration
  ├── Testnet validation
  └── Mainnet deployment + registration
```

**Critical path:** Phase 1 (programs) → Phase 3 (operator) → Phase 6 (E2E) → Phase 7 (deploy)

**Parallelizable:** Phase 2 runs alongside Phase 1. Phase 5 can start once Phase 1 has a stable interface. Phases 3 and 4 can run in parallel.

---

## 13. Risks & Open Questions

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Solana 32-byte addresses break existing `UniversalAddress` | All codebases need updating | Option C: separate hash encoding from display; hash paths already use `[u8; 32]` |
| Solana RPC rate limits on mainnet | Watcher misses deposits | Use dedicated RPC provider (Helius, Triton, etc.); implement fallback URLs like EVM |
| Solana transaction size limits (1232 bytes) | Complex instructions may not fit | Split into multiple instructions if needed; use lookup tables |
| Anchor vs native Solana programs | Framework choice affects maintainability | Anchor is recommended — mature, well-documented, most Solana programs use it |
| Compute unit limits on Solana | keccak256 hash + state writes may exceed limits | Profile compute usage; request additional compute units if needed |
| Solana finality model differs from EVM | Deposits may be dropped before finalization | Wait for `confirmed` or `finalized` commitment level before processing |
| SPL Token 2022 vs classic SPL Token | Some tokens use Token Extensions | Support both token programs; check mint's owning program |

### Open Questions

| # | Question | Options | Impact |
|---|----------|---------|--------|
| 1 | What `bytes4` chain ID for Solana? | `0x00000005`, cluster hash prefix, other | Chain registration on all bridges |
| 2 | Anchor vs native Solana program? | Anchor (recommended), native solana-program | Development speed, audit surface |
| 3 | Which Solana cluster for testnet? | Devnet (recommended), Testnet | Faucet availability, stability |
| 4 | SPL Token 2022 support? | Yes (both programs), Classic only initially | Token compatibility |
| 5 | Solana wallet adapter library version? | `@solana/wallet-adapter-react` (v1) or build on `@solana/kit` (v5) | Frontend architecture |
| 6 | WSOL handling? | Auto-wrap native SOL to WSOL, or handle SOL natively | UX complexity |
| 7 | Multi-signature upgrade authority? | Squads multisig, single upgrade authority, immutable | Program security |
| 8 | Which mainnet RPC provider? | Helius, Triton, QuickNode, public | Cost, reliability |

---

## Appendix A: Solana vs Existing Chains

| Aspect | EVM | Terra Classic | Solana |
|--------|-----|---------------|--------|
| **Contract language** | Solidity | Rust (CosmWasm) | Rust (Anchor/native) |
| **State model** | Contract storage slots | Contract-owned state | Account-based (PDAs) |
| **Token standard** | ERC20 | CW20 / native denoms | SPL Token |
| **Address size** | 20 bytes | 20 bytes (bech32 canonical) | 32 bytes (Ed25519 pubkey) |
| **Address format** | Hex (0x...) | Bech32 (terra1...) | Base58 |
| **Block time** | ~3-12s (varies) | ~6s | ~400ms |
| **Finality** | Chain-dependent | Instant (Tendermint) | Optimistic (~30s confirmed) |
| **Gas model** | Gas price × gas used | Gas + stability fee | Compute units + priority fee |
| **Event system** | Log topics + data | Tx attributes | Program logs + account changes |
| **Indexing** | `eth_getLogs` | LCD `tx_search` | `getSignaturesForAddress` + `getTransaction` |
| **Testing** | Foundry (forge) | cw-multi-test | Anchor bankrun / solana-program-test |
| **Wallet ecosystem** | MetaMask, WalletConnect, etc. | Station, Keplr, Leap | Phantom, Solflare, Backpack |

---

## Related Documentation

- [System Architecture](./architecture.md) — Component overview
- [Security Model](./security-model.md) — Watchtower pattern and roles
- [Cross-Chain Hash Parity](./crosschain-parity.md) — Hash computation and parity requirements
- [Crosschain Transfer Flows](./crosschain-flows.md) — Step-by-step transfer diagrams
- [EVM Contracts](./contracts-evm.md) — Solidity contract details
- [Terra Classic Contracts](./contracts-terraclassic.md) — CosmWasm contract details
- [Operator](./operator.md) — Operator service documentation
- [Canceler Network](./canceler-network.md) — Canceler node setup
