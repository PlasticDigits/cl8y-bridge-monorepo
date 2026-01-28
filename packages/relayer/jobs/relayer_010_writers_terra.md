---
context_files: []
output_dir: src/writers/
output_file: terra.rs
depends_on:
  - relayer_008_writers_mod
---

# Terra Classic Transaction Writer

## Requirements

Implement a Terra writer that submits `Release` transactions to the Terra bridge contract for EVM -> Terra transfers.

## Bridge Release Message

```rust
// ExecuteMsg::Release
{
    "release": {
        "nonce": u64,
        "sender": String,      // EVM sender address
        "recipient": String,   // Terra recipient address
        "token": String,       // Token denom or CW20 address
        "amount": Uint128,
        "source_chain_id": u64,
        "signatures": Vec<String>  // Relayer signatures
    }
}
```

## TerraWriter Structure

```rust
use cosmrs::tx::{Msg, SignDoc, SignerInfo};
use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::AccountId;
use sqlx::PgPool;
use eyre::Result;

pub struct TerraWriter {
    signing_key: SigningKey,
    account: AccountId,
    rpc_url: String,
    lcd_url: String,
    bridge_address: String,
    chain_id: String,
    db: PgPool,
}
```

## Implementation

```rust
impl TerraWriter {
    /// Create a new Terra writer
    pub async fn new(config: &TerraConfig, db: PgPool) -> Result<Self>;
    
    /// Process pending releases from database
    pub async fn process_pending(&self) -> Result<()>;
    
    /// Submit a release transaction
    async fn submit_release(&self, release: &Release) -> Result<String>;
    
    /// Build the release message
    fn build_release_msg(
        &self,
        nonce: u64,
        sender: &str,
        recipient: &str,
        token: &str,
        amount: &str,
        source_chain_id: u64,
        signatures: Vec<String>,
    ) -> Result<cosmwasm_std::CosmosMsg>;
    
    /// Sign a release (for multi-sig, returns signature)
    fn sign_release(
        &self,
        nonce: u64,
        sender: &str,
        recipient: &str,
        token: &str,
        amount: &str,
        source_chain_id: u64,
    ) -> Result<String>;
    
    /// Wait for transaction confirmation
    async fn wait_for_confirmation(&self, tx_hash: &str) -> Result<bool>;
    
    /// Get account sequence for signing
    async fn get_account_sequence(&self) -> Result<u64>;
}
```

## Process Pending Logic

```rust
pub async fn process_pending(&self) -> Result<()> {
    // Get pending EVM deposits that need release
    let deposits = crate::db::get_pending_evm_deposits(&self.db).await?;
    
    for deposit in deposits {
        // Check if release already exists
        if crate::db::release_exists(
            &self.db,
            &deposit.dest_chain_key,
            deposit.nonce,
        ).await? {
            // Already have release, update deposit status
            crate::db::update_evm_deposit_status(&self.db, deposit.id, "processed").await?;
            continue;
        }
        
        // Decode destination account from bytes32
        let recipient = self.decode_terra_address(&deposit.dest_account)?;
        let token = self.decode_token(&deposit.dest_token_address)?;
        
        // Create release record
        let new_release = NewRelease {
            src_chain_key: deposit.dest_chain_key.clone(),
            nonce: deposit.nonce,
            sender: format!("0x{}", hex::encode(&deposit.token)),
            recipient: recipient.clone(),
            token: token.clone(),
            amount: deposit.amount.clone(),
            source_chain_id: deposit.chain_id,
        };
        
        let release_id = crate::db::insert_release(&self.db, &new_release).await?;
        
        // Submit transaction
        match self.submit_release_by_id(release_id).await {
            Ok(tx_hash) => {
                tracing::info!(
                    tx_hash = %tx_hash,
                    nonce = deposit.nonce,
                    "Submitted release transaction"
                );
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    nonce = deposit.nonce,
                    "Failed to submit release"
                );
                crate::db::update_release_failed(&self.db, release_id, &e.to_string()).await?;
            }
        }
        
        // Update deposit status
        crate::db::update_evm_deposit_status(&self.db, deposit.id, "processed").await?;
    }
    
    Ok(())
}
```

## Transaction Submission

```rust
async fn submit_release(&self, release: &Release) -> Result<String> {
    // For single relayer mode, we sign and submit directly
    let signature = self.sign_release(
        release.nonce as u64,
        &release.sender,
        &release.recipient,
        &release.token,
        &release.amount.to_string(),
        release.source_chain_id as u64,
    )?;
    
    // Build the release message
    let msg = serde_json::json!({
        "release": {
            "nonce": release.nonce,
            "sender": release.sender,
            "recipient": release.recipient,
            "token": release.token,
            "amount": release.amount.to_string(),
            "source_chain_id": release.source_chain_id,
            "signatures": [signature]
        }
    });
    
    // Submit via LCD API
    let tx_hash = self.broadcast_execute_msg(&msg).await?;
    
    // Update release with tx_hash
    crate::db::update_release_submitted(&self.db, release.id, &tx_hash).await?;
    
    // Wait for confirmation
    if self.wait_for_confirmation(&tx_hash).await? {
        crate::db::update_release_confirmed(&self.db, release.id).await?;
    }
    
    Ok(tx_hash)
}

async fn broadcast_execute_msg(&self, msg: &serde_json::Value) -> Result<String> {
    // Get account info for sequence
    let sequence = self.get_account_sequence().await?;
    
    // Build the transaction
    // This is simplified - actual implementation needs proper cosmrs TX building
    let execute_msg = cosmwasm_std::CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
        contract_addr: self.bridge_address.clone(),
        msg: cosmwasm_std::to_json_binary(msg)?,
        funds: vec![],
    });
    
    // Sign and broadcast
    // ... (use cosmrs for actual signing and broadcasting)
    
    todo!("Implement actual TX signing and broadcasting with cosmrs")
}
```

## Helper Functions

```rust
fn decode_terra_address(&self, bytes32: &[u8]) -> Result<String> {
    // Terra addresses are bech32 encoded
    // The bytes32 contains the address in the last 20 bytes (like EVM)
    // But Terra uses different encoding
    
    // For now, assume it's stored as a string in the bytes
    let trimmed: Vec<u8> = bytes32.iter()
        .skip_while(|&&b| b == 0)
        .copied()
        .collect();
    
    String::from_utf8(trimmed)
        .wrap_err("Invalid Terra address encoding")
}

fn decode_token(&self, bytes32: &[u8]) -> Result<String> {
    // Token can be a denom (like "uluna") or CW20 address
    let trimmed: Vec<u8> = bytes32.iter()
        .skip_while(|&&b| b == 0)
        .copied()
        .collect();
    
    String::from_utf8(trimmed)
        .wrap_err("Invalid token encoding")
}
```

## Constraints

- Use `cosmrs` for Cosmos transaction building and signing
- Use `cosmwasm-std` for message types
- Use `bip39` for mnemonic handling
- Use `tracing` for structured logging
- Use `eyre::Result` for error handling
- Retry failed transactions based on config
- Track attempt count and last attempt time
- No `unwrap()` calls
- Handle sequence numbers properly to avoid conflicts

## Dependencies

```rust
use bip39::Mnemonic;
use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::tx::SignerInfo;
use cosmrs::AccountId;
use cosmwasm_std;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use tracing::{info, error};

use crate::config::TerraConfig;
use crate::db::{self, Release, NewRelease};
```
