---
output_dir: src/
output_file: terra.rs
context_files:
  - src/config.rs
  - src/docker.rs
verify: true
depends_on:
  - e2e_002_config
  - e2e_004_docker
---

# Terra Chain Interactions Module

## Requirements

Create a module for interacting with LocalTerra via Docker exec and LCD API.
This replaces terrad commands from bash scripts.

## Core Struct

```rust
use crate::config::TerraConfig;
use eyre::Result;

/// Terra chain client for E2E testing
pub struct TerraClient {
    lcd_url: url::Url,
    rpc_url: url::Url,
    chain_id: String,
    container_name: String,
    key_name: String,
}
```

## Required Methods

### new()
```rust
impl TerraClient {
    /// Create a new TerraClient from configuration
    pub fn new(config: &TerraConfig) -> Self;
    
    /// Create with explicit container name
    pub fn with_container(config: &TerraConfig, container_name: &str) -> Self;
}
```

### Health Checks
```rust
/// Check if Terra is responding (LCD status endpoint)
pub async fn is_healthy(&self) -> Result<bool>;

/// Get current block height
pub async fn get_block_height(&self) -> Result<u64>;

/// Check if chain is syncing
pub async fn is_syncing(&self) -> Result<bool>;
```

### Contract Queries (via LCD REST API)
```rust
/// Query a CosmWasm contract
/// Uses: GET /cosmwasm/wasm/v1/contract/{address}/smart/{query_b64}
pub async fn query_contract<T: serde::de::DeserializeOwned>(
    &self,
    contract_address: &str,
    query: &serde_json::Value,
) -> Result<T>;

/// Get contract info
pub async fn get_contract_info(&self, contract_address: &str) -> Result<ContractInfo>;
```

### Account Queries
```rust
/// Get account balance for a denom
pub async fn get_balance(&self, address: &str, denom: &str) -> Result<u128>;

/// Get all balances for an address
pub async fn get_all_balances(&self, address: &str) -> Result<Vec<Coin>>;
```

### Docker Exec Commands (for transactions)
```rust
/// Execute terrad command in container
/// Returns stdout as String
async fn exec_terrad(&self, args: &[&str]) -> Result<String>;

/// Store a WASM contract code
/// Returns code_id
pub async fn store_code(&self, wasm_path: &str) -> Result<u64>;

/// Instantiate a contract
/// Returns contract address
pub async fn instantiate_contract(
    &self,
    code_id: u64,
    init_msg: &serde_json::Value,
    label: &str,
    admin: Option<&str>,
) -> Result<String>;

/// Execute a contract message
pub async fn execute_contract(
    &self,
    contract_address: &str,
    msg: &serde_json::Value,
    funds: Option<&str>,
) -> Result<String>; // Returns tx hash

/// Wait for transaction confirmation
pub async fn wait_for_tx(&self, tx_hash: &str, timeout: std::time::Duration) -> Result<TxResult>;
```

### Bridge-Specific Operations
```rust
/// Lock tokens on Terra bridge (for cross-chain transfer)
pub async fn lock_tokens(
    &self,
    bridge_address: &str,
    dest_chain_id: u64,
    recipient: &str,
    amount: u128,
    denom: &str,
) -> Result<String>; // Returns tx hash

/// Query pending approvals on Terra bridge
pub async fn get_pending_approvals(
    &self,
    bridge_address: &str,
    limit: u32,
) -> Result<Vec<PendingApproval>>;

/// Query withdraw delay from Terra bridge
pub async fn get_withdraw_delay(&self, bridge_address: &str) -> Result<u64>;
```

## Types

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContractInfo {
    pub address: String,
    pub code_id: u64,
    pub creator: String,
    pub admin: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Coin {
    pub denom: String,
    pub amount: String,
}

#[derive(Debug, Clone)]
pub struct TxResult {
    pub tx_hash: String,
    pub height: u64,
    pub success: bool,
    pub raw_log: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PendingApproval {
    pub xchain_hash_id: String,
    pub recipient: String,
    pub amount: String,
    pub created_at: u64,
}
```

## Implementation Notes

1. Use `reqwest` for LCD REST API calls
2. Use `std::process::Command` via docker exec for terrad transactions
3. Base64 encode queries for smart contract queries
4. Parse JSON responses with typed structs
5. Handle LocalTerra-specific quirks (slow block times, eventual consistency)

## Constraints

- No `.unwrap()` - use `?` operator
- Use `eyre::Result` for all errors
- Log all operations with `tracing`
- Timeout all network operations
- Default container name: `cl8y-bridge-monorepo-localterra-1`