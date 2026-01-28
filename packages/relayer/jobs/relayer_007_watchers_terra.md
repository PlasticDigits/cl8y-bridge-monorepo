---
context_files: []
output_dir: src/watchers/
output_file: terra.rs
depends_on:
  - relayer_005_watchers_mod
---

# Terra Classic Transaction Watcher

## Requirements

Implement a Terra Classic watcher that polls for Lock transactions on the bridge contract and stores them in the database.

## Lock Transaction Attributes

When a user locks tokens, the Terra bridge contract emits these attributes:

```
method: "lock" or "lock_cw20"
nonce: u64
sender: String (terra address)
recipient: String (EVM address)
token: String (denom or CW20 address)
amount: Uint128
dest_chain_id: u64
fee: Uint128
```

## TerraWatcher Structure

```rust
use tendermint_rpc::{Client, HttpClient};
use sqlx::PgPool;
use eyre::Result;

pub struct TerraWatcher {
    rpc_client: HttpClient,
    lcd_url: String,
    bridge_address: String,
    chain_id: String,
    db: PgPool,
}
```

## Implementation

```rust
impl TerraWatcher {
    /// Create a new Terra watcher
    pub async fn new(config: &TerraConfig, db: PgPool) -> Result<Self>;
    
    /// Run the watcher loop
    pub async fn run(&self) -> Result<()>;
    
    /// Process transactions in a block
    async fn process_block(&self, height: u64) -> Result<()>;
    
    /// Parse lock attributes from a transaction
    fn parse_lock_tx(&self, tx_response: &TxResponse) -> Result<Option<NewTerraDeposit>>;
    
    /// Get the current block height
    async fn get_current_height(&self) -> Result<u64>;
}
```

## Run Loop Logic

```rust
pub async fn run(&self) -> Result<()> {
    let poll_interval = Duration::from_millis(1000);
    
    loop {
        // Get last processed height from DB
        let last_height = crate::db::get_last_terra_block(&self.db, &self.chain_id)
            .await?
            .unwrap_or(0);
        
        // Get current height
        let current_height = self.get_current_height().await?;
        
        // Skip if no new blocks
        if current_height <= last_height as u64 {
            tokio::time::sleep(poll_interval).await;
            continue;
        }
        
        // Process new blocks one at a time
        for height in (last_height + 1) as u64..=current_height {
            tracing::info!(
                chain_id = %self.chain_id,
                height,
                "Processing Terra block"
            );
            
            self.process_block(height).await?;
            
            // Update last processed height
            crate::db::update_last_terra_block(&self.db, &self.chain_id, height as i64)
                .await?;
        }
        
        tokio::time::sleep(poll_interval).await;
    }
}
```

## Block Processing

```rust
async fn process_block(&self, height: u64) -> Result<()> {
    // Query block results to get transactions
    let block_results = self.rpc_client
        .block_results(height as u32)
        .await
        .wrap_err("Failed to get block results")?;
    
    // Also query via LCD for transaction details
    let url = format!(
        "{}/cosmos/tx/v1beta1/txs?events=wasm._contract_address='{}'&events=tx.height={}",
        self.lcd_url,
        self.bridge_address,
        height
    );
    
    let response: TxSearchResponse = reqwest::get(&url)
        .await
        .wrap_err("Failed to query transactions")?
        .json()
        .await
        .wrap_err("Failed to parse transaction response")?;
    
    for tx in response.tx_responses {
        if let Some(deposit) = self.parse_lock_tx(&tx)? {
            // Check if already exists
            if !crate::db::terra_deposit_exists(&self.db, &deposit.tx_hash, deposit.nonce).await? {
                crate::db::insert_terra_deposit(&self.db, &deposit).await?;
                tracing::info!(
                    tx_hash = %deposit.tx_hash,
                    nonce = deposit.nonce,
                    "Stored Terra lock transaction"
                );
            }
        }
    }
    
    Ok(())
}
```

## Transaction Parsing

```rust
fn parse_lock_tx(&self, tx: &TxResponse) -> Result<Option<NewTerraDeposit>> {
    // Find wasm events for our bridge contract
    for event in &tx.events {
        if event.type_str != "wasm" {
            continue;
        }
        
        // Check if this is our bridge contract
        let contract_addr = event.attributes.iter()
            .find(|a| a.key == "_contract_address")
            .map(|a| &a.value);
        
        if contract_addr != Some(&self.bridge_address) {
            continue;
        }
        
        // Check if this is a lock method
        let method = event.attributes.iter()
            .find(|a| a.key == "method")
            .map(|a| a.value.as_str());
        
        if method != Some("lock") && method != Some("lock_cw20") {
            continue;
        }
        
        // Extract attributes
        let nonce = extract_u64(&event.attributes, "nonce")?;
        let sender = extract_string(&event.attributes, "sender")?;
        let recipient = extract_string(&event.attributes, "recipient")?;
        let token = extract_string(&event.attributes, "token")?;
        let amount = extract_decimal(&event.attributes, "amount")?;
        let dest_chain_id = extract_u64(&event.attributes, "dest_chain_id")?;
        
        return Ok(Some(NewTerraDeposit {
            tx_hash: tx.txhash.clone(),
            nonce: nonce as i64,
            sender,
            recipient,
            token,
            amount,
            dest_chain_id: dest_chain_id as i64,
            block_height: tx.height as i64,
        }));
    }
    
    Ok(None)
}

fn extract_string(attrs: &[Attribute], key: &str) -> Result<String> {
    attrs.iter()
        .find(|a| a.key == key)
        .map(|a| a.value.clone())
        .ok_or_else(|| eyre!("Missing attribute: {}", key))
}

fn extract_u64(attrs: &[Attribute], key: &str) -> Result<u64> {
    extract_string(attrs, key)?
        .parse()
        .wrap_err_with(|| format!("Invalid u64 for {}", key))
}

fn extract_decimal(attrs: &[Attribute], key: &str) -> Result<BigDecimal> {
    BigDecimal::from_str(&extract_string(attrs, key)?)
        .wrap_err_with(|| format!("Invalid decimal for {}", key))
}
```

## Response Types

```rust
#[derive(Debug, Deserialize)]
struct TxSearchResponse {
    tx_responses: Vec<TxResponse>,
}

#[derive(Debug, Deserialize)]
struct TxResponse {
    txhash: String,
    height: i64,
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct Event {
    #[serde(rename = "type")]
    type_str: String,
    attributes: Vec<Attribute>,
}

#[derive(Debug, Deserialize)]
struct Attribute {
    key: String,
    value: String,
}
```

## Constraints

- Use `tendermint-rpc` for RPC calls
- Use `reqwest` for LCD REST API calls
- Use `tracing` for structured logging
- Use `eyre::Result` for error handling
- Handle connection errors with retry
- Skip duplicate deposits (check DB before insert)
- Process blocks sequentially to maintain order
- No `unwrap()` calls

## Dependencies

```rust
use bigdecimal::BigDecimal;
use eyre::{eyre, Result, WrapErr};
use reqwest;
use serde::Deserialize;
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;
use tendermint_rpc::{Client, HttpClient};
use tracing::{info, warn, error};
```
