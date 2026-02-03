# Sprint 16: E2E Full Integration Test Suite

## Overview

**Goal**: Transform the Rust E2E package from an infrastructure verification suite into a complete integration test suite that replaces all bash script functionality.

**Current State**: The Rust E2E package (`packages/e2e`) can:
- Start/stop Docker services
- Verify service health (Anvil, Terra, PostgreSQL)
- Deploy EVM contracts via forge
- Run infrastructure validation tests

**Target State**: Full integration testing with:
- Real token transfers with balance verification
- Terra contract deployment
- Operator/Canceler service management
- Fraud detection and cancellation tests
- Time manipulation for watchtower delay testing

---

## Gap Analysis Summary

### What's Been Migrated (Sprint 14-15)

| Component | Status |
|-----------|--------|
| Docker orchestration | ✅ Complete |
| Service health checks | ✅ Complete |
| EVM contract deployment | ✅ Complete (via forge) |
| Infrastructure tests | ✅ Complete |
| CLI interface | ✅ Complete |
| Teardown with options | ✅ Complete |

### What's Missing

| Component | Priority | Complexity |
|-----------|----------|------------|
| Terra contract deployment | P0 | High |
| Real token transfers | P0 | High |
| Time manipulation (evm_increaseTime) | P1 | Low |
| Balance tracking | P1 | Medium |
| Token/Chain registration | P1 | Medium |
| Operator service management | P2 | Medium |
| Canceler service management | P2 | Medium |
| Fraud detection tests | P2 | High |
| Test token deployment | P2 | Medium |

---

## Sprint 16 Tasks

### Task 1: Terra Contract Deployment (P0)

**Location**: `packages/e2e/src/terra.rs`

The bash script `e2e-setup.sh` deploys Terra contracts via `docker exec terrad`. We need to replicate this in Rust.

**Required functionality**:
1. Store WASM code (`terrad tx wasm store`)
2. Instantiate bridge contract (`terrad tx wasm instantiate`)
3. Configure withdraw delay (`terrad tx wasm execute`)
4. Wait for TX confirmation

**Reference bash code** (from `scripts/e2e-setup.sh:274-435`):

```bash
# Store WASM
docker exec "$CONTAINER_NAME" terrad tx wasm store /tmp/wasm/bridge.wasm \
    --from test1 --chain-id localterra --gas auto --gas-adjustment 1.5 \
    --fees 200000000uluna --broadcast-mode sync -y -o json --keyring-backend test

# Instantiate
local INIT_MSG='{"admin":"'$TEST_ADDRESS'","operators":["'$TEST_ADDRESS'"],...}'
docker exec "$CONTAINER_NAME" terrad tx wasm instantiate "$CODE_ID" "$INIT_MSG" \
    --label "cl8y-bridge-e2e" --admin "$TEST_ADDRESS" --from test1 ...

# Configure delay
local SET_DELAY_MSG='{"set_withdraw_delay":{"delay_seconds":300}}'
docker exec "$CONTAINER_NAME" terrad tx wasm execute "$TERRA_BRIDGE_ADDRESS" "$SET_DELAY_MSG" ...
```

**Rust implementation approach**:

```rust
// In terra.rs, add these methods to TerraClient

impl TerraClient {
    /// Store WASM code on LocalTerra
    pub async fn store_wasm(&self, wasm_path: &str) -> Result<u64> {
        // 1. Copy WASM to container
        self.docker_exec(&["mkdir", "-p", "/tmp/wasm"]).await?;
        self.docker_cp(wasm_path, "/tmp/wasm/bridge.wasm").await?;
        
        // 2. Store code
        let output = self.terrad_tx(&[
            "wasm", "store", "/tmp/wasm/bridge.wasm",
            "--from", "test1",
            "--chain-id", "localterra",
            "--gas", "auto", "--gas-adjustment", "1.5",
            "--fees", "200000000uluna",
            "--broadcast-mode", "sync",
            "-y", "-o", "json", "--keyring-backend", "test"
        ]).await?;
        
        // 3. Wait for confirmation and extract code_id
        let tx_hash = extract_tx_hash(&output)?;
        self.wait_for_tx(&tx_hash).await?;
        self.get_latest_code_id().await
    }
    
    /// Instantiate bridge contract
    pub async fn instantiate_bridge(&self, code_id: u64, admin: &str) -> Result<String> {
        let init_msg = serde_json::json!({
            "admin": admin,
            "operators": [admin],
            "min_signatures": 1,
            "min_bridge_amount": "1000000",
            "max_bridge_amount": "1000000000000000",
            "fee_bps": 30,
            "fee_collector": admin
        });
        
        let output = self.terrad_tx(&[
            "wasm", "instantiate", &code_id.to_string(),
            &init_msg.to_string(),
            "--label", "cl8y-bridge-e2e",
            "--admin", admin,
            "--from", "test1",
            // ... rest of args
        ]).await?;
        
        let tx_hash = extract_tx_hash(&output)?;
        self.wait_for_tx(&tx_hash).await?;
        self.get_contract_by_code(code_id).await
    }
    
    /// Set withdraw delay on bridge
    pub async fn set_withdraw_delay(&self, bridge_addr: &str, delay_seconds: u64) -> Result<()> {
        let msg = serde_json::json!({
            "set_withdraw_delay": { "delay_seconds": delay_seconds }
        });
        
        self.execute_contract(bridge_addr, &msg, None).await
    }
}
```

---

### Task 2: EVM Time Manipulation (P1)

**Location**: `packages/e2e/src/evm.rs`

Add ability to skip time on Anvil for testing watchtower delays.

```rust
impl EvmClient {
    /// Skip time on Anvil (for testing watchtower delays)
    pub async fn increase_time(&self, seconds: u64) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "evm_increaseTime",
                "params": [seconds],
                "id": 1
            }))
            .send()
            .await?;
        
        // Also mine a block to apply the time change
        client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "evm_mine",
                "params": [],
                "id": 2
            }))
            .send()
            .await?;
        
        Ok(())
    }
}
```

---

### Task 3: Real Token Transfer Tests (P0)

**Location**: `packages/e2e/src/tests.rs`

**New test functions needed**:

```rust
/// Execute a real EVM → Terra transfer with balance verification
pub async fn test_real_evm_to_terra_transfer(config: &E2eConfig) -> TestResult {
    // 1. Get initial balances
    let evm_balance_before = get_erc20_balance(config, token, account).await?;
    
    // 2. Approve token spend on LockUnlock adapter
    approve_erc20(config, token, spender, amount).await?;
    
    // 3. Execute deposit via BridgeRouter
    let deposit_tx = execute_deposit(config, token, amount, dest_chain, recipient).await?;
    
    // 4. Verify deposit nonce incremented
    let nonce_after = query_deposit_nonce(config).await?;
    
    // 5. Verify EVM balance decreased
    let evm_balance_after = get_erc20_balance(config, token, account).await?;
    assert!(evm_balance_before - evm_balance_after >= amount);
    
    // 6. If operator running, wait for Terra approval
    // ...
    
    TestResult::passed("Real EVM→Terra transfer completed")
}

/// Execute a real Terra → EVM transfer with balance verification  
pub async fn test_real_terra_to_evm_transfer(config: &E2eConfig) -> TestResult {
    // 1. Get initial Terra balance
    let terra_balance_before = query_terra_balance(config, account, "uluna").await?;
    
    // 2. Execute lock on Terra bridge
    let lock_msg = json!({
        "lock": {
            "dest_chain_id": 31337,
            "recipient": evm_recipient
        }
    });
    execute_terra_contract(config, bridge, &lock_msg, Some("1000000uluna")).await?;
    
    // 3. Verify Terra balance decreased
    let terra_balance_after = query_terra_balance(config, account, "uluna").await?;
    
    // 4. Skip time on Anvil for watchtower delay
    increase_time(config, 310).await?;  // 300s delay + buffer
    
    // 5. If operator running, verify EVM approval created
    // ...
    
    TestResult::passed("Real Terra→EVM transfer completed")
}
```

---

### Task 4: Token and Chain Registration (P1)

**Location**: `packages/e2e/src/deploy.rs`

Add functions to register tokens and chains after deployment:

```rust
impl ContractDeployer {
    /// Register Terra chain key on ChainRegistry
    pub async fn register_terra_chain(&self, chain_registry: &str) -> Result<String> {
        // cast send ChainRegistry "addCOSMWChainKey(string)" "localterra"
        let output = Command::new("cast")
            .args([
                "send", chain_registry,
                "addCOSMWChainKey(string)",
                "localterra",
                "--rpc-url", &self.rpc_url,
                "--private-key", &self.private_key,
                "--json"
            ])
            .output()
            .await?;
        
        // Get computed chain key
        let chain_key = self.call_contract(
            chain_registry,
            "getChainKeyCOSMW(string)(bytes32)",
            &["localterra"]
        ).await?;
        
        Ok(chain_key)
    }
    
    /// Register token on TokenRegistry with destination chain
    pub async fn register_token(
        &self,
        token_registry: &str,
        token: &str,
        bridge_type: u8,  // 0 = MintBurn, 1 = LockUnlock
        dest_chain_key: &str,
        dest_token: &str,
        decimals: u8
    ) -> Result<()> {
        // Step 1: Add token
        self.send_tx(token_registry, "addToken(address,uint8)", &[token, &bridge_type.to_string()]).await?;
        
        // Step 2: Add destination chain key
        self.send_tx(
            token_registry,
            "addTokenDestChainKey(address,bytes32,bytes32,uint256)",
            &[token, dest_chain_key, dest_token, &decimals.to_string()]
        ).await?;
        
        Ok(())
    }
}
```

---

### Task 5: Operator/Canceler Service Management (P2)

**Location**: New file `packages/e2e/src/services.rs`

```rust
use std::process::{Command, Child};
use std::fs;
use std::path::Path;

pub struct ServiceManager {
    project_root: String,
}

impl ServiceManager {
    /// Start the operator service
    pub async fn start_operator(&self, config: &E2eConfig) -> Result<u32> {
        let mut cmd = Command::new("cargo")
            .args(["run", "-p", "cl8y-operator", "--release", "--"])
            .env("DATABASE_URL", &config.database_url)
            .env("EVM_RPC_URL", &config.evm_rpc_url)
            .env("EVM_BRIDGE_ADDRESS", &config.evm_bridge_address)
            // ... other env vars
            .spawn()?;
        
        let pid = cmd.id();
        fs::write(format!("{}/.operator.pid", self.project_root), pid.to_string())?;
        
        // Wait for health check
        self.wait_for_operator_health().await?;
        
        Ok(pid)
    }
    
    /// Stop the operator service
    pub async fn stop_operator(&self) -> Result<()> {
        let pid_file = format!("{}/.operator.pid", self.project_root);
        if Path::new(&pid_file).exists() {
            let pid: i32 = fs::read_to_string(&pid_file)?.trim().parse()?;
            unsafe { libc::kill(pid, libc::SIGTERM); }
            fs::remove_file(&pid_file)?;
        }
        Ok(())
    }
    
    /// Start the canceler service
    pub async fn start_canceler(&self, config: &E2eConfig) -> Result<u32> {
        // Similar to operator
    }
    
    /// Stop the canceler service
    pub async fn stop_canceler(&self) -> Result<()> {
        // Similar to operator
    }
}
```

---

### Task 6: Fraud Detection Tests (P2)

**Location**: `packages/e2e/src/tests.rs`

```rust
/// Test fraud detection: create fake approval, verify canceler detects and cancels it
pub async fn test_fraud_detection_full(config: &E2eConfig) -> TestResult {
    // 1. Start canceler service
    let services = ServiceManager::new(&config.project_root);
    services.start_canceler(config).await?;
    
    // 2. Create fraudulent approval (no matching deposit)
    let fraud_nonce = 999_000_000 + rand::random::<u32>() % 1000;
    let fraud_amount = "1234567890123456789";
    
    let approve_tx = send_tx(
        config,
        &config.evm_bridge_address,
        "approveWithdraw(bytes32,address,address,bytes32,uint256,uint256,uint256,address,bool)",
        &[
            fake_src_chain_key,
            fake_token,
            test_account,
            fake_dest_account,
            fraud_amount,
            &fraud_nonce.to_string(),
            "0",
            ZERO_ADDRESS,
            "false"
        ]
    ).await?;
    
    // 3. Wait for canceler to detect and cancel
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    // 4. Verify approval was cancelled
    let is_cancelled = query_is_cancelled(config, withdraw_hash).await?;
    
    // 5. Stop canceler
    services.stop_canceler().await?;
    
    if is_cancelled {
        TestResult::passed("Fraud detection working - approval cancelled")
    } else {
        TestResult::failed("Canceler did not cancel fraudulent approval")
    }
}
```

---

## WorkSplit Guidance for Next Agent

### CRITICAL: Read These Files First

Before creating ANY WorkSplit job:

1. **Read manager instructions**: `packages/e2e/jobs/_manager_instruction.md`
2. **Read workspace rules**: `.cursor/rules/worksplit-best-practices.mdc`

### WorkSplit Command Reference

```bash
# Create a new job
worksplit new-job <name> -t replace -o <output_dir> -f <output_file>

# Run ALL jobs (correct way)
worksplit run

# Run single job (ONLY for debugging failures)
worksplit run --job <job_name>
```

### Job Sizing Rules

| Lines of Change | Approach |
|-----------------|----------|
| < 50 lines | Edit manually, don't use WorkSplit |
| 50-200 lines | Either approach |
| 200-400 lines | Use WorkSplit, single job OK |
| 400+ lines | **MUST split into multiple jobs** |

### Mode Selection

| Situation | Use |
|-----------|-----|
| Adding new structs/functions/files | **REPLACE mode** |
| Small edits (< 10 lines) | **Manual edit** |
| Large file changes | **REPLACE mode** |
| Multiple related changes | Separate jobs with `depends_on` |

### Example: Breaking Up Complex Implementation

**BAD** - Single massive job that will timeout:

```yaml
# e2e_016_terra_deploy.md - 600 lines, will fail
```

**GOOD** - Split into focused jobs:

```yaml
# e2e_016a_terra_types.md (~100 lines)
# - Add TerraDeployResult struct
# - Add deployment config types

# e2e_016b_terra_wasm.md (~150 lines) 
# depends_on: [e2e_016a_terra_types]
# - store_wasm() function
# - docker_cp() helper

# e2e_016c_terra_instantiate.md (~150 lines)
# depends_on: [e2e_016b_terra_wasm]
# - instantiate_bridge() function
# - wait_for_tx() helper

# e2e_016d_terra_configure.md (~100 lines)
# depends_on: [e2e_016c_terra_instantiate]  
# - set_withdraw_delay() function
# - verify_config() function
```

### Running Jobs

```bash
# Create all jobs first
worksplit new-job e2e_016a_terra_types -t replace -o packages/e2e/src/ -f terra.rs
worksplit new-job e2e_016b_terra_wasm -t replace -o packages/e2e/src/ -f terra.rs
# ... etc

# Edit job files to add requirements

# Run ALL jobs at once (batched execution)
worksplit run

# If a job fails, fix it and run again
worksplit run
```

### On Failure

1. **Never retry edit mode** - switch to replace mode
2. **If job times out** - split into smaller jobs
3. **If build fails** - check dependencies, add `depends_on`
4. **Don't abandon** - get user approval before giving up

---

## File Structure Reference

```
packages/e2e/
├── Cargo.toml
├── README.md
├── jobs/                    # WorkSplit jobs (if used)
│   └── _manager_instruction.md
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Module exports
│   ├── config.rs            # Configuration
│   ├── evm.rs               # EVM client (needs: increase_time)
│   ├── terra.rs             # Terra client (needs: store_wasm, instantiate, configure)
│   ├── setup.rs             # Setup orchestration
│   ├── teardown.rs          # Teardown orchestration
│   ├── deploy.rs            # Contract deployment (needs: register_token, register_chain)
│   ├── tests.rs             # Test implementations (needs: real transfers, fraud)
│   └── services.rs          # NEW: Operator/Canceler management
```

---

## Testing the Implementation

After completing tasks, verify:

```bash
# Build
cargo build -p cl8y-e2e --release

# Run infrastructure tests (current)
cargo run -p cl8y-e2e -- run --quick

# Run full integration tests (after Sprint 16)
cargo run -p cl8y-e2e -- run

# Specific test
cargo run -p cl8y-e2e -- run --test real_evm_to_terra_transfer
```

---

## Estimated Effort

| Task | Jobs | Lines | Effort |
|------|------|-------|--------|
| Task 1: Terra deployment | 4 | ~500 | High |
| Task 2: Time manipulation | 1 | ~50 | Low |
| Task 3: Real transfers | 3 | ~400 | High |
| Task 4: Token registration | 2 | ~200 | Medium |
| Task 5: Service management | 2 | ~300 | Medium |
| Task 6: Fraud tests | 2 | ~250 | Medium |

**Total**: ~14 jobs, ~1700 lines

---

## Success Criteria

Sprint 16 is complete when:

1. [x] `cargo run -p cl8y-e2e -- setup` deploys Terra contracts
2. [x] `cargo run -p cl8y-e2e -- run` executes real token transfers
3. [x] Balance verification passes for EVM→Terra and Terra→EVM
4. [x] Time skip works for watchtower delay testing
5. [x] Fraud detection test creates and cancels fake approval
6. [x] All bash E2E tests can be replaced by Rust equivalents
7. [ ] CI workflow passes with Rust E2E suite

### Implementation Status (Completed)

| Component | Status | Location |
|-----------|--------|----------|
| EVM Time Manipulation | ✅ Complete | `evm.rs` - `AnvilTimeClient` |
| Service Management | ✅ Complete | `services.rs` - `ServiceManager` |
| Real EVM→Terra Transfer | ✅ Complete | `tests.rs` - `test_real_evm_to_terra_transfer` |
| Real Terra→EVM Transfer | ✅ Complete | `tests.rs` - `test_real_terra_to_evm_transfer` |
| Fraud Detection Full | ✅ Complete | `tests.rs` - `test_fraud_detection_full` |
| Terra Bridge Deployment | ✅ Complete | `setup.rs` - `deploy_terra_contracts` |
| Test Token Deployment | ✅ Complete | `deploy.rs` - `deploy_test_token_simple` |
| ABI Encoding | ✅ Complete | `tests.rs` - Proper function selectors |

---

## Reference: Bash Scripts Being Replaced

| Bash Script | Rust Replacement |
|-------------|------------------|
| `scripts/e2e-setup.sh` | `E2eSetup::run_full_setup()` |
| `scripts/e2e-teardown.sh` | `E2eTeardown::run()` |
| `scripts/e2e-test.sh` | `TestSuite::run_all_tests()` |
| `scripts/e2e-helpers/real-transfer-test.sh` | `test_real_*_transfer()` |
| `scripts/e2e-helpers/fraudulent-approval.sh` | `test_fraud_detection_full()` |
| `scripts/e2e-helpers/evm-deposit.sh` | `execute_deposit()` |
| `scripts/e2e-helpers/terra-lock.sh` | `execute_terra_lock()` |
| `scripts/e2e-helpers/common.sh` | Various helper functions in modules |
