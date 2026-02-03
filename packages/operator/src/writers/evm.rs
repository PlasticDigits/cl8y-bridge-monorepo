//! EVM Writer - Submits withdrawal approvals to EVM chains
//!
//! Processes pending Terra deposits and submits corresponding
//! approveWithdraw transactions to the EVM bridge contract.

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use alloy::network::EthereumWallet;
use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{debug, error, info, warn};

use crate::config::{EvmConfig, FeeConfig};
use crate::contracts::evm_bridge::CL8YBridge;
use crate::db::{self, NewApproval, TerraDeposit};
use crate::hash::{bytes32_to_hex, compute_transfer_id};
use crate::types::{ChainKey, EvmAddress, WithdrawHash};

/// Pending approval tracking for auto-execution
#[derive(Debug, Clone)]
struct PendingExecution {
    /// The withdraw hash
    withdraw_hash: [u8; 32],
    /// When the approval was submitted
    approved_at: Instant,
    /// The delay required before execution
    delay_seconds: u64,
    /// Number of execution attempts
    attempts: u32,
}

/// EVM transaction writer for submitting withdrawal approvals
pub struct EvmWriter {
    rpc_url: String,
    bridge_address: Address,
    chain_id: u64,
    signer: PrivateKeySigner,
    default_fee_bps: u32,
    fee_recipient: Address,
    db: PgPool,
    /// Withdraw delay in seconds (queried from contract)
    withdraw_delay: u64,
    /// Pending approvals awaiting execution
    pending_executions: HashMap<[u8; 32], PendingExecution>,
}

impl EvmWriter {
    /// Create a new EVM writer
    pub async fn new(evm_config: &EvmConfig, fee_config: &FeeConfig, db: PgPool) -> Result<Self> {
        let bridge_address =
            Address::from_str(&evm_config.bridge_address).wrap_err("Invalid bridge address")?;
        let fee_recipient =
            Address::from_str(&fee_config.fee_recipient).wrap_err("Invalid fee recipient")?;

        // Parse the private key
        let signer: PrivateKeySigner = evm_config
            .private_key
            .parse()
            .wrap_err("Invalid private key")?;

        info!(
            operator_address = %signer.address(),
            chain_id = evm_config.chain_id,
            bridge_address = %bridge_address,
            "EVM writer initialized"
        );

        // Query withdraw delay from contract
        let withdraw_delay = Self::query_withdraw_delay(&evm_config.rpc_url, bridge_address)
            .await
            .unwrap_or(300); // Default 5 minutes

        info!(delay_seconds = withdraw_delay, "EVM withdraw delay");

        Ok(Self {
            rpc_url: evm_config.rpc_url.clone(),
            bridge_address,
            chain_id: evm_config.chain_id,
            signer,
            default_fee_bps: fee_config.default_fee_bps,
            fee_recipient,
            db,
            withdraw_delay,
            pending_executions: HashMap::new(),
        })
    }

    /// Query the withdraw delay from the contract
    async fn query_withdraw_delay(rpc_url: &str, bridge_address: Address) -> Result<u64> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = CL8YBridge::new(bridge_address, provider);
        let delay = contract.withdrawDelay().call().await?;

        Ok(delay._0.try_into().unwrap_or(300))
    }

    /// Process pending Terra deposits and create approvals
    pub async fn process_pending(&mut self) -> Result<()> {
        // First, check if any pending executions are ready
        self.process_pending_executions().await?;

        // Then process new deposits
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

    /// Process pending executions (after delay has elapsed)
    async fn process_pending_executions(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (hash, pending) in &self.pending_executions {
            let elapsed = now.duration_since(pending.approved_at);

            if elapsed.as_secs() >= pending.delay_seconds {
                // Delay has elapsed, try to execute
                match self.submit_execute_withdraw(*hash).await {
                    Ok(tx_hash) => {
                        info!(
                            withdraw_hash = %bytes32_to_hex(hash),
                            tx_hash = %tx_hash,
                            "Successfully executed EVM withdrawal"
                        );
                        to_remove.push(*hash);
                    }
                    Err(e) => {
                        warn!(
                            withdraw_hash = %bytes32_to_hex(hash),
                            error = %e,
                            attempt = pending.attempts + 1,
                            "Failed to execute EVM withdrawal, will retry"
                        );
                    }
                }
            }
        }

        // Remove successfully executed
        for hash in to_remove {
            self.pending_executions.remove(&hash);
        }

        Ok(())
    }

    /// Process a single Terra deposit
    async fn process_deposit(&mut self, deposit: &TerraDeposit) -> Result<()> {
        // Source chain is always Terra Classic (columbus-5 for mainnet, localterra for local)
        // We use a simple heuristic: if dest_chain_id is 31337 (Anvil), assume localterra
        let src_chain_key = if deposit.dest_chain_id == 31337 {
            ChainKey::cosmos("localterra", "terra")
        } else {
            ChainKey::cosmos("columbus-5", "terra")
        };

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
        match self.submit_approval(deposit, &src_chain_key).await {
            Ok((tx_hash, wh)) => {
                info!(
                    approval_id = approval_id,
                    tx_hash = %tx_hash,
                    withdraw_hash = %bytes32_to_hex(&wh),
                    "Submitted approval transaction"
                );

                // Track for auto-execution
                self.pending_executions.insert(
                    wh,
                    PendingExecution {
                        withdraw_hash: wh,
                        approved_at: Instant::now(),
                        delay_seconds: self.withdraw_delay,
                        attempts: 0,
                    },
                );

                db::update_terra_deposit_status(&self.db, deposit.id, "approved").await?;
                db::update_approval_submitted(&self.db, approval_id, &tx_hash).await?;
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
    async fn submit_approval(
        &self,
        deposit: &TerraDeposit,
        src_chain_key: &ChainKey,
    ) -> Result<(String, [u8; 32])> {
        // Build provider with signer
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        // Parse addresses
        let token: Address = deposit
            .token
            .parse()
            .map_err(|_| eyre!("Invalid token address: {}", deposit.token))?;
        let to: Address = deposit
            .recipient
            .parse()
            .map_err(|_| eyre!("Invalid recipient address: {}", deposit.recipient))?;

        // Parse amount
        let amount: U256 = U256::from_str(&deposit.amount)
            .map_err(|_| eyre!("Invalid amount: {}", deposit.amount))?;

        // Calculate fee
        let fee = self.calculate_fee(&deposit.amount);

        // Convert src_chain_key to bytes32
        let src_chain_key_bytes: FixedBytes<32> = FixedBytes::from_slice(src_chain_key.as_bytes());

        // Compute destAccount as bytes32 (recipient address padded to 32 bytes)
        let dest_account: FixedBytes<32> = {
            let mut bytes = [0u8; 32];
            bytes[12..32].copy_from_slice(to.as_slice());
            FixedBytes::from(bytes)
        };

        // Create contract instance
        let contract = CL8YBridge::new(self.bridge_address, &provider);

        // Build and send transaction
        debug!(
            src_chain_key = %bytes32_to_hex(src_chain_key.as_bytes()),
            token = %token,
            to = %to,
            amount = %amount,
            nonce = deposit.nonce,
            fee = %fee,
            "Submitting approveWithdraw"
        );

        let call = contract.approveWithdraw(
            src_chain_key_bytes,
            token,
            to,
            dest_account,
            amount,
            U256::from(deposit.nonce),
            fee,
            self.fee_recipient,
            false, // deductFromAmount
        );

        // Send transaction
        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send transaction: {}", e))?;

        let tx_hash = *pending_tx.tx_hash();
        info!(tx_hash = %tx_hash, "Transaction sent, waiting for confirmation");

        // Wait for confirmation
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("Transaction reverted"));
        }

        // Compute the withdraw hash for tracking
        let evm_chain_key = crate::hash::evm_chain_key(self.chain_id);
        let dest_token_bytes = {
            let mut bytes = [0u8; 32];
            bytes[12..32].copy_from_slice(token.as_slice());
            bytes
        };
        let dest_account_bytes = {
            let mut bytes = [0u8; 32];
            bytes[12..32].copy_from_slice(to.as_slice());
            bytes
        };

        let withdraw_hash = compute_transfer_id(
            src_chain_key.as_bytes(),
            &evm_chain_key,
            &dest_token_bytes,
            &dest_account_bytes,
            amount.try_into().unwrap_or(0),
            deposit.nonce as u64,
        );

        Ok((format!("0x{:x}", tx_hash), withdraw_hash))
    }

    /// Submit an ExecuteWithdraw transaction
    async fn submit_execute_withdraw(&self, withdraw_hash: [u8; 32]) -> Result<String> {
        // Build provider with signer
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = CL8YBridge::new(self.bridge_address, &provider);

        // Get the approval to check fee
        let approval = contract
            .getWithdrawApproval(FixedBytes::from(withdraw_hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get approval: {}", e))?;

        let fee_value = if approval.deductFromAmount {
            U256::ZERO
        } else {
            approval.fee
        };

        debug!(
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            fee = %fee_value,
            "Submitting withdraw execution"
        );

        let call = contract
            .withdraw(FixedBytes::from(withdraw_hash))
            .value(fee_value);

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send withdraw tx: {}", e))?;

        let tx_hash = *pending_tx.tx_hash();
        info!(tx_hash = %tx_hash, "Withdraw transaction sent");

        // Wait for confirmation
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("Withdraw transaction reverted"));
        }

        Ok(format!("0x{:x}", tx_hash))
    }

    /// Calculate fee based on amount
    fn calculate_fee(&self, amount: &str) -> U256 {
        let amount_u256 = U256::from_str(amount).unwrap_or(U256::ZERO);
        amount_u256 * U256::from(self.default_fee_bps) / U256::from(10000u64)
    }

    /// Get the operator's address
    pub fn operator_address(&self) -> Address {
        self.signer.address()
    }

    /// Get count of pending executions
    pub fn pending_execution_count(&self) -> usize {
        self.pending_executions.len()
    }
}
