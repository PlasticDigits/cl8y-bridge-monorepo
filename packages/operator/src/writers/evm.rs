//! EVM Writer - Submits withdrawal approvals to EVM chains
//!
//! Processes pending Terra deposits and submits corresponding
//! withdrawal approvals to the EVM bridge contract.
//!
//! ## V2 Withdrawal Flow
//!
//! In V2, the withdrawal flow is user-initiated:
//! 1. User calls `withdrawSubmit` on destination chain
//! 2. Operator calls `withdrawApprove(withdrawHash)` to approve
//! 3. After cancel window, anyone can call `withdrawExecuteUnlock/Mint`
//!
//! The operator only needs to approve pending withdrawals, not create them.

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
use crate::contracts::evm_bridge::Bridge;
use crate::db::{self, NewApproval, TerraDeposit};
use crate::hash::{address_to_bytes32, bytes32_to_hex, compute_transfer_hash};
use crate::types::{ChainId, EvmAddress};

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
    /// This chain's registered 4-byte chain ID (V2)
    this_chain_id: ChainId,
    signer: PrivateKeySigner,
    default_fee_bps: u32,
    fee_recipient: Address,
    db: PgPool,
    /// Cancel window in seconds (queried from contract)
    cancel_window: u64,
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

        // V2 configuration - required
        let this_chain_id = ChainId::from_u32(evm_config.this_chain_id.unwrap_or(1));

        info!(
            operator_address = %signer.address(),
            chain_id = evm_config.chain_id,
            this_chain_id = %this_chain_id,
            bridge_address = %bridge_address,
            "EVM writer initialized (V2)"
        );

        // Query cancel window from V2 contract
        let cancel_window = Self::query_cancel_window(&evm_config.rpc_url, bridge_address)
            .await
            .unwrap_or(300);

        info!(delay_seconds = cancel_window, "EVM cancel window");

        Ok(Self {
            rpc_url: evm_config.rpc_url.clone(),
            bridge_address,
            chain_id: evm_config.chain_id,
            this_chain_id,
            signer,
            default_fee_bps: fee_config.default_fee_bps,
            fee_recipient,
            db,
            cancel_window,
            pending_executions: HashMap::new(),
        })
    }

    /// Query the cancel window from the V2 contract
    async fn query_cancel_window(rpc_url: &str, bridge_address: Address) -> Result<u64> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = Bridge::new(bridge_address, provider);
        let window = contract.getCancelWindow().call().await?;

        Ok(window._0.try_into().unwrap_or(300))
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

    /// Process pending executions (after cancel window has elapsed)
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
        // Source chain is Terra Classic
        // Use the 4-byte chain ID from the deposit or derive from the dest_chain_id
        let src_chain_id = if deposit.dest_chain_id == 31337 {
            // Localterra - use a predefined chain ID (e.g., 5)
            ChainId::from_u32(5)
        } else {
            // Columbus-5 mainnet - use a predefined chain ID (e.g., 4)
            ChainId::from_u32(4)
        };

        // Check if approval already exists
        if db::approval_exists(
            &self.db,
            src_chain_id.as_bytes(),
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

        // Get the EVM token address - either from the deposit or try to parse the token field
        let evm_token_str = deposit.evm_token_address.as_ref().unwrap_or(&deposit.token);
        let recipient = EvmAddress::from_hex(&deposit.recipient)?;
        let token = EvmAddress::from_hex(evm_token_str)
            .map_err(|e| eyre!("Invalid EVM token address '{}': {}", evm_token_str, e))?;

        // Token as bytes32
        let mut token_bytes32 = [0u8; 32];
        token_bytes32[12..32].copy_from_slice(&token.0);

        // Source account (Terra sender encoded as universal address)
        let mut src_account = [0u8; 32];
        src_account[0..4].copy_from_slice(&[0, 0, 0, 2]); // Cosmos chain type
        if let Ok((raw, _)) = crate::hash::decode_bech32_address(&deposit.sender) {
            src_account[4..24].copy_from_slice(&raw);
        }

        // Parse amount
        let amount: u128 = deposit
            .amount
            .parse()
            .map_err(|_| eyre!("Invalid amount: {}", deposit.amount))?;

        // Destination account (EVM recipient) encoded as bytes32
        let dest_account = address_to_bytes32(&recipient.0);

        // Compute unified transfer hash using V2 format (7-field)
        let withdraw_hash = compute_transfer_hash(
            src_chain_id.as_bytes(),
            self.this_chain_id.as_bytes(),
            &src_account,
            &dest_account,
            &token_bytes32,
            amount,
            deposit.nonce as u64,
        );

        // Format addresses as standard EVM format (0x + 40 hex chars)
        let token_for_approval = format!("0x{}", hex::encode(token.0));
        let recipient_for_approval = format!("0x{}", hex::encode(recipient.0));

        let new_approval = NewApproval {
            src_chain_key: src_chain_id.as_bytes().to_vec(),
            nonce: deposit.nonce,
            dest_chain_id: self.chain_id as i64,
            withdraw_hash: withdraw_hash.to_vec(),
            token: token_for_approval,
            recipient: recipient_for_approval,
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
        match self
            .submit_approval(deposit, &src_chain_id, &withdraw_hash)
            .await
        {
            Ok(tx_hash) => {
                info!(
                    approval_id = approval_id,
                    tx_hash = %tx_hash,
                    withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                    "Submitted approval transaction"
                );

                // Track for auto-execution
                self.pending_executions.insert(
                    withdraw_hash,
                    PendingExecution {
                        withdraw_hash,
                        approved_at: Instant::now(),
                        delay_seconds: self.cancel_window,
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

    /// Submit an approval transaction to EVM (V2 - user-initiated flow)
    ///
    /// In V2, the user has already submitted the withdrawal request.
    /// The operator just needs to approve it by hash.
    async fn submit_approval(
        &self,
        deposit: &TerraDeposit,
        _src_chain_id: &ChainId,
        withdraw_hash: &[u8; 32],
    ) -> Result<String> {
        // Build provider with signer
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let withdraw_hash_fixed: FixedBytes<32> = FixedBytes::from(*withdraw_hash);

        // Create V2 contract instance
        let contract = Bridge::new(self.bridge_address, &provider);

        debug!(
            withdraw_hash = %bytes32_to_hex(withdraw_hash),
            nonce = deposit.nonce,
            "Submitting withdrawApprove (V2)"
        );

        let call = contract.withdrawApprove(withdraw_hash_fixed);

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

        Ok(format!("0x{:x}", tx_hash))
    }

    /// Submit an ExecuteWithdraw transaction (V2)
    ///
    /// In V2, we call withdrawExecuteUnlock for lock/unlock tokens
    /// or withdrawExecuteMint for mintable tokens.
    /// For now, we default to unlock mode.
    async fn submit_execute_withdraw(&self, withdraw_hash: [u8; 32]) -> Result<String> {
        // Build provider with signer
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = Bridge::new(self.bridge_address, &provider);

        debug!(
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            "Submitting withdraw execution (V2 - unlock mode)"
        );

        // Default to unlock mode
        // TODO: Query token type to determine if we should use mint mode
        let call = contract.withdrawExecuteUnlock(FixedBytes::from(withdraw_hash));

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send withdraw tx: {}", e))?;

        let tx_hash = *pending_tx.tx_hash();
        info!(tx_hash = %tx_hash, "Withdraw transaction sent (V2)");

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
