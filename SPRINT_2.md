# Sprint 2: Relayer Implementation & Integration Testing

**Sprint Duration**: Estimated 2-3 sessions  
**Prerequisites**: Sprint 1 completed (relayer scaffolding, docs, infrastructure)  
**Handoff Date**: 2026-01-28

---

## Sprint 1 Summary (Completed)

### What Was Built
- **Relayer Package Scaffolding** (`packages/relayer/`)
  - All 11 source files generated via WorkSplit + manual fixes
  - Compilation successful, clippy clean, 7 unit tests passing
  - Database schema with migrations (`migrations/001_initial.sql`)
  
- **Infrastructure**
  - `docker-compose.yml` with Anvil, LocalTerra Classic, PostgreSQL
  - Deployment scripts (`scripts/deploy-local.sh`, `setup-bridge.sh`, `test-transfer.sh`)
  - Makefile with common commands

- **Documentation** (`docs/`)
  - Full cross-linked documentation structure
  - Architecture, contracts, relayer, local dev, crosschain flows

### What Was Fixed (This Session)
1. **BigDecimal/sqlx compatibility** - Changed to String for DB storage to avoid version conflicts
2. **alloy 0.1 API** - Fixed provider construction, log parsing, type conversions
3. **tendermint-rpc 0.35 API** - Fixed HttpClient creation, block height queries
4. **Rust 1.85 compatibility** - Downgraded `time` and `home` crates

### Current State
```
packages/relayer/
├── src/
│   ├── main.rs           ✅ Compiles, starts runtime
│   ├── config.rs         ✅ Loads from .env, validates
│   ├── types.rs          ✅ ChainKey, EvmAddress, WithdrawHash
│   ├── db/
│   │   ├── mod.rs        ✅ All CRUD operations
│   │   └── models.rs     ✅ All models with String amounts
│   ├── watchers/
│   │   ├── mod.rs        ✅ WatcherManager
│   │   ├── evm.rs        ✅ Watches DepositRequest events
│   │   └── terra.rs      ✅ Polls for Lock transactions
│   └── writers/
│       ├── mod.rs        ✅ WriterManager loop
│       ├── evm.rs        ⚠️ STUB - logs only, no tx submission
│       └── terra.rs      ⚠️ STUB - logs only, no tx submission
```

---

## Sprint 2 Goals

### Primary Objective
**Complete the relayer transaction submission logic and verify end-to-end crosschain transfers in local environment.**

### Deliverables

#### 1. Complete EVM Writer (`writers/evm.rs`)
**Priority: HIGH | Complexity: MEDIUM | ~150-200 lines**

Implement `approveWithdraw()` transaction submission:
- Build transaction calldata for `CL8YBridge.approveWithdraw()`
- Sign with configured private key
- Submit transaction and track status
- Handle gas estimation, nonce management, retries

```rust
// Key functions to implement:
async fn process_deposit(&self, deposit: &TerraDeposit) -> Result<()>
async fn build_approval_calldata(&self, deposit: &TerraDeposit) -> Result<Bytes>
async fn submit_approval(&self, approval: &NewApproval) -> Result<String>
```

**Reference**: `packages/contracts-evm/DOC.md` for `approveWithdraw()` signature

#### 2. Complete Terra Writer (`writers/terra.rs`)
**Priority: HIGH | Complexity: HIGH | ~200-250 lines**

Implement `Release` message submission:
- Build CosmWasm execute message
- Sign with configured mnemonic (use `cosmrs` for key derivation)
- Broadcast transaction via LCD/RPC
- Track confirmation status

```rust
// Key functions to implement:
async fn process_deposit(&self, deposit: &EvmDeposit) -> Result<()>
async fn build_release_msg(&self, deposit: &EvmDeposit) -> Result<ExecuteMsg>
async fn broadcast_tx(&self, msg: ExecuteMsg) -> Result<String>
```

**Reference**: `packages/contracts-terraclassic/` for Release message schema

#### 3. Add Contract ABI/Message Definitions
**Priority: HIGH | Complexity: LOW | ~50-100 lines**

Create `src/contracts/` module with:
- EVM bridge ABI (just the `approveWithdraw` function)
- Terra bridge message types (ExecuteMsg::Release)

```rust
// src/contracts/mod.rs
pub mod evm_bridge;    // alloy sol! macro for ABI
pub mod terra_bridge;  // Serde structs for CosmWasm messages
```

#### 4. Integration Testing
**Priority: HIGH | Complexity: MEDIUM**

- Start local environment: `make start`
- Deploy contracts: `scripts/deploy-local.sh`
- Configure bridge: `scripts/setup-bridge.sh`
- Run relayer against local chains
- Execute test transfer: `scripts/test-transfer.sh`
- Verify funds arrive on destination chain

#### 5. Error Handling & Retry Logic
**Priority: MEDIUM | Complexity: LOW | ~50 lines**

- Implement exponential backoff for failed transactions
- Add circuit breaker for repeated failures
- Improve error messages with actionable context

---

## Technical Notes for Next Manager

### Key Files to Understand
1. `docs/crosschain-flows.md` - Transfer flow diagrams
2. `packages/contracts-evm/DOC.md` - EVM contract interface
3. `packages/relayer/migrations/001_initial.sql` - Database schema

### Dependencies Already Configured
```toml
# Cargo.toml - all needed deps are present
alloy = { version = "0.1", features = ["full"] }  # EVM
cosmrs = { version = "0.16", features = ["cosmwasm"] }  # Terra
tendermint-rpc = { version = "0.35", features = ["http-client"] }
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "chrono"] }
```

### Amount Handling
- **DB Storage**: Amounts stored as `String` (TEXT) to avoid BigDecimal/sqlx conflicts
- **In Code**: Use `bigdecimal::BigDecimal` for calculations, `.to_string()` for DB
- **On Chain**: Parse to U256 (EVM) or Uint128 (CosmWasm) before submission

### Chain Key Computation
```rust
// EVM chain key (already implemented in types.rs)
ChainKey::evm(chain_id)  // keccak256("EVM", chainId)

// Cosmos chain key
ChainKey::cosmos("rebel-2", "terra")  // keccak256("COSMOS", chainId, ":", prefix)
```

### Local Environment
```bash
# Start all services
docker-compose up -d

# Anvil (EVM): http://localhost:8545
# LocalTerra RPC: http://localhost:26657
# LocalTerra LCD: http://localhost:1317
# PostgreSQL: localhost:5432

# Test accounts are pre-funded
```

### WorkSplit Best Practices (from `.cursor/rules/`)
- Target 100-300 lines per job
- Always use REPLACE mode (edit mode unreliable)
- Break complex features into multiple jobs
- Verify builds between jobs

---

## Stretch Goals (If Time Permits)

1. **Multi-chain Support**
   - Current: Single EVM chain, single Terra chain
   - Stretch: Multiple EVM chains (configurable array)

2. **Metrics & Monitoring**
   - Prometheus metrics endpoint
   - Transaction latency tracking
   - Success/failure rates

3. **Health Check Endpoint**
   - HTTP server with `/health` and `/ready`
   - Chain connectivity status

4. **Graceful Shutdown**
   - Complete in-flight transactions before stopping
   - Save state on SIGTERM

---

## Risk Items

| Risk | Mitigation |
|------|------------|
| LocalTerra Classic image unavailable | Use fallback terrad build or mock Terra responses |
| Gas estimation failures on Anvil | Use fixed gas limits for local testing |
| CosmWasm signing complexity | Start with single-key signing, add multisig later |
| Transaction ordering issues | Use database-level locking on nonce |

---

## Definition of Done

Sprint 2 is complete when:
- [ ] EVM writer submits real `approveWithdraw` transactions
- [ ] Terra writer submits real `Release` transactions  
- [ ] End-to-end transfer EVM→Terra succeeds locally
- [ ] End-to-end transfer Terra→EVM succeeds locally
- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated with any API changes

---

## Quick Start for Next Manager

```bash
cd packages/relayer

# Verify current state
cargo check && cargo test && cargo clippy

# Read key documentation
cat ../../docs/crosschain-flows.md
cat ../../docs/relayer.md

# Start implementing writers
# Focus on writers/evm.rs first (simpler signing)
```

Good luck! The foundation is solid - now make it move funds.

---

## Appendix: Sprint 2 Progress (2026-01-28)

### Completed This Session

#### 1. Contract ABI/Message Definitions ✅
- Created `src/contracts/mod.rs` - module exports
- Created `src/contracts/evm_bridge.rs` - alloy `sol!` macro for `approveWithdraw()`
- Created `src/contracts/terra_bridge.rs` - CosmWasm `ExecuteMsg::Release` types

#### 2. Complete EVM Writer ✅ (~220 lines)
- `process_pending()` - Fetches pending Terra deposits from DB
- `process_deposit()` - Creates approval records and submits transactions
- `build_approval()` - Builds `NewApproval` with chain keys, withdraw hash, fees
- `submit_approval()` - Signs and submits `approveWithdraw` via alloy

#### 3. Complete Terra Writer ✅ (~300 lines)
- `process_pending()` - Fetches pending EVM deposits from DB
- `process_deposit()` - Creates release records and broadcasts transactions
- `build_release()` - Builds `NewRelease` from EVM deposit data
- `build_release_msg()` - Creates CosmWasm execute message
- `broadcast_tx()` - Signs via BIP39 mnemonic and broadcasts via LCD API
- Key derivation using HD path `m/44'/330'/0'/0/0`

#### 4. Dependencies Added
- `base64 = "0.22"` - For LCD API transaction encoding
- `bip39 = "2.0"` - For mnemonic key derivation

#### 5. Verification
- `cargo check` ✅ Compiles
- `cargo test` ✅ 7 tests passing
- `cargo clippy` ✅ No warnings

### Remaining Work

#### 1. Integration Testing (Priority: HIGH)

**Prerequisites:**
- Docker installed and running
- Foundry installed (`forge`, `cast`, `anvil`)
- Rust toolchain

**Step-by-step instructions:**

```bash
# 1. Start infrastructure (Anvil on :8545, PostgreSQL on :5433)
docker compose up -d anvil postgres

# 2. Verify services are healthy
docker compose ps

# 3. Deploy EVM contracts to Anvil
cd packages/contracts-evm
forge script script/DeployLocal.s.sol:DeployLocal \
    --broadcast \
    --rpc-url http://localhost:8545 \
    -vvv

# 4. Note the deployed addresses from output:
#    - EVM_BRIDGE_ADDRESS=0x...
#    - Save these to packages/relayer/.env

# 5. Copy and configure relayer environment
cd packages/relayer
cp .env.example .env
# Edit .env with deployed addresses

# 6. Run database migrations
cargo sqlx migrate run

# 7. Run relayer
cargo run

# 8. (Optional) Start Terra Classic for full testing
# See: https://github.com/classic-terra/core
# Run: make localnet-start
```

**Note:** LocalTerra Classic requires building from source. See `docker-compose.yml` comments for options.

**Integration Test Results (2026-01-28):**
- ✅ Anvil (EVM): Running on port 8545
- ✅ PostgreSQL: Running on port 5433
- ✅ EVM Contracts: Deployed successfully
  - AccessManager: `0x5FbDB2315678afecb367f032d93F642f64180aa3`
  - ChainRegistry: `0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512`
  - TokenRegistry: `0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0`
  - Cl8YBridge: `0x5FC8d32690cc91D4c39d9d3abcBD16989F875707`
- ✅ Relayer: Starts, connects to DB, runs migrations
- ⏸️ Terra: Not tested (requires LocalTerra setup from classic-terra/core)

#### 2. Error Handling & Retry Logic ✅ COMPLETED
- ✅ Added `RetryConfig` struct with configurable parameters
- ✅ Exponential backoff for failed transactions (1s → 2s → 4s → ... → 60s max)
- ✅ Circuit breaker (pauses 5 min after 10 consecutive failures)
- ✅ Improved error messages with failure counts and backoff timing
- Implementation: ~55 lines in `src/writers/mod.rs`

#### 3. Documentation Updates (Priority: LOW)
- Update `docs/relayer.md` with new contracts module
- Add API documentation for new writer methods

### Updated File Tree
```
packages/relayer/src/
├── main.rs           ✅ Updated with mod contracts, mutable writer_manager
├── config.rs         ✅ Unchanged
├── types.rs          ✅ Unchanged
├── contracts/        ✅ NEW
│   ├── mod.rs        ✅ Module exports
│   ├── evm_bridge.rs ✅ alloy sol! ABI
│   └── terra_bridge.rs ✅ CosmWasm messages
├── db/
│   ├── mod.rs        ✅ Unchanged
│   └── models.rs     ✅ Unchanged
├── watchers/
│   ├── mod.rs        ✅ Unchanged
│   ├── evm.rs        ✅ Unchanged
│   └── terra.rs      ✅ Unchanged
└── writers/
    ├── mod.rs        ✅ UPDATED - RetryConfig, exponential backoff, circuit breaker
    ├── evm.rs        ✅ COMPLETE - full tx submission
    └── terra.rs      ✅ COMPLETE - full tx submission
```
