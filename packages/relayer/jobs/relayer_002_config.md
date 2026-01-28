---
context_files: []
output_dir: src/
output_file: config.rs
depends_on:
  - relayer_001_types
---

# Configuration Module for CL8Y Bridge Relayer

## Requirements

Implement configuration loading from environment variables and optional config file.
The configuration should be validated on load and provide sensible defaults.

## Configuration Structure

```rust
/// Main configuration for the relayer
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub evm: EvmConfig,
    pub terra: TerraConfig,
    pub relayer: RelayerConfig,
    pub fees: FeeConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EvmConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub bridge_address: String,
    pub router_address: String,
    pub private_key: String,
    #[serde(default = "default_finality_blocks")]
    pub finality_blocks: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerraConfig {
    pub rpc_url: String,
    pub lcd_url: String,
    pub chain_id: String,
    pub bridge_address: String,
    pub mnemonic: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelayerConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeeConfig {
    #[serde(default = "default_fee_bps")]
    pub default_fee_bps: u32,
    pub fee_recipient: String,
}
```

## Functions to Implement

### Config::load

```rust
impl Config {
    /// Load configuration from environment variables
    /// Loads .env file if present, then reads from environment
    pub fn load() -> Result<Self>;

    /// Load from a specific .env file path
    pub fn load_from_file(path: &str) -> Result<Self>;

    /// Validate the configuration
    fn validate(&self) -> Result<()>;
}
```

### Default Functions

```rust
fn default_finality_blocks() -> u64 { 1 }
fn default_poll_interval() -> u64 { 1000 }
fn default_retry_attempts() -> u32 { 3 }
fn default_retry_delay() -> u64 { 5000 }
fn default_fee_bps() -> u32 { 30 }
```

## Validation Rules

1. `database.url` must be a valid PostgreSQL connection string
2. `evm.rpc_url` must be a valid URL
3. `evm.bridge_address` must be a valid hex address (42 chars with 0x)
4. `evm.private_key` must be 66 chars (0x + 64 hex chars)
5. `terra.mnemonic` must have at least 12 words
6. `fees.fee_recipient` must be a valid EVM address

## Environment Variable Mapping

| Config Path | Environment Variable |
|-------------|---------------------|
| database.url | DATABASE_URL |
| evm.rpc_url | EVM_RPC_URL |
| evm.chain_id | EVM_CHAIN_ID |
| evm.bridge_address | EVM_BRIDGE_ADDRESS |
| evm.router_address | EVM_ROUTER_ADDRESS |
| evm.private_key | EVM_PRIVATE_KEY |
| evm.finality_blocks | FINALITY_BLOCKS |
| terra.rpc_url | TERRA_RPC_URL |
| terra.lcd_url | TERRA_LCD_URL |
| terra.chain_id | TERRA_CHAIN_ID |
| terra.bridge_address | TERRA_BRIDGE_ADDRESS |
| terra.mnemonic | TERRA_MNEMONIC |
| relayer.poll_interval_ms | POLL_INTERVAL_MS |
| relayer.retry_attempts | RETRY_ATTEMPTS |
| relayer.retry_delay_ms | RETRY_DELAY_MS |
| fees.default_fee_bps | DEFAULT_FEE_BPS |
| fees.fee_recipient | FEE_RECIPIENT |

## Constraints

- Use `dotenvy` to load .env file
- Use `config` crate or manual env reading with `std::env`
- Use `eyre` for error handling with descriptive messages
- All validation errors should indicate which field failed and why
- No `unwrap()` calls

## Dependencies

```rust
use eyre::{eyre, Result, WrapErr};
use serde::Deserialize;
```
