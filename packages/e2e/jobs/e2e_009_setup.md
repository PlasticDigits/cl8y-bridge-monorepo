---
output_dir: src/
output_file: setup.rs
context_files:
  - src/config.rs
  - src/docker.rs
verify: true
depends_on:
  - e2e_002_config
  - e2e_004_docker
---

# E2E Setup Orchestration Module

## Requirements

Create a module that orchestrates the complete E2E environment setup.
This replaces the main orchestration logic from `scripts/e2e-setup.sh`.

## Core Struct

```rust
use crate::config::E2eConfig;
use crate::docker::DockerCompose;
use eyre::Result;
use std::path::PathBuf;

/// E2E Setup orchestrator
pub struct E2eSetup {
    project_root: PathBuf,
    docker: DockerCompose,
    config: E2eConfig,
}
```

## Setup Steps Enum

```rust
/// Individual setup steps for progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    CheckPrerequisites,
    CleanupExisting,
    StartServices,
    WaitForServices,
    DeployEvmContracts,
    DeployTerraContracts,
    GrantRoles,
    RegisterChainKeys,
    ExportEnvironment,
    VerifySetup,
}

impl SetupStep {
    pub fn name(&self) -> &'static str {
        match self {
            Self::CheckPrerequisites => "Check Prerequisites",
            Self::CleanupExisting => "Cleanup Existing",
            Self::StartServices => "Start Services",
            Self::WaitForServices => "Wait for Services",
            Self::DeployEvmContracts => "Deploy EVM Contracts",
            Self::DeployTerraContracts => "Deploy Terra Contracts",
            Self::GrantRoles => "Grant Roles",
            Self::RegisterChainKeys => "Register Chain Keys",
            Self::ExportEnvironment => "Export Environment",
            Self::VerifySetup => "Verify Setup",
        }
    }
}
```

## Required Methods

### new()
```rust
impl E2eSetup {
    /// Create a new E2eSetup orchestrator
    pub async fn new(project_root: PathBuf) -> Result<Self>;
}
```

### Prerequisites
```rust
/// Check all prerequisites are met
/// Returns list of missing prerequisites
pub async fn check_prerequisites(&self) -> Result<Vec<String>> {
    // Check: docker, forge, cast, curl
    // Check: Docker daemon running
    // Check: docker compose available
}
```

### Cleanup
```rust
/// Clean up any existing E2E containers and files
pub async fn cleanup_existing(&self) -> Result<()>;
```

### Service Management
```rust
/// Start all Docker services with E2E profile
pub async fn start_services(&self) -> Result<()>;

/// Wait for all services to be healthy
pub async fn wait_for_services(&self, timeout: std::time::Duration) -> Result<()>;
```

### Contract Deployment (placeholder - actual deployment via forge)
```rust
/// Deploy EVM contracts using forge script
/// Returns deployed addresses
pub async fn deploy_evm_contracts(&self) -> Result<DeployedContracts>;

/// Deploy Terra contracts (if LocalTerra running)
pub async fn deploy_terra_contracts(&self) -> Result<Option<String>>; // Returns bridge address
```

### Role Setup
```rust
/// Grant OPERATOR_ROLE and CANCELER_ROLE to test accounts
pub async fn grant_roles(&self, deployed: &DeployedContracts) -> Result<()>;

/// Register Terra chain key on ChainRegistry
pub async fn register_chain_keys(&self, deployed: &DeployedContracts) -> Result<()>;
```

### Environment Export
```rust
/// Export all addresses to .env.e2e file
pub async fn export_environment(&self, deployed: &DeployedContracts) -> Result<PathBuf>;
```

### Verification
```rust
/// Verify setup is complete and working
pub async fn verify_setup(&self) -> Result<SetupVerification>;
```

### Full Setup
```rust
/// Run complete setup with progress callback
pub async fn run_full_setup<F>(&mut self, on_step: F) -> Result<SetupResult>
where
    F: FnMut(SetupStep, bool); // step, success
```

## Types

```rust
/// Deployed contract addresses
#[derive(Debug, Clone)]
pub struct DeployedContracts {
    pub access_manager: alloy::primitives::Address,
    pub chain_registry: alloy::primitives::Address,
    pub token_registry: alloy::primitives::Address,
    pub mint_burn: alloy::primitives::Address,
    pub lock_unlock: alloy::primitives::Address,
    pub bridge: alloy::primitives::Address,
    pub router: alloy::primitives::Address,
    pub terra_bridge: Option<String>,
}

/// Setup verification result
#[derive(Debug)]
pub struct SetupVerification {
    pub anvil_ok: bool,
    pub postgres_ok: bool,
    pub terra_ok: bool,
    pub evm_bridge_ok: bool,
    pub terra_bridge_ok: bool,
    pub env_file_exists: bool,
}

impl SetupVerification {
    pub fn all_ok(&self) -> bool {
        self.anvil_ok && self.postgres_ok && self.evm_bridge_ok && self.env_file_exists
    }
}

/// Complete setup result
#[derive(Debug)]
pub struct SetupResult {
    pub contracts: DeployedContracts,
    pub verification: SetupVerification,
    pub env_file: PathBuf,
    pub duration: std::time::Duration,
}
```

## Implementation Notes

1. For forge script execution, use `std::process::Command`
2. Parse broadcast files with serde for contract addresses
3. Use EVM_RPC_URL and PRIVATE_KEY from environment
4. Log all steps with `tracing::info!`
5. Return early on any failure with descriptive error

## Constraints

- No `.unwrap()` - use `?` operator
- Use `eyre::Result` with context for errors
- All async functions should be cancellation-safe
- Default Anvil private key: `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`