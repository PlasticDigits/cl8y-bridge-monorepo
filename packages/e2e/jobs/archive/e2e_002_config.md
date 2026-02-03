---
output_dir: src/
output_file: config.rs
verify: true
---

# Configuration Module for E2E Tests

## Requirements

Create a typed configuration module that replaces bash environment variables with compile-time safe structs.

This replaces the `.env.e2e` file and bash variable handling from `scripts/e2e-setup.sh`.

## Core Structs

### E2eConfig (root configuration)

```rust
pub struct E2eConfig {
    pub evm: EvmConfig,
    pub terra: TerraConfig,
    pub docker: DockerConfig,
    pub operator: OperatorConfig,
    pub test_accounts: TestAccounts,
}
```

### EvmConfig

```rust
use alloy::primitives::{Address, B256};
use url::Url;

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
```

### TerraConfig

```rust
pub struct TerraConfig {
    pub rpc_url: Url,
    pub lcd_url: Url,
    pub chain_id: String,
    pub bridge_address: Option<String>,  // terra1... address
    pub mnemonic: Option<String>,
}
```

### DockerConfig

```rust
pub struct DockerConfig {
    pub compose_profile: String,
    pub anvil_port: u16,
    pub postgres_port: u16,
    pub terra_rpc_port: u16,
    pub terra_lcd_port: u16,
}
```

### OperatorConfig

```rust
pub struct OperatorConfig {
    pub database_url: Url,
    pub finality_blocks: u64,
    pub poll_interval_ms: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}
```

### TestAccounts

```rust
pub struct TestAccounts {
    pub evm_address: Address,
    pub evm_private_key: B256,
    pub terra_address: String,
    pub terra_key_name: String,
}
```

## Implementation Requirements

### E2eConfig::default()

Create default configuration matching the bash defaults:
- EVM RPC: `http://localhost:8545`
- Chain ID: 31337 (Anvil)
- Terra RPC: `http://localhost:26657`
- Terra LCD: `http://localhost:1317`
- Postgres: `postgres://operator:operator@localhost:5433/operator`

### E2eConfig::from_env()

Load configuration from environment variables, falling back to defaults.
Environment variable names should match the bash script (e.g., `EVM_RPC_URL`, `TERRA_RPC_URL`).

### E2eConfig::from_broadcast(path: &Path)

Parse a forge broadcast JSON file to extract deployed contract addresses.
This replaces the `jq` extraction in bash.

## Constraints

- All addresses MUST be `Address` type, not `String`
- All private keys MUST be `B256` type
- Use `eyre::Result` for all fallible operations
- No `.unwrap()` calls - use `?` operator
- Implement `Default` trait for all config structs
- Use `#[derive(Debug, Clone)]` on all structs
