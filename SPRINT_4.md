# Sprint 4: Production Hardening & Frontend Foundation

**Sprint Duration**: Estimated 3-4 sessions  
**Prerequisites**: Sprint 3 completed (E2E testing, metrics, multi-relayer docs)  
**Handoff Date**: 2026-01-28

---

## Sprint 3 Retrospective

### What Was Completed
- **Integration Test Suite** (`tests/integration_test.rs`)
  - Environment connectivity tests
  - Chain key computation tests
  - Address encoding tests
  
- **Deployment Scripts**
  - `scripts/deploy-terra-local.sh` - LocalTerra deployment
  - `scripts/setup-bridge.sh` - Cross-chain configuration
  - `scripts/test-transfer.sh` - Interactive transfer testing
  - `scripts/e2e-test.sh` - Automated E2E tests

- **Prometheus Metrics** (`src/metrics.rs`)
  - Block processing metrics
  - Transaction counters
  - Circuit breaker gauges
  - Health endpoint (/health)

- **Multi-Relayer Documentation** (`docs/multi-relayer.md`)
  - Docker Compose setup for 3 relayers
  - Prometheus/Grafana integration
  - Coordination strategies

### What Works Well
1. **Code Structure** - Clean separation: watchers, writers, db, types
2. **Error Handling** - Consistent use of eyre with context
3. **Logging** - Structured tracing throughout
4. **Database Schema** - Well-designed with proper indexes

### Areas Needing Improvement

| Area | Issue | Impact | Effort |
|------|-------|--------|--------|
| Transaction Confirmation | No tracking after "submitted" | HIGH | MEDIUM |
| Error Recovery | Failed txs stay failed forever | HIGH | MEDIUM |
| Amount Handling | String conversions everywhere | MEDIUM | HIGH |
| Configuration | No startup validation | MEDIUM | LOW |
| Testing | No unit tests for business logic | MEDIUM | MEDIUM |
| Frontend | Not started | HIGH | HIGH |

---

## Refactoring Assessment

### Worth Doing Now

#### 1. Transaction Confirmation Tracking
**Cost**: ~200 lines | **Benefit**: Critical for reliability

Currently, transactions go from `pending` → `submitted` but never reach `confirmed`. The relayer should:
- Poll for transaction receipts
- Update status to `confirmed` or `failed`
- Handle reorgs (status = `reorged`)

```rust
// Add to writers/mod.rs
async fn confirm_pending_transactions(&self) -> Result<()> {
    let submitted = db::get_submitted_approvals(&self.db).await?;
    for approval in submitted {
        match self.check_receipt(&approval.tx_hash).await {
            Ok(confirmed) if confirmed => {
                db::update_approval_confirmed(&self.db, approval.id).await?;
            }
            Ok(_) => {} // Still pending
            Err(e) if is_reorg(&e) => {
                db::update_approval_status(&self.db, approval.id, "reorged").await?;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to check receipt");
            }
        }
    }
    Ok(())
}
```

#### 2. Retry Failed Transactions
**Cost**: ~100 lines | **Benefit**: Self-healing

Failed transactions should be retried with exponential backoff:

```rust
// In writers/mod.rs - process_pending
let failed = db::get_failed_approvals_for_retry(&self.db, max_retries).await?;
for approval in failed {
    if approval.attempts < self.retry_config.max_retries {
        self.retry_approval(&approval).await?;
    }
}
```

#### 3. Configuration Validation
**Cost**: ~50 lines | **Benefit**: Fail fast on bad config

```rust
impl Config {
    pub fn validate(&self) -> Result<()> {
        // Validate EVM address format
        Address::from_str(&self.evm.bridge_address)
            .wrap_err("Invalid EVM bridge address")?;
        
        // Validate Terra address format (bech32)
        if !self.terra.bridge_address.starts_with("terra1") {
            return Err(eyre!("Invalid Terra bridge address"));
        }
        
        // Validate private key can derive signer
        self.evm.private_key.parse::<PrivateKeySigner>()
            .wrap_err("Invalid EVM private key")?;
        
        Ok(())
    }
}
```

### Not Worth Doing Now

#### 1. Amount Type Abstraction
**Cost**: ~300 lines across entire codebase  
**Benefit**: Type safety, but current string approach works

The current string-based amounts work correctly. A proper `Amount` type would add compile-time safety but requires touching every file. Defer until we hit actual bugs.

#### 2. Trait-based Chain Abstraction
**Cost**: ~500 lines  
**Benefit**: Testability, but we only have 2 chains

Creating `Chain`, `Watcher<C>`, `Writer<C>` traits would be elegant but we only support EVM and Terra. The abstraction cost outweighs the benefit until we add a third chain type.

#### 3. Database Query Generics
**Cost**: ~200 lines  
**Benefit**: DRY, but reduces explicitness

The current explicit queries are easy to understand and debug. Macro-based or generic queries save lines but obscure behavior.

---

## Sprint 4 Goals

### Primary Objective
**Harden the relayer for production reliability and begin frontend development.**

### Deliverables

#### 1. Transaction Confirmation Loop
**Priority: HIGH | Complexity: MEDIUM | ~200 lines**

Add background task to track transaction confirmations:

```
packages/relayer/src/
├── confirmation/
│   ├── mod.rs          # ConfirmationTracker
│   ├── evm.rs          # EVM receipt polling
│   └── terra.rs        # Terra tx polling
```

**Requirements:**
- Poll for receipts of submitted transactions
- Update status: submitted → confirmed or failed
- Handle reorgs: detect missing txs, mark as reorged
- Configurable confirmation depth (e.g., 12 blocks for EVM)

#### 2. Automatic Retry System
**Priority: HIGH | Complexity: LOW | ~100 lines**

Enhance the retry logic:
- Retry failed transactions up to max_retries
- Exponential backoff between attempts
- Don't retry permanently failed txs (e.g., reverted)
- Add `retry_after` timestamp to prevent immediate retries

**Database Migration:**
```sql
ALTER TABLE approvals ADD COLUMN retry_after TIMESTAMPTZ;
ALTER TABLE releases ADD COLUMN retry_after TIMESTAMPTZ;
```

#### 3. Health & Status API
**Priority: HIGH | Complexity: MEDIUM | ~150 lines**

Add HTTP endpoints for monitoring:

```
GET /health          → {"status": "ok"}
GET /metrics         → Prometheus format (already done)
GET /status          → {"evm": {...}, "terra": {...}}
GET /pending         → List of pending transactions
```

**Implementation:** Use the existing metrics server, add routes.

#### 4. Configuration Validation
**Priority: MEDIUM | Complexity: LOW | ~50 lines**

Validate all configuration on startup:
- Address formats (EVM, Terra)
- Private key derivation
- Database connectivity
- RPC endpoint reachability

#### 5. Unit Tests for Business Logic
**Priority: MEDIUM | Complexity: MEDIUM | ~200 lines**

Add unit tests for critical functions:
- `ChainKey::evm()` and `ChainKey::cosmos()`
- `WithdrawHash::compute()`
- Fee calculations
- Address encoding/decoding

```
packages/relayer/src/
├── types.rs           # Add #[cfg(test)] module
├── writers/
│   └── evm.rs         # Add tests for build_approval()
```

#### 6. EVM to EVM Cross-Chain Support
**Priority: HIGH | Complexity: MEDIUM | ~250 lines**

Enable bridging between different EVM-compatible chains (e.g., Ethereum ↔ BSC, Polygon ↔ Arbitrum):

```
packages/relayer/src/
├── watchers/
│   └── evm.rs          # Handle EVM→EVM deposits
├── writers/
│   └── evm.rs          # Write to destination EVM chain
├── config.rs           # Multi-EVM chain configuration
```

**Requirements:**
- Support multiple EVM chain configurations in config
- Detect source chain from deposit event
- Route to correct destination EVM writer
- Compute chain keys for all configured EVM chains
- Update ChainRegistry on each EVM chain with other chains

**Configuration:**
```toml
[[evm_chains]]
name = "ethereum"
chain_id = 1
rpc_url = "https://eth.llamarpc.com"
bridge_address = "0x..."

[[evm_chains]]
name = "bsc"
chain_id = 56
rpc_url = "https://bsc-dataseed.binance.org"
bridge_address = "0x..."

[[evm_chains]]
name = "polygon"
chain_id = 137
rpc_url = "https://polygon-rpc.com"
bridge_address = "0x..."
```

**Database Updates:**
```sql
-- Add source/dest chain tracking
ALTER TABLE deposits ADD COLUMN source_chain_id BIGINT;
ALTER TABLE deposits ADD COLUMN dest_chain_id BIGINT;
ALTER TABLE approvals ADD COLUMN dest_chain_id BIGINT;
```

**Transfer Flow (ETH → BSC):**
1. User deposits on Ethereum bridge (dest_chain_key = BSC)
2. EVM watcher detects deposit, records with source=1, dest=56
3. EVM writer (BSC) picks up pending deposit
4. Submits approval transaction on BSC bridge
5. User withdraws on BSC

#### 7. Terrad Integration for Automated E2E Testing
**Priority: HIGH | Complexity: MEDIUM | ~200 lines**

Add terrad CLI support to E2E test scripts for fully automated Terra interactions:

```
scripts/
├── e2e-test.sh         # Updated with terrad commands
├── lib/
│   └── terra-helpers.sh  # Terrad helper functions
```

**Requirements:**
- Install and configure terrad in Docker environment
- Add terrad helper functions for common operations
- Enable headless key management (non-interactive)
- Full automation of Terra contract interactions
- Wait for transaction confirmations

**Helper Functions:**
```bash
# scripts/lib/terra-helpers.sh

# Query Terra balance
terra_balance() {
    local address=$1
    local denom=$2
    terrad query bank balances "$address" \
        --node "$TERRA_RPC_URL" \
        --output json | jq -r ".balances[] | select(.denom==\"$denom\") | .amount"
}

# Execute lock on Terra bridge
terra_lock() {
    local amount=$1
    local denom=$2
    local dest_chain=$3
    local recipient=$4
    
    terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" \
        "{\"lock\":{\"dest_chain_id\":$dest_chain,\"recipient\":\"$recipient\"}}" \
        --amount "${amount}${denom}" \
        --from "$TERRA_KEY_NAME" \
        --node "$TERRA_RPC_URL" \
        --chain-id "$TERRA_CHAIN_ID" \
        --gas auto --gas-adjustment 1.3 \
        --yes --output json
}

# Wait for Terra transaction
terra_wait_tx() {
    local tx_hash=$1
    local timeout=${2:-60}
    
    for i in $(seq 1 $timeout); do
        result=$(terrad query tx "$tx_hash" --node "$TERRA_RPC_URL" --output json 2>/dev/null)
        if [ $? -eq 0 ]; then
            echo "$result"
            return 0
        fi
        sleep 1
    done
    return 1
}
```

**Docker Integration:**
```yaml
# docker-compose.yml addition
services:
  terrad-cli:
    image: terramoney/localterra-core:latest
    entrypoint: /bin/sh
    command: -c "sleep infinity"
    volumes:
      - ./scripts:/scripts:ro
      - terra-keys:/root/.terra
    networks:
      - bridge-network
```

**E2E Test Updates:**
```bash
# Run terrad commands via Docker
docker compose exec terrad-cli terrad tx wasm execute ...

# Or install terrad locally and use directly
terrad tx wasm execute "$TERRA_BRIDGE" ...
```

**Definition of Done:**
- [ ] terrad helper functions work in Docker and locally
- [ ] E2E tests run without manual intervention
- [ ] Terra lock/unlock fully automated
- [ ] Transaction confirmation waits implemented
- [ ] CI-compatible (headless, no prompts)

#### 8. Frontend Foundation
**Priority: HIGH | Complexity: HIGH | ~500 lines**

Create basic frontend structure:

```
packages/frontend/
├── package.json
├── src/
│   ├── App.tsx
│   ├── components/
│   │   ├── ConnectWallet.tsx
│   │   ├── BridgeForm.tsx
│   │   └── TransactionHistory.tsx
│   ├── hooks/
│   │   ├── useEvmWallet.ts
│   │   └── useTerraWallet.ts
│   └── lib/
│       ├── evm.ts
│       └── terra.ts
├── index.html
└── vite.config.ts
```

**Tech Stack:**
- React 18 + TypeScript
- Vite for bundling
- TailwindCSS for styling
- wagmi + viem for EVM
- @terra-money/wallet-kit for Terra

**MVP Features:**
- Connect EVM wallet (MetaMask, etc.)
- Connect Terra wallet (Station)
- Bridge form (select token, amount, destination)
- Basic transaction history (from local storage)

---

## Technical Debt Tracking

| Item | Priority | Sprint | Notes |
|------|----------|--------|-------|
| Confirmation tracking | HIGH | 4 | Essential for reliability |
| Retry failed txs | HIGH | 4 | Self-healing |
| Health API | HIGH | 4 | Monitoring |
| **EVM to EVM support** | HIGH | 4 | Key feature |
| **Terrad E2E automation** | HIGH | 4 | CI-ready testing |
| Config validation | MEDIUM | 4 | Fail fast |
| Unit tests | MEDIUM | 4 | Confidence |
| Frontend MVP | HIGH | 4-5 | User-facing |
| Amount type | LOW | 5+ | Nice to have |
| Chain abstraction | LOW | 5+ | If adding chains |
| Multi-sig relayer | MEDIUM | 5 | Production security |

---

## Feature Support Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| EVM → Terra transfers | Implemented | Via deposit + release |
| Terra → EVM transfers | Implemented | Via lock + approve |
| EVM → EVM transfers | **Sprint 4** | Multi-EVM chain support |
| LUNC support | Configured | In setup-bridge.sh |
| USTC support | Configured | In setup-bridge.sh |
| Native ETH | Partial | Router not deployed |
| ERC20 tokens | Implemented | Bridge contract supports |
| CW20 tokens | Not tested | May need work |
| Multi-relayer | Documented | Not tested |
| Transaction confirmation | Missing | Sprint 4 |
| Error recovery | Partial | Sprint 4 |
| Automated E2E (terrad) | **Sprint 4** | Full Terra automation |
| Frontend | Missing | Sprint 4-5 |
| Mainnet deployment | Not ready | Sprint 5+ |

---

## Risk Items

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| LocalTerra instability | HIGH | MEDIUM | Use testnet for integration |
| Transaction reorgs | MEDIUM | HIGH | Implement reorg detection |
| Key compromise | LOW | CRITICAL | Use hardware wallets in prod |
| Contract bugs | MEDIUM | CRITICAL | Audit before mainnet |
| API rate limits | MEDIUM | MEDIUM | Implement backoff |

---

## Definition of Done

Sprint 4 is complete when:

- [ ] Confirmation tracking implemented and tested
- [ ] Failed transaction retry working
- [ ] `/health` and `/status` endpoints respond
- [ ] Configuration validates on startup
- [ ] Unit tests added for types.rs
- [ ] **EVM → EVM transfers working** (e.g., Anvil1 ↔ Anvil2)
- [ ] **Multi-EVM config supported in relayer**
- [ ] **terrad helpers integrated into E2E tests**
- [ ] **E2E tests run fully automated (no manual steps)**
- [ ] Frontend project scaffolded
- [ ] Frontend connects to EVM wallet
- [ ] Frontend connects to Terra wallet
- [ ] Basic bridge form renders
- [ ] All existing tests pass

---

## Quick Start for Sprint 4

```bash
# 1. Start infrastructure
docker compose up -d

# 2. Run relayer with new features
cd packages/relayer
cargo run

# 3. Check health endpoint
curl http://localhost:9090/health

# 4. Check status endpoint
curl http://localhost:9090/status

# 5. Start frontend dev server
cd packages/frontend
npm install
npm run dev
```

---

## Sprint 4 Implementation Order

1. **Configuration validation** (30 min) - Quick win, fail fast
2. **Confirmation tracking** (2-3 hrs) - Critical reliability
3. **Retry system** (1 hr) - Build on confirmation
4. **Health/Status API** (1-2 hrs) - Monitoring
5. **EVM to EVM support** (3-4 hrs) - Multi-chain foundation
6. **Terrad E2E automation** (2 hrs) - CI-ready testing
7. **Unit tests** (1-2 hrs) - Confidence building
8. **Frontend scaffold** (2-3 hrs) - Parallel workstream
9. **Wallet connections** (2-3 hrs) - Core frontend feature
10. **Bridge form** (2-3 hrs) - MVP completion

---

## Notes for Future Sprints

### Sprint 5 Candidates
- Frontend: Transaction history from relayer API
- Frontend: Token selection with balances
- Multi-sig relayer implementation
- Testnet deployment (Goerli + Terra testnet)
- Gas estimation improvements
- Admin dashboard

### Sprint 6 Candidates
- Mainnet deployment prep
- Security audit preparation
- Rate limiting per address
- Fee adjustment UI
- Mobile-responsive frontend

---

Good luck! The foundation is solid - now make it production-ready.
