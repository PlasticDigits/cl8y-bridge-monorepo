---
context_files: []
output_dir: src/watchers/
output_file: evm.rs
depends_on:
  - relayer_005_watchers_mod
---

# EVM Event Watcher

## Requirements

Implement an EVM event watcher that subscribes to `DepositRequest` events from the CL8YBridge contract and stores them in the database.

## DepositRequest Event

The CL8YBridge contract emits this event:

```solidity
event DepositRequest(
    bytes32 indexed destChainKey,
    bytes32 indexed destTokenAddress,
    bytes32 indexed destAccount,
    address token,
    uint256 amount,
    uint256 nonce
);
```

Event signature: `DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)`

## EvmWatcher Structure

```rust
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::Filter;
use sqlx::PgPool;
use eyre::Result;

pub struct EvmWatcher {
    provider: Box<dyn Provider>,
    bridge_address: Address,
    chain_id: u64,
    finality_blocks: u64,
    db: PgPool,
}
```

## Implementation

```rust
impl EvmWatcher {
    /// Create a new EVM watcher
    pub async fn new(config: &EvmConfig, db: PgPool) -> Result<Self>;
    
    /// Run the watcher loop
    pub async fn run(&self) -> Result<()>;
    
    /// Process logs from a block range
    async fn process_block_range(&self, from_block: u64, to_block: u64) -> Result<()>;
    
    /// Parse a DepositRequest log
    fn parse_deposit_log(&self, log: &Log) -> Result<NewEvmDeposit>;
    
    /// Get the current finalized block number
    async fn get_finalized_block(&self) -> Result<u64>;
    
    /// Compute the event signature hash
    fn deposit_request_signature() -> B256 {
        // keccak256("DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)")
        alloy::primitives::keccak256(
            b"DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)"
        )
    }
}
```

## Run Loop Logic

```rust
pub async fn run(&self) -> Result<()> {
    let poll_interval = Duration::from_millis(1000);
    
    loop {
        // Get last processed block from DB
        let last_block = crate::db::get_last_evm_block(&self.db, self.chain_id as i64)
            .await?
            .unwrap_or(0);
        
        // Get current finalized block
        let current_block = self.get_finalized_block().await?;
        
        // Skip if no new blocks
        if current_block <= last_block as u64 {
            tokio::time::sleep(poll_interval).await;
            continue;
        }
        
        // Process new blocks
        let from_block = (last_block + 1) as u64;
        let to_block = current_block;
        
        tracing::info!(
            chain_id = self.chain_id,
            from_block,
            to_block,
            "Processing EVM blocks"
        );
        
        self.process_block_range(from_block, to_block).await?;
        
        // Update last processed block
        crate::db::update_last_evm_block(&self.db, self.chain_id as i64, to_block as i64)
            .await?;
        
        tokio::time::sleep(poll_interval).await;
    }
}
```

## Log Parsing

```rust
fn parse_deposit_log(&self, log: &Log) -> Result<NewEvmDeposit> {
    // Indexed topics:
    // topics[0] = event signature
    // topics[1] = destChainKey (bytes32)
    // topics[2] = destTokenAddress (bytes32)
    // topics[3] = destAccount (bytes32)
    
    // Non-indexed data (abi encoded):
    // token (address)
    // amount (uint256)
    // nonce (uint256)
    
    let dest_chain_key = log.topics()[1].as_slice().to_vec();
    let dest_token_address = log.topics()[2].as_slice().to_vec();
    let dest_account = log.topics()[3].as_slice().to_vec();
    
    // Decode non-indexed data
    let data = log.data().data.as_ref();
    // First 32 bytes: token address (right-aligned in 32 bytes)
    // Next 32 bytes: amount
    // Next 32 bytes: nonce
    
    let token = Address::from_slice(&data[12..32]);
    let amount = U256::from_be_slice(&data[32..64]);
    let nonce = U256::from_be_slice(&data[64..96]);
    
    Ok(NewEvmDeposit {
        chain_id: self.chain_id as i64,
        tx_hash: format!("0x{}", hex::encode(log.transaction_hash.unwrap().as_slice())),
        log_index: log.log_index.unwrap() as i32,
        nonce: nonce.to::<u64>() as i64,
        dest_chain_key,
        dest_token_address,
        dest_account,
        token: format!("0x{}", hex::encode(token.as_slice())),
        amount: BigDecimal::from_str(&amount.to_string())?,
        block_number: log.block_number.unwrap() as i64,
        block_hash: format!("0x{}", hex::encode(log.block_hash.unwrap().as_slice())),
    })
}
```

## Constraints

- Use `alloy` crate for EVM interactions
- Use `tracing` for structured logging with fields
- Use `eyre::Result` for error handling
- Handle provider connection errors with retry
- Skip duplicate deposits (check DB before insert)
- Use finality_blocks to determine safe block height
- No `unwrap()` calls - use proper error handling
- Poll-based approach (not WebSocket subscriptions for reliability)

## Dependencies

```rust
use alloy::providers::{Provider, ProviderBuilder};
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{Filter, Log};
use bigdecimal::BigDecimal;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;
use tracing::{info, warn, error};
```
