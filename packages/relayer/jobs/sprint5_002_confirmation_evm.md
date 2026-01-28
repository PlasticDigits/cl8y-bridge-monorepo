---
context_files:
  - src/db/models.rs
  - src/confirmation/mod.rs
output_dir: src/confirmation/
output_file: evm.rs
---

# Complete EVM Confirmation Checker

## Overview

Replace the placeholder EVM confirmation checker with real RPC polling that:
- Queries the EVM RPC for transaction receipts
- Verifies block confirmations
- Detects chain reorgs
- Handles RPC errors gracefully

## Requirements

### 1. RPC Client

Add an RPC client to query transaction status. Use `reqwest` for HTTP calls:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
```

### 2. Transaction Receipt Query

Implement `eth_getTransactionReceipt` RPC call:

```rust
#[derive(Debug, Deserialize)]
struct TransactionReceipt {
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    #[serde(rename = "blockNumber")]
    block_number: Option<String>,  // hex string
    #[serde(rename = "blockHash")]
    block_hash: Option<String>,
    status: Option<String>,  // "0x1" for success, "0x0" for failure
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}
```

### 3. Block Number Query

Implement `eth_blockNumber` RPC call to get current block height.

### 4. Confirmation Logic

```rust
pub async fn check_approval_confirmation(&self, approval: &Approval) -> Result<ConfirmationResult> {
    let tx_hash = approval.tx_hash.as_ref()
        .ok_or_else(|| eyre!("Approval has no tx_hash"))?;
    
    // 1. Get transaction receipt
    let receipt = self.get_transaction_receipt(tx_hash).await?;
    
    // 2. If no receipt, transaction is still pending
    if receipt.is_none() {
        return Ok(ConfirmationResult::Pending);
    }
    
    let receipt = receipt.unwrap();
    
    // 3. Check if transaction failed
    if receipt.status == Some("0x0".to_string()) {
        return Ok(ConfirmationResult::Failed);
    }
    
    // 4. Get current block number
    let current_block = self.get_block_number().await?;
    
    // 5. Calculate confirmations
    let tx_block = u64::from_str_radix(&receipt.block_number.unwrap_or_default().trim_start_matches("0x"), 16)?;
    let confirmations = current_block.saturating_sub(tx_block);
    
    // 6. Check if enough confirmations
    if confirmations >= self.required_confirmations as u64 {
        return Ok(ConfirmationResult::Confirmed);
    }
    
    Ok(ConfirmationResult::WaitingConfirmations(confirmations as u32))
}
```

### 5. ConfirmationResult Enum

```rust
pub enum ConfirmationResult {
    /// Transaction is pending (no receipt yet)
    Pending,
    /// Transaction confirmed with enough blocks
    Confirmed,
    /// Waiting for more confirmations
    WaitingConfirmations(u32),
    /// Transaction failed on-chain
    Failed,
    /// Transaction was reorged (no longer in chain)
    Reorged,
}
```

### 6. Configuration

The struct should store:
- `db: PgPool` - Database pool
- `required_confirmations: u32` - Required block confirmations
- `rpc_url: String` - EVM RPC endpoint URL
- `client: reqwest::Client` - HTTP client for RPC calls

### 7. Constructor

```rust
pub fn new(db: PgPool, required_confirmations: u32, rpc_url: String) -> Result<Self> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    
    Ok(Self {
        db,
        required_confirmations,
        rpc_url,
        client,
    })
}
```

### 8. RPC Helper Methods

```rust
async fn get_transaction_receipt(&self, tx_hash: &str) -> Result<Option<TransactionReceipt>> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
        "id": 1
    });
    
    let response = self.client
        .post(&self.rpc_url)
        .json(&body)
        .send()
        .await?
        .json::<RpcResponse<TransactionReceipt>>()
        .await?;
    
    if let Some(error) = response.error {
        return Err(eyre!("RPC error: {} - {}", error.code, error.message));
    }
    
    Ok(response.result)
}

async fn get_block_number(&self) -> Result<u64> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });
    
    let response = self.client
        .post(&self.rpc_url)
        .json(&body)
        .send()
        .await?
        .json::<RpcResponse<String>>()
        .await?;
    
    let hex = response.result.ok_or_else(|| eyre!("No block number returned"))?;
    let block = u64::from_str_radix(hex.trim_start_matches("0x"), 16)?;
    
    Ok(block)
}
```

### 9. Imports

```rust
#![allow(dead_code)]

use eyre::{eyre, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, info, warn};

use crate::db::Approval;
```
