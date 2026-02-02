# Sprint 10: Full E2E Integration with LocalTerra

**Previous Sprint:** [SPRINT9.md](./SPRINT9.md) - Terra Classic Watchtower Implementation

---

## Sprint 9 Retrospective

### What Went Well

1. **LocalTerra Finally Working** - After investigating multiple approaches:
   - mint-cash/LocalTerra: Failed with nil pointer in staking params
   - Attempted to build from git: Same genesis.json issues
   - **Solution:** Official `classic-terra/localterra-core:0.5.18` image with config from `classic-terra/localterra`
   - Config files stored in `infra/localterra/config/` and mounted correctly

2. **Canceler EVM Verification Implemented** - Real deposit verification added:
   - Uses `alloy` to query `getDepositFromHash()` on EVM source chain
   - Verifies all parameters match (destChainKey, destToken, destAccount, amount, nonce)
   - Graceful fallback when source chain unreachable

3. **Documentation Fixed** - Incorrect LocalTerra references updated across all docs:
   - README.md, docs/testing.md, SPRINT8.md, SPRINT9.md
   - Now correctly uses `docker compose up -d localterra`

4. **Watchtower E2E Tests Added** - New tests in `scripts/e2e-test.sh`:
   - `test_evm_watchtower_approve_execute_flow()` - Tests approve → delay → execute
   - `test_evm_watchtower_cancel_flow()` - Tests cancel mechanism  
   - `test_hash_parity()` - Runs hash parity tests

5. **Pre-existing Implementation Verified** - Discovered that all core watchtower functionality was already implemented:
   - Terra contract: `hash.rs`, `execute/watchtower.rs`
   - Operator: `writers/terra.rs` with approve → wait → execute
   - Canceler: `terra_client.rs`, `evm_client.rs` for cancel submission

### What Went Wrong

1. **LocalTerra Image Confusion** - Multiple LocalTerra options exist:
   - `classic-terra/LocalTerra` - Old Luna (pre-Classic)
   - `mint-cash/LocalTerra` - Fork with genesis bugs
   - `classic-terra/localterra` - Official Terra Classic, but repo structure was unclear
   - **Lesson:** The official image is `classic-terra/localterra-core`, but it needs config files from `classic-terra/localterra`

2. **Config Mount Required** - The LocalTerra image doesn't include genesis.json:
   - Image expects config at `/root/.terra/config`
   - Must mount `genesis.json`, `config.toml`, `app.toml`, etc.
   - **Lesson:** Docker images may be minimal; check what needs to be mounted

3. **Documentation vs Reality Gap** - Sprint docs described features as "not implemented" that were actually complete:
   - All watchtower contract code already existed
   - Operator and canceler already had the new flow
   - **Lesson:** Always verify codebase state before planning

### Key Metrics

| Metric | Before Sprint 9 | After Sprint 9 |
|--------|-----------------|----------------|
| LocalTerra working | ❌ No | ✅ Yes |
| E2E tests passing | 7 | 8 |
| Terra connectivity | ❌ Blocked | ✅ Works |
| Canceler EVM verification | MVP stub | ✅ Real queries |
| Hash parity tests | Passing | Passing |

---

## Sprint 10 Objectives

### Priority 1: Deploy Terra Bridge to LocalTerra

Now that LocalTerra is working, we need to deploy our Terra contract and complete the bridge setup.

#### 1.1 Build Terra Contract WASM

```bash
cd packages/contracts-terraclassic
cargo build --release --target wasm32-unknown-unknown
```

#### 1.2 Deploy to LocalTerra

```bash
./scripts/deploy-terra-local.sh
```

**Expected Output:**
- Contract address stored
- Operator configured
- Withdraw delay set

**Acceptance Criteria:**
- [x] Terra contract deployed to LocalTerra
- [x] `TERRA_BRIDGE_ADDRESS` set in environment
- [x] Contract responds to queries (`config`, `withdraw_delay`)

### Priority 2: Configure Bridge Cross-Chain Connections

Both bridges need to know about each other.

#### 2.1 Register Terra Chain on EVM Bridge

```bash
cast send $EVM_BRIDGE_ADDRESS "addChain(bytes32)" $TERRA_CHAIN_KEY --rpc-url $EVM_RPC_URL
```

#### 2.2 Register EVM Chain on Terra Bridge

```bash
terrad tx wasm execute $TERRA_BRIDGE '{"add_chain":{"chain_id":31337,"name":"Anvil"}}' --from test1 ...
```

**Acceptance Criteria:**
- [x] EVM bridge recognizes Terra chain key
- [x] Terra bridge recognizes EVM chain (31337)
- [x] Cross-chain queries work

### Priority 3: Full E2E Transfer Tests

Complete the transfer tests that were blocked by LocalTerra issues.

#### 3.1 EVM → Terra Transfer

1. Deposit on EVM (lock tokens)
2. Operator detects deposit event
3. Operator calls `ApproveWithdraw` on Terra
4. Wait for delay
5. Operator calls `ExecuteWithdraw` on Terra
6. Verify recipient received funds

#### 3.2 Terra → EVM Transfer

1. Lock on Terra
2. Operator detects lock event
3. Operator calls `approveWithdraw` on EVM
4. Wait for delay
5. Execute withdrawal on EVM
6. Verify recipient received tokens

**Acceptance Criteria:**
- [ ] EVM → Terra transfer completes successfully
- [ ] Terra → EVM transfer completes successfully
- [ ] Balances update correctly on both chains
- [ ] Events emitted and logged

### Priority 4: Canceler E2E Verification

Test the canceler's ability to detect and cancel fraudulent approvals.

#### 4.1 Fraudulent Approval Detection

1. Create approval with invalid source deposit
2. Start canceler
3. Verify canceler detects mismatch
4. Verify cancel transaction submitted
5. Verify withdrawal blocked

#### 4.2 Valid Approval Passthrough

1. Create valid deposit on source chain
2. Create corresponding approval on destination
3. Start canceler
4. Verify canceler does NOT cancel
5. Verify withdrawal succeeds after delay

**Acceptance Criteria:**
- [ ] Canceler detects invalid approvals
- [ ] Canceler submits cancel transactions
- [ ] Valid approvals are not cancelled
- [ ] E2E cancellation flow works on both chains

---

## Technical Notes

### LocalTerra Configuration

The working LocalTerra setup builds from source (see Appendix A for full details):

```yaml
# docker-compose.yml
localterra:
  # Build from classic-terra/core GitHub repository
  build:
    context: https://github.com/classic-terra/core.git#main
    dockerfile: Dockerfile
  platform: linux/amd64
  volumes:
    - ./infra/localterra/config:/root/.terra/config
    - ./infra/localterra/data:/root/.terra/data
  ports:
    - "26657:26657"  # RPC
    - "1317:1317"    # LCD
    - "9090:9090"    # gRPC
  command: terrad start
```

**Important:** No pre-built Docker images exist for classic-terra. You must build from source.

Config files are generated via `terrad init` and `terrad gentx`:
- `genesis.json` - Network genesis with test accounts and validator
- `config.toml` - CometBFT config (timeout settings)
- `app.toml` - Application config (API endpoints)
- `priv_validator_key.json` - Validator key
- `node_key.json` - Node key

### Test Accounts

LocalTerra pre-funded accounts:

| Account | Address | Use |
|---------|---------|-----|
| test1 | `terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v` | Default operator/deployer |
| validator | `terra1dcegyrekltswvyy0xy69ydgxn9x8x32zdtapd8` | Validator |

Mnemonic for test1:
```
notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius
```

### Chain IDs

| Chain | ID | Type |
|-------|-----|------|
| LocalTerra | `localterra` | Terra Classic |
| Anvil | `31337` | EVM |

---

## Quick Start for Next Agent

```bash
# 1. Start infrastructure
make start

# 2. Verify all services running
./scripts/status.sh
# Should show: Anvil (running), LocalTerra (running), PostgreSQL (running)

# 3. Deploy EVM contracts
make deploy-evm

# 4. Deploy Terra contract
./scripts/deploy-terra-local.sh

# 5. Configure cross-chain connections
./scripts/setup-bridge.sh

# 6. Run E2E tests
source .env.local  # Contains all contract addresses
./scripts/e2e-test.sh --full
```

---

## Definition of Done for Sprint 10

### Infrastructure
- [x] LocalTerra running and producing blocks
- [x] Terra contract deployed (`terra14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9ssrc8au`)
- [x] CW20-mintable deployed (`terra1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrquka9l6`)
- [x] EVM contracts deployed (`0x5FC8d32690cc91D4c39d9d3abcBD16989F875707`)
- [x] Cross-chain connections configured

### E2E Testing
- [x] EVM connectivity tests pass
- [x] Terra connectivity tests pass
- [x] EVM bridge configuration verified (300s withdraw delay)
- [x] Terra bridge configuration verified (300s withdraw delay)
- [x] Watchtower delay mechanism works
- [x] EVM watchtower approve → execute flow verified
- [x] EVM watchtower cancel flow verified
- [x] Hash parity tests pass
- [ ] Database migrations (operator tables)
- [ ] EVM → Terra transfer succeeds
- [ ] Terra → EVM transfer succeeds
- [ ] Canceler detects fraudulent approvals
- [ ] Canceler cancel flow works

### Documentation
- [x] Deploy scripts work end-to-end
- [ ] E2E test documentation updated
- [x] Troubleshooting guide for LocalTerra (See Appendix A)

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Terra contract deploy fails | Low | High | Test with cw-multi-test first |
| Block time too slow | Medium | Medium | Speed up in config.toml |
| terrad CLI missing | Medium | Low | Use docker exec |
| Gas estimation fails | Low | Medium | Use fixed gas limits |

---

## Sprint 10 Progress Report

### E2E Test Results (2026-02-02)

```
========================================
         E2E TEST SUMMARY
========================================

  Passed: 9
  Failed: 1 (database migrations only)
```

| Test | Status | Notes |
|------|--------|-------|
| EVM Connectivity | ✅ Pass | Block 503 |
| EVM Time Skip | ✅ Pass | Advanced 100s |
| EVM Bridge Configuration | ✅ Pass | 300s withdraw delay |
| Terra Connectivity | ✅ Pass | Block 2405 |
| Terra Bridge Configuration | ✅ Pass | 300s withdraw delay |
| Database Tables | ❌ Fail | Needs migrations |
| Watchtower Delay Mechanism | ✅ Pass | Time skip works |
| EVM Watchtower Approve→Execute | ✅ Pass | Full flow verified |
| EVM Watchtower Cancel Flow | ✅ Pass | Cancel mechanism verified |
| Transfer ID Hash Parity | ✅ Pass | Chain key matching works |

### Deployed Contracts

| Contract | Address | Chain |
|----------|---------|-------|
| Bridge (Terra) | `terra14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9ssrc8au` | LocalTerra |
| CW20 Token (Terra) | `terra1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrquka9l6` | LocalTerra |
| Cl8YBridge (EVM) | `0x5FC8d32690cc91D4c39d9d3abcBD16989F875707` | Anvil |
| ChainRegistry (EVM) | `0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512` | Anvil |
| TokenRegistry (EVM) | `0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0` | Anvil |
| AccessManager (EVM) | `0x5FbDB2315678afecb367f032d93F642f64180aa3` | Anvil |

### Remaining Work

1. **Database Migrations** - Run operator migrations to create tables
2. **Full Transfer Tests** - EVM → Terra and Terra → EVM with operator
3. **Canceler E2E** - Test fraudulent approval detection

---

## Appendix A: LocalTerra Setup Guide - The Full Journey

This appendix documents the complete process of getting LocalTerra (Terra Classic) working from scratch. This was an extremely challenging endeavor due to outdated documentation, confusion between Terra 2.0 and Terra Classic, and Docker image availability issues.

### The Challenge

**Goal:** Run a local Terra Classic blockchain for E2E testing of our cross-chain bridge.

**Initial Obstacles:**
1. Multiple conflicting LocalTerra repositories exist
2. Pre-built Docker images referenced in docs don't exist or are for wrong chain
3. Terra 2.0 (`terramoney`) vs Terra Classic (`classic-terra`) confusion
4. CosmWasm version compatibility between contracts and runtime
5. Genesis configuration requirements unclear

### What Didn't Work

#### Attempt 1: terramoney/localterra-core (Terra 2.0)
```yaml
image: terramoney/localterra-core:0.5.18
```
**Problem:** This is Terra 2.0, not Terra Classic. Different chain, incompatible genesis.

#### Attempt 2: classic-terra Docker Hub Images
```bash
docker pull classic-terra/localterra-core:latest
# Error: manifest unknown
```
**Problem:** No pre-built images exist on Docker Hub for classic-terra.

#### Attempt 3: ghcr.io Registry
```bash
docker pull ghcr.io/classic-terra/core:latest
# Error: manifest unknown
```
**Problem:** No images on GitHub Container Registry either.

### The Solution: Build from Source

The key insight was that **classic-terra/core provides a Dockerfile** that can be used to build the LocalTerra image directly from source.

#### Step 1: Update docker-compose.yml to Build from GitHub

```yaml
localterra:
  # Build from classic-terra/core GitHub repository
  build:
    context: https://github.com/classic-terra/core.git#main
    dockerfile: Dockerfile
  platform: linux/amd64
  ports:
    - "26657:26657"  # RPC
    - "1317:1317"    # LCD/REST
    - "9090:9090"    # gRPC
    - "9091:9091"    # gRPC-web
  volumes:
    - ./infra/localterra/config:/root/.terra/config
    - ./infra/localterra/data:/root/.terra/data
  healthcheck:
    test: ["CMD", "curl", "-sf", "http://localhost:26657/status"]
    interval: 10s
    timeout: 10s
    retries: 30
    start_period: 60s
  command: terrad start
```

**Key insight:** Docker Compose can build directly from a Git URL with `context: https://github.com/...git#branch`.

#### Step 2: Build the Image (~5 minutes)

```bash
docker compose build localterra
```

This builds:
- Go 1.23 environment
- wasmvm v3.0.2 (CosmWasm 1.5.x compatible)
- terrad binary with all Terra Classic modules

#### Step 3: Initialize the Chain

Unlike pre-configured images, a fresh build requires chain initialization:

```bash
# Clean any existing data
docker run --rm -v "$(pwd)/infra/localterra:/data" alpine sh -c \
  "rm -rf /data/config/* /data/data/*"

# Initialize chain and set up genesis
TEST_MNEMONIC="notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius"

docker run --rm \
  -v "$(pwd)/infra/localterra/config:/root/.terra/config" \
  -v "$(pwd)/infra/localterra/data:/root/.terra/data" \
  cl8y-bridge-monorepo-localterra \
  sh -c "
    terrad init localterra --chain-id localterra && \
    echo '$TEST_MNEMONIC' | terrad keys add test1 --recover --keyring-backend test && \
    terrad add-genesis-account test1 1000000000000uluna,10000000000000uusd,10000000000000ukrw,10000000000000usdr,10000000000000ueur,100000000000stake --keyring-backend test && \
    terrad gentx test1 10000000stake --chain-id localterra --keyring-backend test && \
    terrad collect-gentxs
  "
```

**What this does:**
1. `terrad init` - Creates default config files and genesis
2. `terrad keys add --recover` - Imports the test mnemonic
3. `terrad add-genesis-account` - Funds the test account with LUNC, USTC, and other tokens
4. `terrad gentx` - Creates validator genesis transaction
5. `terrad collect-gentxs` - Finalizes genesis with validator

#### Step 4: Configure for Fast Block Times

Edit `infra/localterra/config/config.toml`:
```toml
timeout_propose = "200ms"  # Was 3s
timeout_commit = "200ms"   # Was 5s
```

Edit `infra/localterra/config/config.toml`:
```toml
# RPC server binding (default is 127.0.0.1, need 0.0.0.0 for Docker)
laddr = "tcp://0.0.0.0:26657"
```

Edit `infra/localterra/config/app.toml`:
```toml
[api]
enable = true
address = "tcp://0.0.0.0:1317"  # Expose externally

[grpc]
enable = true
address = "0.0.0.0:9090"
```

**Note:** Files created by Docker are owned by root. Use Docker to edit:
```bash
docker run --rm -v "$(pwd)/infra/localterra:/data" alpine sh -c "
  chmod -R 777 /data/config /data/data && \
  sed -i 's/timeout_propose = \"3s\"/timeout_propose = \"200ms\"/g' /data/config/config.toml && \
  sed -i 's/timeout_commit = \"5s\"/timeout_commit = \"200ms\"/g' /data/config/config.toml && \
  sed -i 's/enable = false/enable = true/g' /data/config/app.toml && \
  sed -i 's/address = \"tcp:\/\/localhost:1317\"/address = \"tcp:\/\/0.0.0.0:1317\"/g' /data/config/app.toml
"
```

#### Step 5: Start LocalTerra

```bash
docker compose up -d localterra
```

#### Step 6: Verify It's Working

```bash
# Check block production (should increase every ~200ms)
curl -sf "http://localhost:1317/cosmos/base/tendermint/v1beta1/blocks/latest" | jq '.block.header.height'

# Check test1 balance
curl -sf "http://localhost:1317/cosmos/bank/v1beta1/balances/terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v" | jq '.balances'
```

**Expected output:**
```json
[
  {"denom":"stake","amount":"99990000000"},
  {"denom":"ueur","amount":"10000000000000"},
  {"denom":"ukrw","amount":"10000000000000"},
  {"denom":"uluna","amount":"1000000000000"},
  {"denom":"usdr","amount":"10000000000000"},
  {"denom":"uusd","amount":"10000000000000"}
]
```

### Deploying Contracts

#### Build with CosmWasm Optimizer

```bash
cd packages/contracts-terraclassic
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename $(pwd))_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.16.0
```

**Output:**
- `artifacts/bridge.wasm` (~462KB optimized)
- `artifacts/cw20_mintable.wasm` (~420KB optimized)

#### Deploy via Docker Exec

Since `terrad` CLI is inside the container, use `docker exec`:

```bash
CONTAINER="cl8y-bridge-monorepo-localterra-1"

# Copy WASM to container
docker cp artifacts/bridge.wasm "$CONTAINER:/tmp/bridge.wasm"

# Store contract (note: high fees required)
docker exec "$CONTAINER" terrad --keyring-backend test tx wasm store /tmp/bridge.wasm \
  --from test1 --chain-id localterra \
  --gas auto --gas-adjustment 1.5 \
  --fees 200000000uluna \
  --broadcast-mode sync -y

# Wait for confirmation
sleep 5

# Get code ID
docker exec "$CONTAINER" terrad query wasm list-code -o json | jq '.code_infos[-1].code_id'
# Output: "1"

# Instantiate bridge
INIT_MSG='{"admin":"terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v","operators":["terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"],"min_signatures":1,"min_bridge_amount":"1000000","max_bridge_amount":"1000000000000000","fee_bps":30,"fee_collector":"terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"}'

docker exec "$CONTAINER" terrad --keyring-backend test tx wasm instantiate 1 "$INIT_MSG" \
  --label "cl8y-bridge-local" \
  --admin "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v" \
  --from test1 --chain-id localterra \
  --gas auto --gas-adjustment 1.5 \
  --fees 50000000uluna \
  --broadcast-mode sync -y

# Get contract address
docker exec "$CONTAINER" terrad query wasm list-contract-by-code 1 -o json | jq -r '.contracts[-1]'
# Output: terra14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9ssrc8au
```

### Key Learnings

#### 1. Terra Classic Uses Modern CosmWasm (1.5.x)

Despite being "classic", Terra Classic runs wasmvm v3 and supports CosmWasm 1.5.x. The `cw20-mintable` contract from mainnet uses these versions:
```toml
cosmwasm-std = "1.5.11"
cosmwasm-schema = "1.5.9"
cw-storage-plus = "1.2.0"
```

#### 2. Gas Fees Are High

Terra Classic has complex gas pricing. For WASM store operations:
- ~4 million gas required for ~450KB contract
- Fees must be ~200,000,000 uluna (0.2 LUNC) or equivalent

#### 3. No Pre-built Docker Images

Unlike many chains, classic-terra doesn't publish Docker images. You must build from source. The Dockerfile in `classic-terra/core` is well-maintained and builds cleanly.

#### 4. terrad Command Syntax

The newer terrad (from classic-terra/core main branch) uses slightly different command syntax:
- `terrad add-genesis-account` (not `terrad genesis add-genesis-account`)
- `terrad gentx` (not `terrad genesis gentx`)
- `terrad collect-gentxs` (not `terrad genesis collect-gentxs`)

#### 5. Config File Permissions

Docker creates files as root. When mounting config directories, you may need to fix permissions before the host can edit them.

### Final Working State

| Component | Status | Details |
|-----------|--------|---------|
| LocalTerra | ✅ Running | Block time ~200ms |
| Bridge Contract | ✅ Deployed | `terra14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9ssrc8au` |
| CW20 Token | ✅ Deployed | `terra1nc5tatafv6eyq7llkr2gv50ff9e22mnf70qgjlv737ktmt4eswrquka9l6` |
| EVM Bridge | ✅ Deployed | `0x5FC8d32690cc91D4c39d9d3abcBD16989F875707` |
| Cross-chain Config | ✅ Done | Both bridges aware of each other |

### Troubleshooting

#### "manifest unknown" when pulling image
**Solution:** Build from source using `build: context: https://github.com/classic-terra/core.git#main`

#### "insufficient fees" error
**Solution:** Increase fees significantly. For WASM store: `--fees 200000000uluna`

#### "priv_validator_state.json not found" 
**Solution:** Run `terrad init` before starting the node, or create the file manually:
```bash
echo '{"height":"0","round":0,"step":0}' > infra/localterra/data/priv_validator_state.json
```

#### Contract queries fail with empty response
**Solution:** Wait for a few blocks after instantiation. Query the LCD API:
```bash
curl "http://localhost:1317/cosmwasm/wasm/v1/contract/ADDR/smart/$(echo -n '{"config":{}}' | base64 -w0)"
```

#### Permission denied editing config files
**Solution:** Use Docker to chmod or edit:
```bash
docker run --rm -v "$(pwd)/infra/localterra:/data" alpine chmod -R 777 /data/config
```

---

*Created: 2026-02-02*
*Previous Sprint: SPRINT9.md - Terra Classic Watchtower Implementation*
