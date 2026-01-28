---
context_files:
  - src/config.rs
output_dir: src/
output_file: multi_evm.rs
---

# Multi-EVM Chain Configuration

Create a module for supporting multiple EVM chain configurations for EVM-to-EVM bridging.

## Requirements

1. Create `EvmChainConfig` struct for individual chain config:
   - `name: String` - Human-readable name (e.g., "ethereum", "bsc")
   - `chain_id: u64` - Chain ID
   - `rpc_url: String` - RPC endpoint
   - `bridge_address: String` - Bridge contract address
   - `router_address: Option<String>` - Optional router address
   - `finality_blocks: u64` - Required confirmations (default 12)
   - `enabled: bool` - Whether this chain is active (default true)

2. Create `MultiEvmConfig` struct:
   - `chains: Vec<EvmChainConfig>` - List of configured chains
   - `private_key: String` - Shared relayer private key (used for all chains)

3. Methods for `MultiEvmConfig`:
   - `get_chain(&self, chain_id: u64) -> Option<&EvmChainConfig>`
   - `get_chain_by_name(&self, name: &str) -> Option<&EvmChainConfig>`
   - `enabled_chains(&self) -> impl Iterator<Item = &EvmChainConfig>`
   - `chain_ids(&self) -> Vec<u64>`
   - `validate(&self) -> Result<()>` - Validate all chain configs

4. Create `ChainKey` helpers:
   - Method to compute chain key from EvmChainConfig
   - Method to look up chain by chain key

## Environment Loading

Support loading from environment variables:
```
EVM_CHAINS_COUNT=3

EVM_CHAIN_1_NAME=ethereum
EVM_CHAIN_1_CHAIN_ID=1
EVM_CHAIN_1_RPC_URL=https://eth.llamarpc.com
EVM_CHAIN_1_BRIDGE_ADDRESS=0x...

EVM_CHAIN_2_NAME=bsc
EVM_CHAIN_2_CHAIN_ID=56
EVM_CHAIN_2_RPC_URL=https://bsc-dataseed.binance.org
EVM_CHAIN_2_BRIDGE_ADDRESS=0x...

# etc.
```

## Imports Needed

```rust
use eyre::{eyre, Result, WrapErr};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;

use crate::types::ChainKey;
```

## Struct Definitions

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct EvmChainConfig {
    pub name: String,
    pub chain_id: u64,
    pub rpc_url: String,
    pub bridge_address: String,
    #[serde(default)]
    pub router_address: Option<String>,
    #[serde(default = "default_finality_blocks")]
    pub finality_blocks: u64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct MultiEvmConfig {
    chains: Vec<EvmChainConfig>,
    chain_key_map: HashMap<ChainKey, usize>,
    private_key: String,
}

fn default_finality_blocks() -> u64 { 12 }
fn default_enabled() -> bool { true }
```

## Validation

The validate method should:
- Ensure no duplicate chain IDs
- Validate all bridge addresses are valid EVM addresses
- Ensure at least one chain is configured
- Validate private key format

## Integration Notes

This module is intended to eventually replace the single EvmConfig when multi-EVM support is enabled. For now it can coexist with the existing config and be used optionally.
