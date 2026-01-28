# Sprint 3: Terra Classic Integration & End-to-End Testing

**Sprint Duration**: Estimated 2-3 sessions  
**Prerequisites**: Sprint 2 completed (relayer writers, EVM contracts deployed)  
**Handoff Date**: 2026-01-28

---

## Sprint 2 Summary (Completed)

### What Was Built
- **Complete EVM Writer** (`writers/evm.rs`)
  - Full `approveWithdraw()` transaction submission
  - Chain key computation, withdraw hash, fee calculation
  
- **Complete Terra Writer** (`writers/terra.rs`)
  - Full `Release` message submission
  - BIP39 mnemonic key derivation, LCD API broadcast
  
- **Contract ABI/Messages** (`contracts/`)
  - EVM bridge ABI via alloy `sol!` macro
  - Terra bridge message types for CosmWasm

- **Error Handling & Retry Logic**
  - `RetryConfig` with exponential backoff
  - Circuit breaker for consecutive failures

- **EVM Integration Verified**
  - Anvil running, contracts deployed
  - Relayer connects to DB and EVM

### What's Missing
- LocalTerra Classic setup
- Terra contract deployment
- Cross-chain configuration (both sides need to know about each other)
- End-to-end transfer testing

---

## Sprint 3 Goals

### Primary Objective
**Establish bidirectional communication between EVM and Terra Classic, completing the full bridge flow.**

### Deliverables

#### 1. LocalTerra Classic Environment
**Priority: HIGH | Complexity: MEDIUM**

Set up a local Terra Classic node for development and testing.

**Option A: Pre-built Image (Quick Start)**
```bash
# Start with older LocalTerra image
docker compose up -d localterra

# Note: May have compatibility issues
```

**Option B: Build from Source (Recommended)**
```bash
# Clone Terra Classic core
git clone https://github.com/classic-terra/core.git ../terra-classic-core
cd ../terra-classic-core

# Build and run localnet
make install
make localnet-start

# Default endpoints:
# RPC: http://localhost:26657
# LCD: http://localhost:1317
# gRPC: http://localhost:9090
```

**Option C: Use Docker with Classic-Terra Image**
```bash
# Build Docker image from classic-terra/core
cd ../terra-classic-core
docker build -t localterra-classic:latest .

# Run with custom command
docker run -d --name localterra \
  -p 26657:26657 -p 1317:1317 -p 9090:9090 \
  localterra-classic:latest \
  terrad start --rpc.laddr tcp://0.0.0.0:26657
```

**Verification:**
```bash
# Check node status
curl http://localhost:26657/status

# Query account (LocalTerra test accounts)
terrad query bank balances terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v \
  --node http://localhost:26657
```

---

#### 2. Build & Deploy Terra Bridge Contract
**Priority: HIGH | Complexity: MEDIUM | ~100 lines scripts**

Build the CosmWasm contract and deploy to LocalTerra.

**Build Contract:**
```bash
cd packages/contracts-terraclassic

# Install wasm target
rustup target add wasm32-unknown-unknown

# Build
cargo build --release --target wasm32-unknown-unknown

# Optimize (requires docker)
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="contracts_cache",target=/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/optimizer:0.15.0

# Output: artifacts/bridge.wasm
```

**Deploy Contract:**
```bash
# LocalTerra test mnemonic (DO NOT USE IN PRODUCTION)
# notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius

# Import key
terrad keys add testkey --recover << EOF
notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius
EOF

# Store contract
terrad tx wasm store artifacts/bridge.wasm \
  --from testkey \
  --chain-id localterra \
  --node http://localhost:26657 \
  --gas auto --gas-adjustment 1.4 \
  --fees 10000uluna \
  -y

# Get code ID from tx result
CODE_ID=$(terrad query wasm list-code --node http://localhost:26657 -o json | jq -r '.code_infos[-1].code_id')
echo "Code ID: $CODE_ID"

# Instantiate
terrad tx wasm instantiate $CODE_ID \
  '{
    "admin": "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    "relayers": ["terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"],
    "min_signatures": 1,
    "min_bridge_amount": "1000000",
    "max_bridge_amount": "1000000000000000",
    "fee_bps": 30,
    "fee_collector": "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"
  }' \
  --label "cl8y-bridge-v1" \
  --admin terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v \
  --from testkey \
  --chain-id localterra \
  --node http://localhost:26657 \
  --gas auto --gas-adjustment 1.4 \
  --fees 10000uluna \
  -y

# Get contract address
TERRA_BRIDGE=$(terrad query wasm list-contract-by-code $CODE_ID --node http://localhost:26657 -o json | jq -r '.contracts[-1]')
echo "Terra Bridge: $TERRA_BRIDGE"
```

---

#### 3. Configure Cross-Chain Registration
**Priority: HIGH | Complexity: LOW | ~50 lines scripts**

Both bridges need to know about each other and supported tokens.

**On Terra Side - Register EVM Chain:**
```bash
# Add Anvil (chain ID 31337) as supported chain
terrad tx wasm execute $TERRA_BRIDGE \
  '{
    "add_chain": {
      "chain_id": 31337,
      "name": "Anvil Local",
      "bridge_address": "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"
    }
  }' \
  --from testkey \
  --chain-id localterra \
  --node http://localhost:26657 \
  --gas auto --gas-adjustment 1.4 \
  --fees 5000uluna \
  -y

# Add uluna as supported token
terrad tx wasm execute $TERRA_BRIDGE \
  '{
    "add_token": {
      "token": "uluna",
      "is_native": true,
      "evm_token_address": "0x0000000000000000000000000000000000001234",
      "terra_decimals": 6,
      "evm_decimals": 18
    }
  }' \
  --from testkey \
  --chain-id localterra \
  --node http://localhost:26657 \
  --gas auto --gas-adjustment 1.4 \
  --fees 5000uluna \
  -y
```

**On EVM Side - Register Terra Chain:**
```bash
cd packages/contracts-evm

# Compute Terra chain key: keccak256("COSMOS", "localterra", "terra")
TERRA_CHAIN_KEY=$(cast keccak "$(cast abi-encode 'f(string,string,string)' 'COSMOS' 'localterra' 'terra')")
echo "Terra Chain Key: $TERRA_CHAIN_KEY"

# Register Terra chain in ChainRegistry
cast send 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512 \
  "registerChain(bytes32,uint8,string)" \
  $TERRA_CHAIN_KEY \
  2 \
  "Terra Classic Local" \
  --rpc-url http://localhost:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

---

#### 4. Update Relayer Configuration
**Priority: HIGH | Complexity: LOW**

Update `.env` with Terra contract address:

```bash
# packages/relayer/.env
DATABASE_URL=postgres://relayer:relayer@localhost:5433/relayer

# EVM Configuration (Anvil)
EVM_RPC_URL=http://localhost:8545
EVM_CHAIN_ID=31337
EVM_BRIDGE_ADDRESS=0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
EVM_ROUTER_ADDRESS=0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
EVM_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Terra Configuration (LocalTerra)
TERRA_RPC_URL=http://localhost:26657
TERRA_LCD_URL=http://localhost:1317
TERRA_CHAIN_ID=localterra
TERRA_BRIDGE_ADDRESS=terra1...  # Fill in deployed address
TERRA_MNEMONIC=notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius

# Relayer Settings
POLL_INTERVAL_MS=1000
FINALITY_BLOCKS=1
RETRY_ATTEMPTS=3
RETRY_DELAY_MS=2000
DEFAULT_FEE_BPS=30
FEE_RECIPIENT=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

---

#### 5. End-to-End Transfer Testing
**Priority: HIGH | Complexity: HIGH**

##### Test A: Terra → EVM Transfer

1. **Lock tokens on Terra:**
```bash
# Send 1 LUNA from Terra to EVM address
terrad tx wasm execute $TERRA_BRIDGE \
  '{
    "lock": {
      "dest_chain_id": 31337,
      "recipient": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    }
  }' \
  --amount 1000000uluna \
  --from testkey \
  --chain-id localterra \
  --node http://localhost:26657 \
  --gas auto --gas-adjustment 1.4 \
  --fees 5000uluna \
  -y

# Verify lock event
terrad query tx <TX_HASH> --node http://localhost:26657 -o json | \
  jq '.logs[0].events[] | select(.type=="wasm")'
```

2. **Run relayer (watches Terra, writes to EVM):**
```bash
cd packages/relayer
cargo run

# Expected logs:
# INFO Processing Terra block
# INFO Stored Terra lock transaction
# INFO Processing Terra deposit for EVM approval
# INFO Approval submitted successfully
```

3. **Verify approval on EVM:**
```bash
# Query pending withdrawals on bridge
cast call 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707 \
  "withdrawals(bytes32)(bool,address,uint256,uint256,bool,bool)" \
  <WITHDRAW_HASH> \
  --rpc-url http://localhost:8545
```

##### Test B: EVM → Terra Transfer

1. **Deposit tokens on EVM:**
```bash
# First, we need a test ERC20 token
# For now, use native ETH via router (if router deployed)
# Or directly call bridge deposit function

cast send 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707 \
  "deposit(address,uint256,bytes32,bytes32)" \
  0x0000000000000000000000000000000000001234 \  # token
  1000000000000000000 \                           # 1 token (18 decimals)
  $TERRA_CHAIN_KEY \                              # dest chain
  0x7465727261317836366e...00000000000000000000 \ # dest account (padded terra address)
  --rpc-url http://localhost:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --value 0
```

2. **Relayer watches EVM, writes to Terra:**
```bash
# Expected logs:
# INFO Processing EVM block
# INFO Stored EVM deposit event
# INFO Processing EVM deposit for Terra release
# INFO Release submitted successfully
```

3. **Verify release on Terra:**
```bash
terrad query bank balances terra1... --node http://localhost:26657
```

---

#### 6. Deployment Scripts
**Priority: MEDIUM | Complexity: LOW | ~200 lines**

Create automated deployment scripts:

**`scripts/deploy-terra-local.sh`:**
```bash
#!/bin/bash
set -e

# Configuration
CHAIN_ID="localterra"
NODE="http://localhost:26657"
KEY_NAME="testkey"
WASM_PATH="../packages/contracts-terraclassic/artifacts/bridge.wasm"

# Store contract
echo "Storing contract..."
TX=$(terrad tx wasm store $WASM_PATH \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --node $NODE \
  --gas auto --gas-adjustment 1.4 \
  --fees 50000uluna \
  -y -o json)

TX_HASH=$(echo $TX | jq -r '.txhash')
echo "Store TX: $TX_HASH"
sleep 6

# Get code ID
CODE_ID=$(terrad query wasm list-code --node $NODE -o json | jq -r '.code_infos[-1].code_id')
echo "Code ID: $CODE_ID"

# Get admin address
ADMIN=$(terrad keys show $KEY_NAME -a)

# Instantiate
echo "Instantiating..."
INIT_MSG=$(cat << EOF
{
  "admin": "$ADMIN",
  "relayers": ["$ADMIN"],
  "min_signatures": 1,
  "min_bridge_amount": "1000000",
  "max_bridge_amount": "1000000000000000",
  "fee_bps": 30,
  "fee_collector": "$ADMIN"
}
EOF
)

TX=$(terrad tx wasm instantiate $CODE_ID "$INIT_MSG" \
  --label "cl8y-bridge-local" \
  --admin $ADMIN \
  --from $KEY_NAME \
  --chain-id $CHAIN_ID \
  --node $NODE \
  --gas auto --gas-adjustment 1.4 \
  --fees 50000uluna \
  -y -o json)

TX_HASH=$(echo $TX | jq -r '.txhash')
echo "Instantiate TX: $TX_HASH"
sleep 6

# Get contract address
CONTRACT=$(terrad query wasm list-contract-by-code $CODE_ID --node $NODE -o json | jq -r '.contracts[-1]')
echo "================================"
echo "TERRA_BRIDGE_ADDRESS=$CONTRACT"
echo "================================"
```

---

#### 7. Integration Test Suite
**Priority: MEDIUM | Complexity: MEDIUM | ~150 lines**

Create Rust integration tests in `packages/relayer/tests/`:

**`packages/relayer/tests/integration_test.rs`:**
```rust
//! Integration tests for cross-chain transfers
//! 
//! Run with: cargo test --test integration_test -- --nocapture
//! 
//! Prerequisites:
//! - Anvil running on localhost:8545
//! - LocalTerra running on localhost:26657
//! - Contracts deployed and configured
//! - DATABASE_URL set

use std::env;

#[tokio::test]
#[ignore] // Run manually with --ignored
async fn test_terra_to_evm_transfer() {
    // 1. Lock tokens on Terra
    // 2. Poll until relayer processes
    // 3. Verify approval exists on EVM
    todo!("Implement integration test")
}

#[tokio::test]
#[ignore]
async fn test_evm_to_terra_transfer() {
    // 1. Deposit tokens on EVM
    // 2. Poll until relayer processes
    // 3. Verify release on Terra
    todo!("Implement integration test")
}
```

---

## Technical Notes

### LocalTerra Test Accounts

Default LocalTerra provides test accounts with funds:

| Name | Address | Mnemonic |
|------|---------|----------|
| test1 | terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v | `notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius` |
| test2 | terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp | (different mnemonic) |

### Chain Key Computation

```rust
// Terra Classic chain key
ChainKey::cosmos("localterra", "terra")
// = keccak256(abi.encode("COSMOS", "localterra", "terra"))

// Anvil chain key
ChainKey::evm(31337)
// = keccak256(abi.encode("EVM", 31337))
```

### Address Encoding

When bridging:
- EVM → Terra: EVM addresses stored as hex strings
- Terra → EVM: Terra addresses encoded as bytes32 (left-padded with zeros)

### Amount Conversion

| Terra | EVM | Notes |
|-------|-----|-------|
| 1 LUNA = 1,000,000 uluna | 1 wLUNA = 10^18 wei | 6 → 18 decimals |

---

## Risk Items

| Risk | Mitigation |
|------|------------|
| LocalTerra build issues | Use pre-built Docker image as fallback |
| Cosmos SDK version mismatch | Pin to specific classic-terra/core commit |
| Key derivation differences | Verify HD path matches Terra Classic standard |
| Nonce tracking across restarts | Store last processed height in DB |
| LCD API rate limits | Add backoff, use RPC for high-frequency queries |

---

## Definition of Done

Sprint 3 is complete when:
- [ ] LocalTerra Classic running locally
- [ ] Terra bridge contract built and deployed
- [ ] Both bridges configured with each other's addresses
- [ ] Relayer successfully watches both chains
- [ ] Terra → EVM transfer works end-to-end
- [ ] EVM → Terra transfer works end-to-end
- [ ] Deployment scripts created and tested
- [ ] All tests pass (`cargo test`)
- [ ] Documentation updated

---

## Quick Start for Next Session

```bash
# 1. Start infrastructure
docker compose up -d anvil postgres

# 2. Deploy EVM contracts (if not already)
cd packages/contracts-evm
forge script script/DeployLocal.s.sol:DeployLocal --broadcast --rpc-url http://localhost:8545

# 3. Build and start LocalTerra
cd ../terra-classic-core
make localnet-start

# 4. Build and deploy Terra contract
cd ../packages/contracts-terraclassic
cargo build --release --target wasm32-unknown-unknown
# ... deploy steps ...

# 5. Configure bridges (add chains, tokens)
# ... execute messages ...

# 6. Run relayer
cd packages/relayer
cp .env.example .env
# Update TERRA_BRIDGE_ADDRESS in .env
cargo run
```

---

## Stretch Goals

1. **Automated E2E Test Script**
   - Single script that runs full transfer cycle
   - Verifies balances before/after

2. **Multiple Token Support**
   - Add USTC alongside LUNC
   - Test with CW20 tokens

3. **Metrics Dashboard**
   - Prometheus metrics from relayer
   - Grafana dashboard for monitoring

4. **Multi-Relayer Setup**
   - Run 3 relayer instances
   - Test signature aggregation

---

Good luck! The EVM side is proven working - now connect it to Terra.
