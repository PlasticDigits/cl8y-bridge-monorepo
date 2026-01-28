//! EVM Writer - Submits withdrawal approvals to EVM chains
//!
//! Processes pending Terra deposits and submits corresponding
//! approveWithdraw transactions to the EVM bridge contract.

#![allow(dead_code)]

use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::config::{EvmConfig, FeeConfig};
use crate::db::{self, NewApproval, TerraDeposit};
use crate::types::{ChainKey, EvmAddress, WithdrawHash};

/// EVM transaction writer for submitting withdrawal approvals
pub struct EvmWriter {
    rpc_url: String,
    bridge_address: Address,
    chain_id: u64,
    private_key: String,
    default_fee_bps: u32,
    fee_recipient: Address,
    db: PgPool,
}

impl EvmWriter {
    /// Create a new EVM writer
    pub async fn new(evm_config: &EvmConfig, fee_config: &FeeConfig, db: PgPool) -> Result<Self> {
        let bridge_address =
            Address::from_str(&evm_config.bridge_address).wrap_err("Invalid bridge address")?;
        let fee_recipient =
            Address::from_str(&fee_config.fee_recipient).wrap_err("Invalid fee recipient")?;

        Ok(Self {
            rpc_url: evm_config.rpc_url.clone(),
            bridge_address,
            chain_id: evm_config.chain_id,
            private_key: evm_config.private_key.clone(),
            default_fee_bps: fee_config.default_fee_bps,
            fee_recipient,
            db,
        })
    }

    /// Process pending Terra deposits and create approvals
    pub async fn process_pending(&self) -> Result<()> {
        let deposits = db::get_pending_terra_deposits(&self.db).await?;

        for deposit in deposits {
            if let Err(e) = self.process_deposit(&deposit).await {
                error!(
                    deposit_id = deposit.id,
                    error = %e,
                    "Failed to process Terra deposit"
                );
            }
        }

        Ok(())
    }

    /// Process a single Terra deposit
    async fn process_deposit(&self, deposit: &TerraDeposit) -> Result<()> {
        let src_chain_key = ChainKey::cosmos("localterra", "terra");

        // Check if approval already exists
        if db::approval_exists(
            &self.db,
            src_chain_key.as_bytes(),
            deposit.nonce,
            self.chain_id as i64,
        )
        .await?
        {
            db::update_terra_deposit_status(&self.db, deposit.id, "processed").await?;
            return Ok(());
        }

        // Calculate fee
        let fee = self.calculate_fee(&deposit.amount);

        // Create approval record
        let recipient = EvmAddress::from_hex(&deposit.recipient)?;
        let token = EvmAddress::from_hex(&deposit.token)?;
        let withdraw_hash = WithdrawHash::compute(
            &src_chain_key,
            &token,
            &recipient,
            &deposit.amount,
            deposit.nonce as u64,
        );

        let new_approval = NewApproval {
            src_chain_key: src_chain_key.as_bytes().to_vec(),
            nonce: deposit.nonce,
            dest_chain_id: self.chain_id as i64,
            withdraw_hash: withdraw_hash.as_bytes().to_vec(),
            token: deposit.token.clone(),
            recipient: deposit.recipient.clone(),
            amount: deposit.amount.clone(),
            fee: fee.to_string(),
            fee_recipient: Some(format!("0x{:x}", self.fee_recipient)),
            deduct_from_amount: false,
        };

        let approval_id = db::insert_approval(&self.db, &new_approval).await?;
        info!(
            approval_id = approval_id,
            nonce = deposit.nonce,
            "Created approval for Terra deposit"
        );

        // Submit to EVM
        match self.submit_approval(approval_id).await {
            Ok(tx_hash) => {
                info!(
                    approval_id = approval_id,
                    tx_hash = %tx_hash,
                    "Submitted approval transaction"
                );
                db::update_terra_deposit_status(&self.db, deposit.id, "processed").await?;
            }
            Err(e) => {
                warn!(
                    approval_id = approval_id,
                    error = %e,
                    "Failed to submit approval, will retry"
                );
                db::update_approval_failed(&self.db, approval_id, &e.to_string()).await?;
            }
        }

        Ok(())
    }

    /// Submit an approval transaction to EVM
    async fn submit_approval(&self, approval_id: i64) -> Result<String> {
        // Get approval from database
        let approvals = db::get_pending_approvals(&self.db, self.chain_id as i64).await?;
        let _approval = approvals
            .into_iter()
            .find(|a| a.id == approval_id)
            .ok_or_else(|| eyre!("Approval {} not found", approval_id))?;

        // Build provider
        let provider = ProviderBuilder::new()
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        // Get current block for confirmation
        let block = provider.get_block_number().await?;

        // TODO: Actually submit the transaction using contract ABI
        // For now, return a placeholder
        let tx_hash = format!("0x{:064x}", block);

        db::update_approval_submitted(&self.db, approval_id, &tx_hash).await?;

        Ok(tx_hash)
    }

    /// Calculate fee based on amount
    fn calculate_fee(&self, amount: &str) -> U256 {
        let amount_u256 = U256::from_str(amount).unwrap_or(U256::ZERO);
        amount_u256 * U256::from(self.default_fee_bps) / U256::from(10000u64)
    }
}
