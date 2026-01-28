#![allow(dead_code)]

use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use alloy::network::EthereumWallet;
use eyre::{Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{info, error, warn};

use crate::config::{EvmConfig, FeeConfig};
use crate::contracts::CL8YBridge;
use crate::db::{self, TerraDeposit, NewApproval};
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
    pub async fn new(
        evm_config: &EvmConfig,
        fee_config: &FeeConfig,
        db: PgPool,
    ) -> Result<Self> {
        let bridge_address = Address::from_str(&evm_config.bridge_address)
            .wrap_err("Invalid bridge address")?;
        let fee_recipient = Address::from_str(&fee_config.fee_recipient)
            .wrap_err("Invalid fee recipient")?;

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

    /// Calculate fee based on amount and default fee bps
    fn calculate_fee(&self, amount_str: &str) -> String {
        if let Ok(amount) = amount_str.parse::<u128>() {
            let fee = amount * self.default_fee_bps as u128 / 10000;
            return fee.to_string();
        }
        "0".to_string()
    }

    /// Determine if fee should be deducted from amount (native path)
    fn should_deduct_from_amount(&self, _token: &str) -> bool {
        // For ERC20 path, user pays fee separately
        // For native path, deduct from amount
        false
    }

    /// Process all pending Terra deposits
    pub async fn process_pending(&self) -> Result<()> {
        let deposits = db::get_pending_terra_deposits(&self.db).await?;
        
        if !deposits.is_empty() {
            info!(count = deposits.len(), "Processing pending Terra deposits");
        }

        for deposit in deposits {
            if let Err(e) = self.process_deposit(&deposit).await {
                error!(error = %e, nonce = deposit.nonce, tx_hash = %deposit.tx_hash, "Failed to process deposit");
            }
        }

        Ok(())
    }

    /// Process a single Terra deposit - create and submit approval
    async fn process_deposit(&self, deposit: &TerraDeposit) -> Result<()> {
        info!(
            nonce = deposit.nonce,
            sender = %deposit.sender,
            recipient = %deposit.recipient,
            amount = %deposit.amount,
            "Processing Terra deposit for EVM approval"
        );

        // Build the approval record
        let approval = self.build_approval(deposit)?;

        // Check if approval already exists
        let exists = db::approval_exists(
            &self.db,
            &approval.src_chain_key,
            approval.nonce,
            approval.dest_chain_id,
        ).await?;

        if exists {
            warn!(nonce = deposit.nonce, "Approval already exists, skipping");
            return Ok(());
        }

        // Insert the approval record
        let approval_id = db::insert_approval(&self.db, &approval).await?;
        info!(approval_id = approval_id, "Created approval record");

        // Submit the transaction
        match self.submit_approval(&approval).await {
            Ok(tx_hash) => {
                info!(tx_hash = %tx_hash, nonce = deposit.nonce, "Approval submitted successfully");
                db::update_approval_submitted(&self.db, approval_id, &tx_hash).await?;
                db::update_terra_deposit_status(&self.db, deposit.id, "submitted").await?;
            }
            Err(e) => {
                error!(error = %e, nonce = deposit.nonce, "Failed to submit approval");
                db::update_approval_failed(&self.db, approval_id, &e.to_string()).await?;
                return Err(e);
            }
        }

        Ok(())
    }

    /// Build an approval record from a Terra deposit
    fn build_approval(&self, deposit: &TerraDeposit) -> Result<NewApproval> {
        // Source chain key for Terra Classic
        // Using config chain_id would be better, but for now use convention
        let src_chain_key = ChainKey::cosmos("rebel-2", "terra");

        // Parse recipient as EVM address
        let recipient = EvmAddress::from_hex(&deposit.recipient)
            .wrap_err("Invalid recipient address")?;

        // Parse token as EVM address
        let token = EvmAddress::from_hex(&deposit.token)
            .wrap_err("Invalid token address")?;

        // Calculate fee
        let fee = self.calculate_fee(&deposit.amount);
        let deduct_from_amount = self.should_deduct_from_amount(&deposit.token);

        // Compute withdraw hash
        let withdraw_hash = WithdrawHash::compute(
            &src_chain_key,
            &token,
            &recipient,
            &deposit.amount,
            deposit.nonce as u64,
        );

        Ok(NewApproval {
            src_chain_key: src_chain_key.0.to_vec(),
            nonce: deposit.nonce,
            dest_chain_id: deposit.dest_chain_id,
            withdraw_hash: withdraw_hash.0.to_vec(),
            token: deposit.token.clone(),
            recipient: deposit.recipient.clone(),
            amount: deposit.amount.clone(),
            fee,
            fee_recipient: Some(format!("{:?}", self.fee_recipient)),
            deduct_from_amount,
        })
    }

    /// Submit an approval transaction to the EVM bridge
    async fn submit_approval(&self, approval: &NewApproval) -> Result<String> {
        // Parse private key
        let signer: PrivateKeySigner = self.private_key.parse()
            .wrap_err("Failed to parse private key")?;
        
        // Create wallet
        let wallet = EthereumWallet::from(signer);

        // Create provider with wallet
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        // Create contract instance
        let contract = CL8YBridge::new(self.bridge_address, &provider);

        // Prepare parameters
        let src_chain_key: FixedBytes<32> = FixedBytes::from_slice(&approval.src_chain_key);
        let token = Address::from_str(&approval.token)
            .wrap_err("Invalid token address")?;
        let to = Address::from_str(&approval.recipient)
            .wrap_err("Invalid recipient address")?;
        let amount = U256::from_str(&approval.amount)
            .wrap_err("Invalid amount")?;
        let nonce = U256::from(approval.nonce as u64);
        let fee = U256::from_str(&approval.fee)
            .wrap_err("Invalid fee")?;
        let fee_recipient = approval.fee_recipient
            .as_ref()
            .map(|s| Address::from_str(s))
            .transpose()
            .wrap_err("Invalid fee recipient")?
            .unwrap_or(self.fee_recipient);

        // Call approveWithdraw
        let call = contract.approveWithdraw(
            src_chain_key,
            token,
            to,
            amount,
            nonce,
            fee,
            fee_recipient,
            approval.deduct_from_amount,
        );

        // Send transaction
        let pending_tx = call.send().await
            .wrap_err("Failed to send approval transaction")?;

        info!(tx_hash = ?pending_tx.tx_hash(), "Transaction sent, waiting for confirmation");

        // Wait for receipt
        let receipt = pending_tx.get_receipt().await
            .wrap_err("Failed to get transaction receipt")?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        
        if receipt.status() {
            info!(tx_hash = %tx_hash, "Transaction confirmed successfully");
        } else {
            return Err(eyre::eyre!("Transaction reverted: {}", tx_hash));
        }

        Ok(tx_hash)
    }
}
