# Sprint 14: E2E Test Migration to Rust

**Previous Sprint:** [SPRINT13.md](./SPRINT13.md) - Security Hardening & Production Readiness

---

## Executive Summary

Migrate 100% of E2E test infrastructure from bash scripts to a Rust package (`packages/e2e`). This eliminates an entire class of runtime bugs that are currently blocking commits and CI.

### Why This Migration Is Critical

The current bash E2E scripts have fundamental issues that Rust would solve:

| Bash Problem | Example | Rust Solution |
|--------------|---------|---------------|
| **Typos compile** | `$TOKEN_REGISTRY_ADDRESS_ADDRESS` | Compiler error: undefined variable |
| **No type safety** | Empty string vs null vs missing | `Option<Address>` with explicit handling |
| **Variable scoping** | Local vars invisible between functions | Explicit function returns, struct fields |
| **JSON parsing** | `jq` commands fail silently | `serde_json` with typed deserialization |
| **No IDE support** | No autocomplete, no refactoring | Full rust-analyzer support |
| **Flaky tests** | Race conditions, timing hacks | `tokio::time::timeout`, proper async |

**Current State:** E2E tests have been blocking commits due to bash variable typos (`TOKEN_REGISTRY_ADDRESS_ADDRESS` instead of `TOKEN_REGISTRY_ADDRESS`).

**Target State:** Type-safe, maintainable E2E test suite in Rust with compile-time guarantees.

---

## WorkSplit Strategy

This migration is ideal for WorkSplit:
- **200+ lines per module** - well above the 50-line threshold
- **Structured output** - each Rust module is self-contained
- **Clear interfaces** - existing bash scripts define the API

### Pre-Migration Checklist (For Next Agent)

Before creating any WorkSplit jobs, you MUST:

1. **Read the WorkSplit guide:** `docs/worksplit-guide.md`
2. **Verify Ollama is running:** `curl http://localhost:11434/api/tags`
3. **Initialize WorkSplit in the new package:**
   ```bash
   mkdir -p packages/e2e
   cd packages/e2e
   cargo init
   worksplit init --lang rust --model worksplit-coder-glm-4.7:32k
   ```

### Job Sizing Rules

| Component | Estimated Lines | Jobs Required |
|-----------|-----------------|---------------|
| Config & types | 150 | 1-2 jobs |
| EVM interactions | 200 | 2 jobs |
| Terra interactions | 200 | 2 jobs |
| Docker management | 150 | 1 job |
| Test runner framework | 200 | 2 jobs |
| Individual test cases | 100 each | 5-8 jobs |

**Total: ~15-20 WorkSplit jobs**

---

## Package Structure

```
packages/e2e/
├── Cargo.toml
├── worksplit.toml
├── jobs/
│   └── _managerinstruction.md
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library exports
│   ├── config.rs            # Configuration structs
│   ├── contracts/
│   │   ├── mod.rs
│   │   ├── evm.rs           # EVM contract bindings
│   │   └── terra.rs         # Terra contract bindings
│   ├── docker/
│   │   ├── mod.rs
│   │   └── compose.rs       # Docker Compose management
│   ├── setup/
│   │   ├── mod.rs
│   │   ├── anvil.rs         # Anvil deployment
│   │   ├── terra.rs         # LocalTerra deployment
│   │   └── tokens.rs        # Test token deployment
│   ├── tests/
│   │   ├── mod.rs
│   │   ├── connectivity.rs   # Infrastructure tests
│   │   ├── evm_to_terra.rs   # Cross-chain transfer tests
│   │   ├── terra_to_evm.rs   # Reverse direction tests
│   │   ├── fraud.rs          # Fraud detection tests
│   │   └── security.rs       # Security hardening tests
│   └── utils/
│       ├── mod.rs
│       ├── addresses.rs      # Address extraction from broadcast
│       └── wait.rs           # Polling utilities
└── tests/
    └── integration.rs
```

---

## WorkSplit Job Plan

### Batch 1: Foundation (No Dependencies)

These jobs create the base types and configuration. Run all in parallel.

#### Job: `e2e_001_cargo_toml`
**Mode:** REPLACE  
**Output:** `Cargo.toml`  
**Lines:** ~50

```toml
[package]
name = "cl8y-e2e"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
alloy = { version = "0.1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
eyre = "0.6"
tracing = "0.1"
tracing-subscriber = "0.3"
bollard = "0.16"           # Docker API
clap = { version = "4", features = ["derive"] }
cosmrs = "0.15"
tendermint-rpc = "0.34"

[dev-dependencies]
tokio-test = "0.4"
```

#### Job: `e2e_002_config`
**Mode:** REPLACE  
**Output:** `src/config.rs`  
**Lines:** ~120

```rust
/// Configuration for E2E test environment
/// Replaces: scripts/e2e-setup.sh environment variables
pub struct E2eConfig {
    pub evm: EvmConfig,
    pub terra: TerraConfig,
    pub docker: DockerConfig,
    pub operator: OperatorConfig,
}

pub struct EvmConfig {
    pub rpc_url: Url,
    pub chain_id: u64,
    pub private_key: B256,
    pub contracts: EvmContracts,
}

pub struct EvmContracts {
    pub access_manager: Address,
    pub chain_registry: Address,
    pub token_registry: Address,
    pub mint_burn: Address,
    pub lock_unlock: Address,
    pub bridge: Address,
    pub router: Address,
}

impl E2eConfig {
    /// Load from broadcast file - type-safe extraction
    pub fn from_broadcast(path: &Path) -> Result<Self>;
}
```

#### Job: `e2e_003_types`
**Mode:** REPLACE  
**Output:** `src/lib.rs`  
**Lines:** ~80

```rust
pub mod config;
pub mod contracts;
pub mod docker;
pub mod setup;
pub mod tests;
pub mod utils;

pub use config::E2eConfig;

/// Test result with typed pass/fail
pub enum TestResult {
    Pass { name: String, duration: Duration },
    Fail { name: String, error: String },
    Skip { name: String, reason: String },
}

/// Test suite runner
pub struct TestSuite {
    config: E2eConfig,
    results: Vec<TestResult>,
}
```

### Batch 2: Infrastructure (Depends on Batch 1)

#### Job: `e2e_004_docker_compose`
**Mode:** REPLACE  
**Output:** `src/docker/compose.rs`  
**Lines:** ~150  
**Context:** `src/config.rs`

Replaces: `scripts/e2e-setup.sh` Docker management

```rust
/// Docker Compose management for E2E infrastructure
pub struct DockerCompose {
    project_root: PathBuf,
    profile: String,
}

impl DockerCompose {
    pub async fn up(&self) -> Result<()>;
    pub async fn down(&self) -> Result<()>;
    pub async fn wait_healthy(&self, timeout: Duration) -> Result<()>;
    async fn wait_for_anvil(&self) -> Result<()>;
    async fn wait_for_postgres(&self) -> Result<()>;
    async fn wait_for_localterra(&self) -> Result<()>;
}
```

#### Job: `e2e_005_broadcast_parser`
**Mode:** REPLACE  
**Output:** `src/utils/addresses.rs`  
**Lines:** ~100  
**Context:** `src/config.rs`

Replaces: `jq` extraction with typed parsing

```rust
/// Parse forge broadcast file with full type safety
#[derive(Deserialize)]
struct BroadcastFile {
    transactions: Vec<BroadcastTransaction>,
}

#[derive(Deserialize)]
struct BroadcastTransaction {
    #[serde(rename = "contractName")]
    contract_name: String,
    #[serde(rename = "contractAddress")]
    contract_address: Address,
    #[serde(rename = "transactionType")]
    transaction_type: String,
}

pub fn extract_addresses(broadcast_path: &Path) -> Result<EvmContracts> {
    let file = std::fs::read_to_string(broadcast_path)?;
    let broadcast: BroadcastFile = serde_json::from_str(&file)?;
    
    // Type-safe extraction - compiler catches typos!
    let bridge = broadcast.find_create("Cl8YBridge")?;
    let router = broadcast.find_create("BridgeRouter")?;
    // ...
}
```

### Batch 3: EVM Interactions (Depends on Batch 2)

#### Job: `e2e_006_evm_contracts`
**Mode:** REPLACE  
**Output:** `src/contracts/evm.rs`  
**Lines:** ~200  
**Context:** `src/config.rs`, `src/utils/addresses.rs`

Replaces: `cast send`/`cast call` commands

```rust
/// Type-safe EVM contract interactions
pub struct EvmBridge {
    provider: Arc<Provider>,
    bridge: Address,
    router: Address,
}

impl EvmBridge {
    pub async fn deposit(
        &self,
        token: Address,
        amount: U256,
        dest_chain_key: B256,
        dest_account: B256,
    ) -> Result<TxHash>;
    
    pub async fn deposit_nonce(&self) -> Result<u64>;
    pub async fn approve_token(&self, token: Address, spender: Address, amount: U256) -> Result<TxHash>;
}
```

#### Job: `e2e_007_evm_deploy`
**Mode:** REPLACE  
**Output:** `src/setup/anvil.rs`  
**Lines:** ~150  
**Context:** `src/contracts/evm.rs`

Replaces: `forge script` deployment

```rust
/// Deploy EVM contracts to Anvil
pub struct AnvilDeployer {
    config: EvmConfig,
}

impl AnvilDeployer {
    pub async fn deploy_all(&self) -> Result<EvmContracts>;
    pub async fn grant_operator_role(&self, account: Address) -> Result<TxHash>;
    pub async fn register_chain_key(&self, chain_name: &str) -> Result<B256>;
}
```

### Batch 4: Terra Interactions (Depends on Batch 2)

#### Job: `e2e_008_terra_contracts`
**Mode:** REPLACE  
**Output:** `src/contracts/terra.rs`  
**Lines:** ~180  
**Context:** `src/config.rs`

Replaces: `terrad tx wasm` commands

```rust
/// Type-safe Terra contract interactions
pub struct TerraBridge {
    client: HttpClient,
    bridge_address: String,
}

impl TerraBridge {
    pub async fn query_config(&self) -> Result<BridgeConfig>;
    pub async fn query_pending_approvals(&self) -> Result<Vec<Approval>>;
    pub async fn execute_withdraw(&self, approval_hash: B256) -> Result<TxHash>;
}
```

#### Job: `e2e_009_terra_deploy`
**Mode:** REPLACE  
**Output:** `src/setup/terra.rs`  
**Lines:** ~200  
**Context:** `src/contracts/terra.rs`

Replaces: `docker exec terrad tx wasm` deployment

```rust
/// Deploy Terra contracts to LocalTerra
pub struct TerraDeployer {
    config: TerraConfig,
}

impl TerraDeployer {
    pub async fn store_wasm(&self, wasm_path: &Path) -> Result<u64>;
    pub async fn instantiate(&self, code_id: u64, msg: InstantiateMsg) -> Result<String>;
    pub async fn configure_bridge(&self, withdraw_delay: u64) -> Result<TxHash>;
}
```

### Batch 5: Test Cases (Depends on Batches 3-4)

#### Job: `e2e_010_test_connectivity`
**Mode:** REPLACE  
**Output:** `src/tests/connectivity.rs`  
**Lines:** ~100

Replaces: `test_evm_connectivity()`, `test_terra_connectivity()`

```rust
pub async fn test_evm_connectivity(config: &EvmConfig) -> TestResult;
pub async fn test_terra_connectivity(config: &TerraConfig) -> TestResult;
pub async fn test_database_connectivity(config: &DbConfig) -> TestResult;
```

#### Job: `e2e_011_test_evm_to_terra`
**Mode:** REPLACE  
**Output:** `src/tests/evm_to_terra.rs`  
**Lines:** ~150  
**Context:** `src/contracts/evm.rs`, `src/contracts/terra.rs`

Replaces: `test_evm_to_terra_transfer()`

```rust
pub async fn test_evm_to_terra_transfer(
    evm: &EvmBridge,
    terra: &TerraBridge,
    config: &E2eConfig,
) -> TestResult {
    // 1. Check initial balances
    // 2. Approve token
    // 3. Call router.deposit() - NOT bridge.deposit()!
    // 4. Verify nonce incremented
    // 5. Wait for Terra approval
}
```

#### Job: `e2e_012_test_fraud`
**Mode:** REPLACE  
**Output:** `src/tests/fraud.rs`  
**Lines:** ~200  
**Context:** `src/contracts/evm.rs`

Replaces: `test_canceler_fraudulent_detection()`

```rust
pub async fn test_fraud_detection(
    evm: &EvmBridge,
    canceler: &CancelerHandle,
) -> TestResult;

pub async fn test_fraud_cancellation(
    evm: &EvmBridge,
    canceler: &CancelerHandle,
) -> TestResult;
```

### Batch 6: CLI and Runner (Depends on Batch 5)

#### Job: `e2e_013_main`
**Mode:** REPLACE  
**Output:** `src/main.rs`  
**Lines:** ~200

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

enum Commands {
    Setup,
    Run { test: Option<String> },
    Teardown,
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Setup => setup::run().await?,
        Commands::Run { test } => tests::run(test).await?,
        Commands::Teardown => teardown::run().await?,
        Commands::Status => status::run().await?,
    }
}
```

---

## Migration Phases

### Phase 1: Parallel Development (Week 1)

Build the Rust package alongside existing bash scripts.

```bash
# Old way still works
./scripts/e2e-test.sh --full

# New way being developed
cargo run -p cl8y-e2e -- run --full
```

### Phase 2: Feature Parity (Week 2)

All existing tests ported to Rust.

| Bash Script | Rust Equivalent |
|-------------|-----------------|
| `e2e-setup.sh` | `cargo run -p cl8y-e2e -- setup` |
| `e2e-test.sh` | `cargo run -p cl8y-e2e -- run` |
| `e2e-teardown.sh` | `cargo run -p cl8y-e2e -- teardown` |

### Phase 3: Cutover (Week 3)

1. Update pre-commit hook to use Rust E2E
2. Update CI workflows
3. Archive bash scripts (don't delete yet)

### Phase 4: Cleanup (Week 4)

1. Remove bash scripts
2. Add new test cases only in Rust
3. Document new patterns

---

## Success Criteria

### Compile-Time Guarantees

- [ ] All contract addresses are typed (`Address`, not `String`)
- [ ] All chain keys are typed (`B256`, not `String`)
- [ ] Variable name typos cause compiler errors
- [ ] Missing fields cause compiler errors

### Test Coverage

- [ ] All existing tests ported
- [ ] All tests pass on fresh environment
- [ ] Tests run in < 5 minutes (parallel execution)

### Developer Experience

- [ ] `cargo run -p cl8y-e2e -- run` works from project root
- [ ] IDE autocomplete for all test utilities
- [ ] Clear error messages on failure

### Documentation

- [ ] `packages/e2e/README.md` with usage instructions and examples
- [ ] Inline rustdoc comments on all public functions and structs
- [ ] `docs/e2e-testing.md` guide replacing bash script references
- [ ] Root `README.md` updated with new E2E commands
- [ ] `CONTRIBUTING.md` updated with E2E development workflow
- [ ] Migration notes in `docs/changelog.md` or similar
- [ ] WorkSplit job files documented in `packages/e2e/jobs/_managerinstruction.md`

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Rust learning curve | Use alloy/cosmrs which mirror cast/terrad |
| Docker API complexity | Use bollard crate, well-documented |
| Parallel bash/Rust confusion | Clear naming: `e2e-test.sh` (old) vs `cl8y-e2e` (new) |
| WorkSplit job failures | Keep jobs <200 lines, use REPLACE mode |

---

## Questions for Next Agent

1. **Should we keep bash scripts as fallback?**
   Recommend: Archive to `scripts/legacy/` after cutover.

2. **Should E2E binary be published to crates.io?**
   Recommend: No, internal tool only.

3. **Should we add property-based testing?**
   Recommend: Yes, add proptest for edge cases after migration.

4. **What's the test parallelism strategy?**
   Recommend: Per-test-file parallelism with shared Docker infrastructure.

---

## Quick Start for Sprint 14

### Step 1: Create Package

```bash
mkdir -p packages/e2e/src
cd packages/e2e
cargo init
```

### Step 2: Initialize WorkSplit

```bash
worksplit init --lang rust --model worksplit-coder-glm-4.7:32k
mkdir jobs
```

### Step 3: Create Manager Instructions

Create `jobs/_managerinstruction.md`:

```markdown
# E2E Package WorkSplit Jobs

## Mode Selection

ALWAYS use REPLACE mode. Never use EDIT mode for this migration.

## Job Naming

Use prefix: `e2e_NNN_description.md`

## Dependencies

Jobs in Batch N+1 must have `depends_on` for Batch N jobs.

## Verification

All jobs must have:
\`\`\`toml
verify_build = true
verify_tests = false  # Tests require Docker
\`\`\`
```

### Step 4: Run Batch 1

```bash
worksplit new-job e2e_001_cargo_toml --template replace -o . -f Cargo.toml
worksplit new-job e2e_002_config --template replace -o src/ -f config.rs
worksplit new-job e2e_003_types --template replace -o src/ -f lib.rs

worksplit run
worksplit status
```

### Step 5: Continue Batches

After Batch 1 passes, proceed to Batch 2, etc.

---

## Definition of Done for Sprint 14

### Package Structure
- [ ] `packages/e2e/` exists with proper Cargo.toml
- [ ] WorkSplit initialized and configured
- [ ] All jobs created and documented

### Core Functionality
- [ ] `cargo run -p cl8y-e2e -- setup` starts infrastructure
- [ ] `cargo run -p cl8y-e2e -- run` executes all tests
- [ ] `cargo run -p cl8y-e2e -- teardown` cleans up

### Test Parity
- [ ] All bash tests have Rust equivalents
- [ ] All tests pass in CI
- [ ] No more variable typo bugs possible

### Integration
- [ ] Pre-commit hook uses Rust E2E
- [ ] CI workflow uses Rust E2E
- [ ] Makefile targets updated

### Documentation
- [ ] `packages/e2e/README.md` created with usage examples
- [ ] All public APIs have rustdoc comments
- [ ] `docs/e2e-testing.md` updated for Rust workflow
- [ ] Root README updated with new commands

---

## Appendix: Bash → Rust Function Mapping

| Bash Function | Rust Equivalent |
|---------------|-----------------|
| `deploy_evm_contracts()` | `AnvilDeployer::deploy_all()` |
| `deploy_terra_contracts()` | `TerraDeployer::deploy_all()` |
| `grant_operator_role()` | `AnvilDeployer::grant_operator_role()` |
| `register_terra_chain_key()` | `AnvilDeployer::register_chain_key()` |
| `register_test_tokens()` | `AnvilDeployer::register_tokens()` |
| `export_env_file()` | `E2eConfig::save()` |
| `test_evm_connectivity()` | `tests::connectivity::test_evm()` |
| `test_evm_to_terra_transfer()` | `tests::evm_to_terra::test_transfer()` |

---

---

## Prompt for Next Agent

Copy and paste this to begin Sprint 14:

```
Begin Sprint 14: E2E Test Migration to Rust

Read SPRINT14.md for full context. Summary:

We are migrating 100% of bash E2E scripts (scripts/e2e-*.sh) to a Rust package 
at packages/e2e. This eliminates runtime bugs like variable typos that currently 
block commits.

Your first tasks:

1. Read the WorkSplit guide: docs/worksplit-guide.md
2. Create the package structure:
   mkdir -p packages/e2e/src
   cd packages/e2e && cargo init
3. Initialize WorkSplit:
   worksplit init --lang rust --model worksplit-coder-glm-4.7:32k
   mkdir jobs
4. Create jobs/_managerinstruction.md with job conventions
5. Create Batch 1 jobs (foundation - no dependencies):
   - e2e_001_cargo_toml (Cargo.toml with dependencies)
   - e2e_002_config (src/config.rs - typed configuration)
   - e2e_003_types (src/lib.rs - core types and exports)
6. Run: worksplit run && worksplit status

Key constraints:
- ALWAYS use REPLACE mode, never EDIT mode
- Keep jobs under 200 lines output
- Use depends_on in frontmatter for job ordering
- All contract addresses must be typed (Address, not String)

Reference the existing bash scripts for behavior:
- scripts/e2e-setup.sh (infrastructure setup)
- scripts/e2e-test.sh (test execution)
- scripts/e2e-teardown.sh (cleanup)

Success = cargo build passes for each batch before starting the next.
```

---

*Created: 2026-02-03*  
*Previous Sprint: SPRINT13.md - Security Hardening & Production Readiness*
