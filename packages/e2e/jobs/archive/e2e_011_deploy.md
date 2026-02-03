---
output_dir: src/
output_file: deploy.rs
context_files:
  - src/config.rs
  - src/evm.rs
verify: true
depends_on:
  - e2e_002_config
  - e2e_006_evm
---

# Contract Deployment Module

## Requirements

Create a module for deploying contracts in E2E tests.
This replaces the forge script and jq extraction from `scripts/e2e-setup.sh`.

## Forge Script Wrapper

```rust
use alloy::primitives::Address;
use eyre::Result;
use std::path::PathBuf;

/// Run forge script and capture output
pub async fn run_forge_script(
    script_path: &str,
    rpc_url: &str,
    private_key: &str,
    project_dir: &PathBuf,
) -> Result<ForgeScriptResult>;

/// Result of a forge script execution
#[derive(Debug)]
pub struct ForgeScriptResult {
    pub success: bool,
    pub broadcast_file: Option<PathBuf>,
    pub stdout: String,
    pub stderr: String,
}
```

## Broadcast File Parsing

```rust
use serde::Deserialize;

/// Parse a forge broadcast JSON file
#[derive(Debug, Deserialize)]
pub struct BroadcastFile {
    pub transactions: Vec<BroadcastTransaction>,
    pub chain: u64,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastTransaction {
    #[serde(rename = "contractName")]
    pub contract_name: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<Address>,
    #[serde(rename = "transactionType")]
    pub transaction_type: String,
    pub hash: Option<String>,
}

impl BroadcastFile {
    /// Load from file path
    pub fn from_file(path: &PathBuf) -> Result<Self>;
    
    /// Find a contract address by name
    pub fn find_contract(&self, name: &str) -> Result<Address>;
    
    /// Get all deployed contracts
    pub fn get_deployed_contracts(&self) -> Vec<(String, Address)>;
}
```

## EVM Deployment

```rust
/// Deploy all EVM contracts using forge script
pub async fn deploy_evm_contracts(
    project_root: &PathBuf,
    rpc_url: &str,
    private_key: &str,
) -> Result<EvmDeployment>;

/// EVM deployment result
#[derive(Debug, Clone)]
pub struct EvmDeployment {
    pub access_manager: Address,
    pub chain_registry: Address,
    pub token_registry: Address,
    pub mint_burn: Address,
    pub lock_unlock: Address,
    pub bridge: Address,
    pub router: Address,
    pub broadcast_file: PathBuf,
}

impl EvmDeployment {
    /// Verify all contracts are deployed (not zero address)
    pub fn verify(&self) -> Result<()>;
    
    /// Load from existing broadcast file
    pub fn from_broadcast(path: &PathBuf) -> Result<Self>;
}
```

## Role Granting

```rust
use alloy::primitives::B256;

/// Grant operator role to an address
pub async fn grant_operator_role(
    access_manager: Address,
    account: Address,
    rpc_url: &str,
    private_key: B256,
) -> Result<()>;

/// Grant canceler role to an address
pub async fn grant_canceler_role(
    access_manager: Address,
    account: Address,
    rpc_url: &str,
    private_key: B256,
) -> Result<()>;

/// Role IDs
pub const OPERATOR_ROLE_ID: u64 = 1;
pub const CANCELER_ROLE_ID: u64 = 2;
```

## Chain Key Registration

```rust
/// Register a COSMW chain key on ChainRegistry
pub async fn register_cosmw_chain(
    chain_registry: Address,
    chain_id: &str,
    rpc_url: &str,
    private_key: B256,
) -> Result<B256>; // Returns chain key

/// Get chain key for a COSMW chain
pub async fn get_cosmw_chain_key(
    chain_registry: Address,
    chain_id: &str,
    rpc_url: &str,
) -> Result<B256>;
```

## Token Registration

```rust
/// Bridge type enum
#[derive(Debug, Clone, Copy)]
pub enum BridgeType {
    MintBurn = 0,
    LockUnlock = 1,
}

/// Register a token on TokenRegistry
pub async fn register_token(
    token_registry: Address,
    token: Address,
    bridge_type: BridgeType,
    rpc_url: &str,
    private_key: B256,
) -> Result<()>;

/// Add destination chain for a token
pub async fn add_token_dest_chain(
    token_registry: Address,
    token: Address,
    dest_chain_key: B256,
    dest_token_address: B256,
    decimals: u8,
    rpc_url: &str,
    private_key: B256,
) -> Result<()>;
```

## Test Token Deployment

```rust
/// Deploy a test ERC20 token
pub async fn deploy_test_token(
    project_root: &PathBuf,
    rpc_url: &str,
    private_key: &str,
) -> Result<Option<Address>>;
```

## Implementation Notes

1. Use `std::process::Command` for forge script execution
2. Parse broadcast JSON with serde
3. Use alloy for contract calls (role granting, registration)
4. Handle "already exists" errors gracefully (idempotent)
5. Log all deployment steps with `tracing`

## Constraints

- No `.unwrap()` - use `?` operator
- Use `eyre::Result` with context
- All addresses must be typed as `Address`
- All keys must be typed as `B256`
- Return early on deployment failure