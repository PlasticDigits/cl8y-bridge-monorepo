# Sprint 15: E2E Migration Completion & Documentation

**Previous Sprint:** [SPRINT14.md](./SPRINT14.md) - E2E Test Migration to Rust

---

## Executive Summary

Sprint 14 established the foundation for the Rust E2E package (`packages/e2e`). Sprint 15 completes the migration by implementing remaining test cases, integrating with CI/pre-commit hooks, creating comprehensive documentation, and archiving the legacy bash scripts.

### Sprint 14 Accomplishments

| Module | Status | Lines | Description |
|--------|--------|-------|-------------|
| `config.rs` | ✅ Complete | 353 | Typed configuration with Address/B256 |
| `docker.rs` | ✅ Complete | 340 | Docker Compose management via bollard |
| `evm.rs` | ✅ Complete | 229 | Type-safe EVM contract interactions |
| `terra.rs` | ✅ Complete | 552 | Terra chain client via LCD/docker exec |
| `setup.rs` | ✅ Complete | 570 | Setup orchestration |
| `teardown.rs` | ✅ Complete | 504 | Teardown orchestration |
| `deploy.rs` | ✅ Complete | 459 | Contract deployment & role management |
| `utils.rs` | ✅ Complete | 155 | Polling & helper utilities |
| `tests.rs` | ⚠️ Partial | 376 | Connectivity tests done, integration tests TODO |
| `main.rs` | ⚠️ Partial | 130 | CLI structure done, commands TODO |

**Current State:** Package compiles, basic tests run, but integration tests and CLI commands are stubs.

**Target State:** Fully functional E2E suite that replaces all bash scripts with CI integration.

---

## Remaining Work Overview

### 1. Test Implementation (High Priority)

The following test functions in `src/tests.rs` are stubs that need implementation:

| Test Function | Bash Equivalent | Priority | Est. Lines |
|---------------|-----------------|----------|------------|
| `test_evm_to_terra_transfer` | `test_evm_to_terra_transfer()` | P0 | 150 |
| `test_terra_to_evm_transfer` | `test_terra_to_evm_transfer()` | P0 | 150 |
| `test_fraud_detection` | `test_canceler_fraudulent_detection()` | P0 | 200 |
| `test_deposit_nonce` | `test_evm_watchtower_approve_execute_flow()` | P1 | 100 |
| `test_token_registry` | `register_test_tokens()` validation | P1 | 80 |
| `test_chain_registry` | `register_terra_chain_key()` validation | P1 | 80 |
| `test_access_manager` | `grant_operator_role()` validation | P1 | 80 |

### 2. CLI Command Implementation (High Priority)

| Command | Current State | Needs |
|---------|---------------|-------|
| `setup` | Stub with TODO | Call `E2eSetup::run_full_setup()` |
| `run` | Works for tests | Single test filtering |
| `teardown` | Stub with TODO | Call `E2eTeardown::run()` |
| `status` | Basic info only | Check Docker/service health |

### 3. Documentation (Medium Priority)

| Document | Status | Description |
|----------|--------|-------------|
| `packages/e2e/README.md` | ❌ Missing | Usage, examples, architecture |
| `docs/e2e-testing.md` | ❌ Missing | Developer guide for E2E tests |
| Rustdoc comments | ⚠️ Partial | Complete public API docs |
| Root `README.md` | ⚠️ Needs update | Add new E2E commands |

### 4. Integration (Medium Priority)

| Integration Point | Status | Description |
|-------------------|--------|-------------|
| Pre-commit hook | ❌ Not done | Run quick tests on commit |
| CI workflow | ❌ Not done | Full E2E in GitHub Actions |
| Makefile | ❌ Not done | `make e2e-test` target |

### 5. Cleanup (Low Priority)

| Task | Status | Description |
|------|--------|-------------|
| Archive bash scripts | ❌ Not done | Move to `scripts/legacy/` |
| Update `.gitignore` | ❌ Not done | Ignore E2E artifacts |
| Remove bash references | ❌ Not done | Update docs pointing to bash |

---

## WorkSplit Job Plan

### Batch 1: Test Implementations (Priority)

These jobs implement the TODO test stubs with actual functionality.

#### Job: `e2e_012_test_transfer_evm_terra`
**Mode:** REPLACE  
**Output:** `src/tests/transfer.rs`  
**Lines:** ~300  
**Context:** `src/evm.rs`, `src/terra.rs`, `src/config.rs`

```rust
//! Cross-chain transfer tests

use crate::{E2eConfig, TestResult};
use crate::evm::EvmBridgeClient;
use crate::terra::TerraClient;
use alloy::primitives::{Address, U256};

/// Full EVM → Terra transfer test
pub async fn test_evm_to_terra_transfer(config: &E2eConfig) -> TestResult {
    // 1. Record initial balances on both chains
    // 2. Approve token spend on LockUnlock
    // 3. Call Router.deposit()
    // 4. Verify deposit nonce incremented
    // 5. Wait for operator to relay to Terra
    // 6. Verify Terra approval created
    // 7. Execute withdraw on Terra
    // 8. Verify final balances
}

/// Full Terra → EVM transfer test
pub async fn test_terra_to_evm_transfer(config: &E2eConfig) -> TestResult {
    // 1. Record initial balances
    // 2. Lock tokens on Terra bridge
    // 3. Wait for operator to relay to EVM
    // 4. Verify EVM approval created
    // 5. Skip time past delay (Anvil)
    // 6. Execute withdraw on EVM
    // 7. Verify final balances
}
```

#### Job: `e2e_013_test_fraud`
**Mode:** REPLACE  
**Output:** `src/tests/fraud.rs`  
**Lines:** ~200  
**Context:** `src/evm.rs`, `src/config.rs`

```rust
//! Fraud detection and cancellation tests

use crate::{E2eConfig, TestResult};
use crate::evm::AccessManagerClient;
use alloy::primitives::{Address, B256, U256};

/// Test fraudulent approval detection
pub async fn test_fraud_detection(config: &E2eConfig) -> TestResult {
    // 1. Create fraudulent approval (no matching deposit)
    // 2. Wait for canceler to detect
    // 3. Verify approval is cancelled
    // 4. Verify withdraw attempt fails with ApprovalCancelled
}

/// Test cancel and reenable flow
pub async fn test_cancel_reenable(config: &E2eConfig) -> TestResult {
    // 1. Create approval
    // 2. Cancel it
    // 3. Verify cancelled
    // 4. Reenable as admin
    // 5. Verify reenabled
}

/// Test watchtower delay enforcement
pub async fn test_watchtower_delay(config: &E2eConfig) -> TestResult {
    // 1. Create approval
    // 2. Attempt immediate withdraw (should fail)
    // 3. Skip time on Anvil
    // 4. Attempt withdraw (should succeed)
}
```

#### Job: `e2e_014_test_registry`
**Mode:** REPLACE  
**Output:** `src/tests/registry.rs`  
**Lines:** ~200  
**Context:** `src/evm.rs`, `src/deploy.rs`

```rust
//! Registry contract tests

use crate::{E2eConfig, TestResult};
use crate::deploy::{register_token, register_cosmw_chain, BridgeType};
use alloy::primitives::{Address, B256};

/// Test token registry operations
pub async fn test_token_registry(config: &E2eConfig) -> TestResult {
    // 1. Register a test token
    // 2. Verify it's registered
    // 3. Add destination chain key
    // 4. Verify destination is set
}

/// Test chain registry operations
pub async fn test_chain_registry(config: &E2eConfig) -> TestResult {
    // 1. Register a test chain
    // 2. Get chain key
    // 3. Verify non-zero
}

/// Test access manager permissions
pub async fn test_access_manager(config: &E2eConfig) -> TestResult {
    // 1. Check test account has OPERATOR_ROLE
    // 2. Check test account has CANCELER_ROLE
    // 3. Test unauthorized call fails
}

/// Test deposit nonce tracking
pub async fn test_deposit_nonce(config: &E2eConfig) -> TestResult {
    // 1. Get current nonce
    // 2. Make deposit
    // 3. Verify nonce incremented by 1
}
```

### Batch 2: CLI Completion

#### Job: `e2e_015_main_complete`
**Mode:** REPLACE  
**Output:** `src/main.rs`  
**Lines:** ~250  
**Context:** `src/setup.rs`, `src/teardown.rs`, `src/tests.rs`

```rust
//! CL8Y Bridge E2E Test CLI - Complete Implementation

use clap::{Parser, Subcommand};
use cl8y_e2e::{
    E2eConfig, E2eSetup, E2eTeardown, TeardownOptions,
    run_all_tests, run_quick_tests, TestSuite,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    match cli.command {
        Commands::Setup => {
            let project_root = std::env::current_dir()?;
            let mut setup = E2eSetup::new(project_root).await?;
            let result = setup.run_full_setup(|step, success| {
                // Progress callback
            }).await?;
            println!("Setup complete in {:?}", result.duration);
        }
        
        Commands::Run { test, no_terra, quick } => {
            // Existing implementation + single test filtering
            if let Some(test_name) = test {
                run_single_test(&config, &test_name).await?;
            } else {
                // ... existing logic
            }
        }
        
        Commands::Teardown { keep_volumes } => {
            let project_root = std::env::current_dir()?;
            let mut teardown = E2eTeardown::new(project_root).await?;
            let result = teardown.run(TeardownOptions {
                keep_volumes,
                ..Default::default()
            }).await?;
            println!("Teardown complete in {:?}", result.duration);
        }
        
        Commands::Status => {
            check_infrastructure_status(&config).await?;
        }
    }
}
```

### Batch 3: Module Integration

Update `src/lib.rs` to export new test modules and wire them into `run_all_tests`.

#### Job: `e2e_016_lib_update`
**Mode:** REPLACE  
**Output:** `src/lib.rs`  
**Lines:** ~150  
**Context:** Current `src/lib.rs`

```rust
pub mod config;
pub mod deploy;
pub mod docker;
pub mod evm;
pub mod setup;
pub mod teardown;
pub mod terra;
pub mod tests;
pub mod utils;

// Test submodules
mod tests {
    pub mod connectivity;
    pub mod transfer;
    pub mod fraud;
    pub mod registry;
}

pub use config::E2eConfig;
pub use deploy::{deploy_evm_contracts, EvmDeployment};
pub use docker::DockerCompose;
pub use setup::{E2eSetup, SetupResult, SetupStep};
pub use teardown::{E2eTeardown, TeardownOptions, TeardownResult};
pub use terra::TerraClient;
pub use tests::{run_all_tests, run_quick_tests};
```

---

## Documentation Plan

### 1. Package README (`packages/e2e/README.md`)

```markdown
# CL8Y Bridge E2E Test Suite

Type-safe end-to-end testing for the CL8Y cross-chain bridge.

## Quick Start

\`\`\`bash
# From project root
cargo run -p cl8y-e2e -- setup    # Start infrastructure
cargo run -p cl8y-e2e -- run      # Run all tests
cargo run -p cl8y-e2e -- teardown # Clean up
\`\`\`

## Commands

| Command | Description |
|---------|-------------|
| `setup` | Start Docker services, deploy contracts |
| `run` | Execute E2E tests |
| `run --quick` | Connectivity tests only |
| `run --no-terra` | Skip Terra tests |
| `run --test <name>` | Run single test |
| `teardown` | Stop services, clean up |
| `status` | Show infrastructure status |

## Architecture

- `config.rs` - Typed configuration (Address, B256)
- `docker.rs` - Docker Compose via bollard
- `evm.rs` - EVM contract clients via alloy
- `terra.rs` - Terra client via LCD API
- `setup.rs` - Infrastructure orchestration
- `teardown.rs` - Cleanup orchestration
- `tests/*.rs` - Test implementations

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `EVM_RPC_URL` | Anvil RPC endpoint | `http://localhost:8545` |
| `TERRA_LCD_URL` | LocalTerra LCD | `http://localhost:1317` |
| `DATABASE_URL` | PostgreSQL connection | `postgres://...` |

## Test Categories

### Connectivity Tests
- EVM/Anvil connectivity
- Terra/LocalTerra connectivity  
- PostgreSQL connectivity

### Transfer Tests
- EVM → Terra full cycle
- Terra → EVM full cycle
- Balance verification

### Security Tests
- Fraud detection & cancellation
- Watchtower delay enforcement
- Access manager permissions

### Registry Tests
- Token registration
- Chain key registration
- Deposit nonce tracking
```

### 2. Developer Guide (`docs/e2e-testing.md`)

```markdown
# E2E Testing Guide

## Overview

The E2E test suite (`packages/e2e`) validates the complete bridge system 
including EVM contracts, Terra contracts, operator relay, and canceler 
fraud detection.

## Prerequisites

- Docker & docker compose
- Rust toolchain
- Foundry (forge, cast, anvil)

## Running Tests

### Full Suite
\`\`\`bash
cargo run -p cl8y-e2e -- setup
cargo run -p cl8y-e2e -- run
cargo run -p cl8y-e2e -- teardown
\`\`\`

### Quick Validation
\`\`\`bash
cargo run -p cl8y-e2e -- run --quick
\`\`\`

## Writing New Tests

1. Add test function to appropriate module:
   - Connectivity → `tests/connectivity.rs`
   - Transfers → `tests/transfer.rs`
   - Security → `tests/fraud.rs`
   - Contracts → `tests/registry.rs`

2. Follow the pattern:
\`\`\`rust
pub async fn test_my_feature(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "my_feature";
    
    match do_something(&config).await {
        Ok(_) => TestResult::pass(name, start.elapsed()),
        Err(e) => TestResult::fail(name, e.to_string(), start.elapsed()),
    }
}
\`\`\`

3. Add to `run_all_tests()` in appropriate category.

## Architecture

### Type Safety

All addresses use `alloy::primitives::Address`, not `String`.
All chain keys use `alloy::primitives::B256`, not `String`.

This catches typos at compile time:
\`\`\`rust
// Compiler error: field `token_registri` does not exist
let addr = config.evm.contracts.token_registri;
\`\`\`

### Contract Clients

Use the typed clients in `src/evm.rs`:
\`\`\`rust
let bridge = EvmBridgeClient::new(provider, config.evm.contracts.bridge);
let nonce = bridge.deposit_nonce().await?;
\`\`\`

### Terra Interactions

Use `TerraClient` in `src/terra.rs`:
\`\`\`rust
let terra = TerraClient::new(&config.terra);
let approvals = terra.get_pending_approvals(&bridge_addr, 10).await?;
\`\`\`
```

---

## Integration Tasks

### 1. Pre-commit Hook

Update `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: e2e-quick
        name: E2E Quick Tests
        entry: cargo run -p cl8y-e2e -- run --quick
        language: system
        pass_filenames: false
        stages: [commit]
```

### 2. GitHub Actions CI

Create/update `.github/workflows/e2e.yml`:

```yaml
name: E2E Tests

on:
  push:
    branches: [main]
  pull_request:

jobs:
  e2e:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_USER: operator
          POSTGRES_PASSWORD: operator
          POSTGRES_DB: operator
        ports:
          - 5433:5432
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust
        uses: dtolnay/rust-action@stable
      
      - name: Setup Foundry
        uses: foundry-rs/foundry-toolchain@v1
      
      - name: Start Anvil
        run: anvil --port 8545 &
      
      - name: Setup E2E
        run: cargo run -p cl8y-e2e -- setup
      
      - name: Run E2E Tests
        run: cargo run -p cl8y-e2e -- run --no-terra
      
      - name: Teardown
        if: always()
        run: cargo run -p cl8y-e2e -- teardown
```

### 3. Makefile Targets

Add to project `Makefile`:

```makefile
.PHONY: e2e-setup e2e-test e2e-teardown e2e-quick

e2e-setup:
	cargo run -p cl8y-e2e -- setup

e2e-test:
	cargo run -p cl8y-e2e -- run

e2e-quick:
	cargo run -p cl8y-e2e -- run --quick

e2e-teardown:
	cargo run -p cl8y-e2e -- teardown

e2e-full: e2e-setup e2e-test e2e-teardown
```

---

## Cleanup Tasks

### Archive Bash Scripts

```bash
# Create legacy directory
mkdir -p scripts/legacy

# Move bash E2E scripts
mv scripts/e2e-setup.sh scripts/legacy/
mv scripts/e2e-test.sh scripts/legacy/
mv scripts/e2e-teardown.sh scripts/legacy/
mv scripts/e2e-helpers/ scripts/legacy/

# Add deprecation notice
cat > scripts/legacy/README.md << 'EOF'
# Legacy E2E Scripts

These bash scripts have been replaced by the Rust E2E package.

**Use instead:**
```bash
cargo run -p cl8y-e2e -- setup
cargo run -p cl8y-e2e -- run
cargo run -p cl8y-e2e -- teardown
```

See `packages/e2e/README.md` for documentation.
EOF
```

### Update .gitignore

Add E2E-specific ignores:

```gitignore
# E2E test artifacts
.env.e2e
.env.local
.operator.log
.canceler-*.log
packages/e2e/target/
```

---

## Success Criteria

### Test Coverage

- [ ] All connectivity tests pass
- [ ] `test_evm_to_terra_transfer` implemented and passing
- [ ] `test_terra_to_evm_transfer` implemented and passing
- [ ] `test_fraud_detection` implemented and passing
- [ ] `test_deposit_nonce` implemented and passing
- [ ] `test_token_registry` implemented and passing
- [ ] `test_chain_registry` implemented and passing
- [ ] `test_access_manager` implemented and passing

### CLI Completion

- [ ] `setup` command fully functional
- [ ] `run --test <name>` filters to single test
- [ ] `teardown` command fully functional
- [ ] `status` shows real service health

### Documentation

- [ ] `packages/e2e/README.md` created
- [ ] `docs/e2e-testing.md` created
- [ ] All public functions have rustdoc
- [ ] Root README updated with new commands

### Integration

- [ ] Pre-commit hook runs quick tests
- [ ] CI workflow runs full E2E
- [ ] Makefile has e2e targets
- [ ] Bash scripts archived to `scripts/legacy/`

### Code Quality

- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo test` passes
- [ ] All `TODO` comments resolved
- [ ] No `.unwrap()` calls outside tests

---

## Interfaces Reference

### E2eConfig (from config.rs)

```rust
pub struct E2eConfig {
    pub evm: EvmConfig,
    pub terra: TerraConfig,
    pub docker: DockerConfig,
    pub operator: OperatorConfig,
    pub test_accounts: TestAccounts,
}

impl E2eConfig {
    pub fn from_env() -> Result<Self>;
    pub fn default() -> Self;
}
```

### EvmBridgeClient (from evm.rs)

```rust
pub struct EvmBridgeClient<P> {
    provider: P,
    bridge_address: Address,
    router_address: Address,
}

impl<P: Provider + Clone> EvmBridgeClient<P> {
    pub fn new(provider: P, bridge: Address, router: Address) -> Self;
    pub async fn deposit_nonce(&self) -> Result<u64>;
    pub async fn withdraw_delay(&self) -> Result<u64>;
    pub async fn get_approval(&self, hash: B256) -> Result<WithdrawApproval>;
}
```

### TerraClient (from terra.rs)

```rust
pub struct TerraClient {
    lcd_url: Url,
    rpc_url: Url,
    chain_id: String,
    container_name: String,
    key_name: String,
}

impl TerraClient {
    pub fn new(config: &TerraConfig) -> Self;
    pub async fn is_healthy(&self) -> Result<bool>;
    pub async fn get_block_height(&self) -> Result<u64>;
    pub async fn query_contract<T>(&self, addr: &str, query: &Value) -> Result<T>;
    pub async fn execute_contract(&self, addr: &str, msg: &Value, funds: Option<&str>) -> Result<String>;
    pub async fn get_pending_approvals(&self, bridge: &str, limit: u32) -> Result<Vec<PendingApproval>>;
}
```

### E2eSetup (from setup.rs)

```rust
pub struct E2eSetup {
    project_root: PathBuf,
    docker: DockerCompose,
    config: E2eConfig,
}

impl E2eSetup {
    pub async fn new(project_root: PathBuf) -> Result<Self>;
    pub async fn check_prerequisites(&self) -> Result<Vec<String>>;
    pub async fn deploy_evm_contracts(&self) -> Result<DeployedContracts>;
    pub async fn deploy_terra_contracts(&self) -> Result<Option<String>>;
    pub async fn grant_roles(&self, deployed: &DeployedContracts) -> Result<()>;
    pub async fn run_full_setup<F>(&mut self, on_step: F) -> Result<SetupResult>;
}
```

### E2eTeardown (from teardown.rs)

```rust
pub struct E2eTeardown {
    project_root: PathBuf,
    docker: DockerCompose,
}

impl E2eTeardown {
    pub async fn new(project_root: PathBuf) -> Result<Self>;
    pub async fn stop_docker_services(&self, options: &TeardownOptions) -> Result<()>;
    pub async fn cleanup_files(&self) -> Result<Vec<PathBuf>>;
    pub async fn run(&mut self, options: TeardownOptions) -> Result<TeardownResult>;
}
```

### TestResult (from lib.rs)

```rust
pub enum TestResult {
    Pass { name: String, duration: Duration },
    Fail { name: String, error: String, duration: Duration },
    Skip { name: String, reason: String },
}

impl TestResult {
    pub fn pass(name: &str, duration: Duration) -> Self;
    pub fn fail(name: &str, error: impl ToString, duration: Duration) -> Self;
    pub fn skip(name: &str, reason: impl ToString) -> Self;
}
```

---

## Module Dependencies

```
lib.rs (exports)
├── config.rs (E2eConfig, EvmConfig, TerraConfig)
├── docker.rs (DockerCompose) 
│   └── uses: config
├── evm.rs (EvmBridgeClient, AccessManagerClient)
│   └── uses: config
├── terra.rs (TerraClient)
│   └── uses: config
├── deploy.rs (deploy_evm_contracts, grant_*_role, register_*)
│   └── uses: config, evm
├── setup.rs (E2eSetup)
│   └── uses: config, docker, deploy
├── teardown.rs (E2eTeardown)
│   └── uses: config, docker
├── utils.rs (poll_until, retry_with_backoff)
│   └── uses: (none)
└── tests/
    ├── connectivity.rs
    │   └── uses: config
    ├── transfer.rs
    │   └── uses: config, evm, terra
    ├── fraud.rs
    │   └── uses: config, evm
    └── registry.rs
        └── uses: config, evm, deploy
```

---

## Prompt for Next Agent

Copy and paste this to begin Sprint 15:

```
Continue Sprint 15: E2E Migration Completion

Read SPRINT15.md for full context. Summary:

Sprint 14 created the Rust E2E package foundation. Sprint 15 completes it:

Completed in Sprint 14:
- Core modules: config, docker, evm, terra, setup, teardown, deploy, utils
- Package compiles with cargo check
- Basic connectivity tests work

Remaining for Sprint 15:

1. PRIORITY - Implement test stubs in tests.rs:
   - test_evm_to_terra_transfer (lines 156-168)
   - test_terra_to_evm_transfer (lines 175-187)
   - test_fraud_detection (lines 194-206)
   - test_deposit_nonce (lines 213-225)
   - test_token_registry (lines 232-244)
   - test_chain_registry (lines 251-263)
   - test_access_manager (lines 270-281)

2. Wire CLI commands in main.rs:
   - Commands::Setup -> call E2eSetup::run_full_setup()
   - Commands::Teardown -> call E2eTeardown::run()
   - Commands::Run with --test filtering

3. Create documentation:
   - packages/e2e/README.md
   - docs/e2e-testing.md

4. Integration:
   - Update Makefile with e2e targets
   - Create/update CI workflow
   - Archive bash scripts to scripts/legacy/

Use WorkSplit for test implementations (> 100 lines each).
Do manual edits for CLI wiring and docs.

Key files to reference:
- packages/e2e/src/evm.rs for EVM interactions
- packages/e2e/src/terra.rs for Terra interactions
- scripts/e2e-test.sh for bash test logic to port
```

---

*Created: 2026-02-03*  
*Previous Sprint: SPRINT14.md - E2E Test Migration to Rust*
