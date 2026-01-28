---
context_files: []
output_dir: src/writers/
output_file: evm.rs
depends_on:
  - relayer_008_writers_mod
---

# EVM Transaction Writer

## Requirements

Implement an EVM writer that submits `approveWithdraw` transactions to the CL8YBridge contract for Terra -> EVM transfers.

## CL8YBridge.approveWithdraw

```solidity
function approveWithdraw(
    bytes32 srcChainKey,
    address token,
    address to,
    uint256 amount,
    uint256 nonce,
    uint256 fee,
    address feeRecipient,
    bool deductFromAmount
) external;
```

## EvmWriter Structure

```rust
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, U256, Bytes};
use sqlx::PgPool;
use eyre::Result;

pub struct EvmWriter {
    provider: Box<dyn Provider>,
    signer: PrivateKeySigner,
    bridge_address: Address,
    chain_id: u64,
    default_fee_bps: u32,
    fee_recipient: Address,
    db: PgPool,
}
```

## Implementation

```rust
impl EvmWriter {
    /// Create a new EVM writer
    pub async fn new(
        evm_config: &EvmConfig,
        fee_config: &FeeConfig,
        db: PgPool,
    ) -> Result<Self>;
    
    /// Process pending approvals from database
    pub async fn process_pending(&self) -> Result<()>;
    
    /// Submit an approval transaction
    async fn submit_approval(&self, approval: &Approval) -> Result<String>;
    
    /// Build the approveWithdraw calldata
    fn build_approve_calldata(
        &self,
        src_chain_key: &[u8],
        token: &Address,
        to: &Address,
        amount: U256,
        nonce: u64,
        fee: U256,
        fee_recipient: &Address,
        deduct_from_amount: bool,
    ) -> Bytes;
    
    /// Wait for transaction confirmation
    async fn wait_for_confirmation(&self, tx_hash: &str) -> Result<bool>;
}
```

## Process Pending Logic

```rust
pub async fn process_pending(&self) -> Result<()> {
    // Get pending Terra deposits that need approval
    let deposits = crate::db::get_pending_terra_deposits(&self.db).await?;
    
    for deposit in deposits {
        // Check if approval already exists
        let src_chain_key = ChainKey::cosmos(&deposit.token, "terra").as_bytes().to_vec();
        
        if crate::db::approval_exists(
            &self.db,
            &src_chain_key,
            deposit.nonce,
            self.chain_id as i64,
        ).await? {
            // Already have approval, update deposit status
            crate::db::update_terra_deposit_status(&self.db, deposit.id, "processed").await?;
            continue;
        }
        
        // Create approval record
        let fee = self.calculate_fee(&deposit.amount);
        let deduct_from_amount = self.should_deduct_from_amount(&deposit.token);
        
        let token = EvmAddress::from_hex(&deposit.recipient)?;
        let withdraw_hash = WithdrawHash::compute(
            &ChainKey::from_bytes(&src_chain_key),
            &token,
            &EvmAddress::from_hex(&deposit.recipient)?,
            &deposit.amount,
            deposit.nonce as u64,
        );
        
        let new_approval = NewApproval {
            src_chain_key: src_chain_key.clone(),
            nonce: deposit.nonce,
            dest_chain_id: self.chain_id as i64,
            withdraw_hash: withdraw_hash.as_bytes().to_vec(),
            token: deposit.recipient.clone(), // EVM token address
            recipient: deposit.recipient.clone(),
            amount: deposit.amount.clone(),
            fee: fee.clone(),
            fee_recipient: Some(self.fee_recipient.to_hex()),
            deduct_from_amount,
        };
        
        let approval_id = crate::db::insert_approval(&self.db, &new_approval).await?;
        
        // Submit transaction
        match self.submit_approval_by_id(approval_id).await {
            Ok(tx_hash) => {
                tracing::info!(
                    tx_hash = %tx_hash,
                    nonce = deposit.nonce,
                    "Submitted approval transaction"
                );
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    nonce = deposit.nonce,
                    "Failed to submit approval"
                );
                crate::db::update_approval_failed(&self.db, approval_id, &e.to_string()).await?;
            }
        }
        
        // Update deposit status
        crate::db::update_terra_deposit_status(&self.db, deposit.id, "processed").await?;
    }
    
    Ok(())
}
```

## Transaction Submission

```rust
async fn submit_approval(&self, approval: &Approval) -> Result<String> {
    let src_chain_key: [u8; 32] = approval.src_chain_key.clone().try_into()
        .map_err(|_| eyre!("Invalid src_chain_key length"))?;
    
    let token = Address::from_hex(&approval.token)?;
    let to = Address::from_hex(&approval.recipient)?;
    let amount = U256::from_str(&approval.amount.to_string())?;
    let fee = U256::from_str(&approval.fee.to_string())?;
    let fee_recipient = approval.fee_recipient.as_ref()
        .map(|s| Address::from_hex(s))
        .transpose()?
        .unwrap_or(self.fee_recipient);
    
    let calldata = self.build_approve_calldata(
        &src_chain_key,
        &token,
        &to,
        amount,
        approval.nonce as u64,
        fee,
        &fee_recipient,
        approval.deduct_from_amount,
    );
    
    // Build and send transaction
    let tx = TransactionRequest::default()
        .to(self.bridge_address)
        .input(calldata.into());
    
    let pending_tx = self.provider
        .send_transaction(tx)
        .await
        .wrap_err("Failed to send transaction")?;
    
    let tx_hash = format!("0x{}", hex::encode(pending_tx.tx_hash()));
    
    // Update approval with tx_hash
    crate::db::update_approval_submitted(&self.db, approval.id, &tx_hash).await?;
    
    // Wait for confirmation
    if self.wait_for_confirmation(&tx_hash).await? {
        crate::db::update_approval_confirmed(&self.db, approval.id).await?;
    }
    
    Ok(tx_hash)
}
```

## Constraints

- Use `alloy` for EVM interactions
- Use `tracing` for structured logging
- Use `eyre::Result` for error handling
- Retry failed transactions based on config
- Track attempt count and last attempt time
- No `unwrap()` calls
- Handle nonce management properly

## Dependencies

```rust
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, U256, Bytes, FixedBytes};
use alloy::rpc::types::TransactionRequest;
use bigdecimal::BigDecimal;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{info, error};

use crate::types::{ChainKey, EvmAddress, WithdrawHash};
use crate::config::{EvmConfig, FeeConfig};
use crate::db::{self, Approval, NewApproval};
```
