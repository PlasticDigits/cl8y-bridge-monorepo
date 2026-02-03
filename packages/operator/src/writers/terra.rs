//! Terra Writer - Submits ApproveWithdraw and ExecuteWithdraw transactions
//!
//! Implements the watchtower pattern for incoming transfers to Terra:
//! 1. Watches for EVM deposits destined for Terra
//! 2. Submits ApproveWithdraw to Terra contract (starts delay timer)
//! 3. After delay elapses, submits ExecuteWithdraw to complete the transfer

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use sqlx::PgPool;
use tracing::{debug, error, info, warn};

use crate::config::TerraConfig;
use crate::contracts::terra_bridge::{build_approve_withdraw_msg, build_execute_withdraw_msg};
use crate::db::{self, EvmDeposit, NewRelease};
use crate::hash::{
    bytes32_to_hex, compute_transfer_id, encode_evm_address, evm_chain_key, localterra_chain_key,
    terra_chain_key,
};
use crate::terra_client::TerraClient;
use crate::types::ChainKey;

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

/// Terra transaction writer for submitting approvals and executions
pub struct TerraWriter {
    lcd_url: String,
    chain_id: String,
    contract_address: String,
    client: Client,
    terra_client: TerraClient,
    db: PgPool,
    /// Withdraw delay in seconds (queried from contract)
    withdraw_delay: u64,
    /// Fee recipient for withdrawals
    fee_recipient: String,
    /// Pending approvals awaiting execution
    pending_executions: HashMap<[u8; 32], PendingExecution>,
}

impl TerraWriter {
    /// Create a new Terra writer
    pub async fn new(terra_config: &TerraConfig, db: PgPool) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .wrap_err("Failed to create HTTP client")?;

        // Create Terra client for transaction signing
        let terra_client = TerraClient::new(
            &terra_config.lcd_url,
            &terra_config.chain_id,
            &terra_config.mnemonic,
        )?;

        // Query withdraw delay from contract (default to 60 seconds if query fails)
        let withdraw_delay = Self::query_withdraw_delay(
            &client,
            &terra_config.lcd_url,
            &terra_config.bridge_address,
        )
        .await
        .unwrap_or(60);

        info!(
            delay_seconds = withdraw_delay,
            operator_address = %terra_client.address,
            "Terra writer initialized with withdraw delay"
        );

        Ok(Self {
            lcd_url: terra_config.lcd_url.clone(),
            chain_id: terra_config.chain_id.clone(),
            contract_address: terra_config.bridge_address.clone(),
            client,
            terra_client,
            db,
            withdraw_delay,
            fee_recipient: terra_config.fee_recipient.clone().unwrap_or_default(),
            pending_executions: HashMap::new(),
        })
    }

    /// Query the withdraw delay from the contract
    async fn query_withdraw_delay(client: &Client, lcd_url: &str, contract: &str) -> Result<u64> {
        let query = serde_json::json!({
            "withdraw_delay": {}
        });

        let query_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            serde_json::to_string(&query)?,
        );

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            lcd_url, contract, query_b64
        );

        let response: serde_json::Value = client.get(&url).send().await?.json().await?;

        let delay = response["data"]["delay_seconds"]
            .as_u64()
            .ok_or_else(|| eyre!("Invalid withdraw delay response"))?;

        Ok(delay)
    }

    /// Process pending EVM deposits and create approvals
    pub async fn process_pending(&mut self) -> Result<()> {
        // First, check if any pending executions are ready
        self.process_pending_executions().await?;

        // Then process new deposits
        let deposits = db::get_pending_evm_deposits(&self.db).await?;

        for deposit in deposits {
            if let Err(e) = self.process_deposit(&deposit).await {
                error!(
                    deposit_id = deposit.id,
                    error = %e,
                    "Failed to process EVM deposit"
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
                            "Successfully executed withdrawal"
                        );
                        to_remove.push(*hash);
                    }
                    Err(e) => {
                        warn!(
                            withdraw_hash = %bytes32_to_hex(hash),
                            error = %e,
                            attempt = pending.attempts + 1,
                            "Failed to execute withdrawal, will retry"
                        );
                        // Don't remove - will retry on next cycle
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

    /// Process a single EVM deposit
    async fn process_deposit(&mut self, deposit: &EvmDeposit) -> Result<()> {
        let src_chain_key = ChainKey::evm(deposit.chain_id as u64);

        // Check if release already exists
        if db::release_exists(&self.db, src_chain_key.as_bytes(), deposit.nonce).await? {
            db::update_evm_deposit_status(&self.db, deposit.id, "processed").await?;
            return Ok(());
        }

        // Decode destination account from bytes
        let recipient = self.decode_terra_address(&deposit.dest_account)?;

        // Create release record
        let new_release = NewRelease {
            src_chain_key: src_chain_key.as_bytes().to_vec(),
            nonce: deposit.nonce,
            sender: format!("0x{}", hex::encode(&deposit.token)),
            recipient: recipient.clone(),
            token: deposit.token.clone(),
            amount: deposit.amount.clone(),
            source_chain_id: deposit.chain_id,
        };

        let release_id = db::insert_release(&self.db, &new_release).await?;
        info!(
            release_id = release_id,
            nonce = deposit.nonce,
            "Created release for EVM deposit"
        );

        // Submit ApproveWithdraw to Terra
        match self.submit_approve_withdraw(deposit, &recipient).await {
            Ok((tx_hash, withdraw_hash)) => {
                info!(
                    release_id = release_id,
                    tx_hash = %tx_hash,
                    withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                    "Submitted ApproveWithdraw transaction"
                );

                // Track for auto-execution
                self.pending_executions.insert(
                    withdraw_hash,
                    PendingExecution {
                        withdraw_hash,
                        approved_at: Instant::now(),
                        delay_seconds: self.withdraw_delay,
                        attempts: 0,
                    },
                );

                db::update_evm_deposit_status(&self.db, deposit.id, "approved").await?;
                db::update_release_submitted(&self.db, release_id, &tx_hash).await?;
            }
            Err(e) => {
                warn!(
                    release_id = release_id,
                    error = %e,
                    "Failed to submit ApproveWithdraw, will retry"
                );
                db::update_release_failed(&self.db, release_id, &e.to_string()).await?;
            }
        }

        Ok(())
    }

    /// Submit an ApproveWithdraw transaction to Terra
    async fn submit_approve_withdraw(
        &self,
        deposit: &EvmDeposit,
        recipient: &str,
    ) -> Result<(String, [u8; 32])> {
        // Compute chain keys
        let src_chain_key = evm_chain_key(deposit.chain_id as u64);
        let dest_chain_key = if self.chain_id == "localterra" {
            localterra_chain_key()
        } else {
            terra_chain_key()
        };

        // Encode destination account
        let dest_account = encode_evm_address(&format!("0x{}", hex::encode(&deposit.dest_account)))
            .map_err(|e| eyre!("Failed to encode dest account: {}", e))?;

        // Parse amount
        let amount: u128 = deposit
            .amount
            .parse()
            .map_err(|e| eyre!("Failed to parse amount: {}", e))?;

        // Encode token address (for hash computation)
        let dest_token_address = if deposit.token.starts_with("0x") || deposit.token.len() == 40 {
            encode_evm_address(&deposit.token)
                .map_err(|e| eyre!("Failed to encode token: {}", e))?
        } else {
            // Native denom - use keccak256 of the denom string
            crate::hash::keccak256(deposit.token.as_bytes())
        };

        // Compute withdraw hash
        let withdraw_hash = compute_transfer_id(
            &src_chain_key,
            &dest_chain_key,
            &dest_token_address,
            &dest_account,
            amount,
            deposit.nonce as u64,
        );

        // Build the message
        let msg = build_approve_withdraw_msg(
            src_chain_key,
            &deposit.token,
            recipient,
            dest_account,
            amount,
            deposit.nonce as u64,
            0,                   // No fee for now
            &self.fee_recipient, // Fee recipient
            false,               // Don't deduct from amount
        );

        // Serialize to JSON for logging
        let msg_json = serde_json::to_string(&msg)?;
        debug!(msg = %msg_json, "ApproveWithdraw message");

        // Sign and broadcast the transaction
        let tx_hash = self
            .terra_client
            .execute_contract(&self.contract_address, &msg, vec![])
            .await
            .map_err(|e| eyre!("Failed to execute ApproveWithdraw: {}", e))?;

        Ok((tx_hash, withdraw_hash))
    }

    /// Submit an ExecuteWithdraw transaction to Terra
    async fn submit_execute_withdraw(&self, withdraw_hash: [u8; 32]) -> Result<String> {
        let msg = build_execute_withdraw_msg(withdraw_hash);

        // Serialize to JSON for logging
        let msg_json = serde_json::to_string(&msg)?;
        debug!(msg = %msg_json, "ExecuteWithdraw message");

        // Sign and broadcast the transaction
        let tx_hash = self
            .terra_client
            .execute_contract(&self.contract_address, &msg, vec![])
            .await
            .map_err(|e| eyre!("Failed to execute ExecuteWithdraw: {}", e))?;

        Ok(tx_hash)
    }

    /// Decode Terra address from bytes
    fn decode_terra_address(&self, bytes: &[u8]) -> Result<String> {
        // Try to decode as UTF-8 string first
        if let Ok(s) = String::from_utf8(bytes.to_vec()) {
            let s = s.trim_end_matches('\0');
            if s.starts_with("terra") {
                return Ok(s.to_string());
            }
        }

        // Otherwise, try to decode as bech32
        // For now, just encode as hex for debugging
        Err(eyre!(
            "Unable to decode Terra address from bytes: {}",
            hex::encode(bytes)
        ))
    }

    /// Get count of pending executions
    pub fn pending_execution_count(&self) -> usize {
        self.pending_executions.len()
    }

    /// Get the operator's Terra address
    pub fn operator_address(&self) -> String {
        self.terra_client.address.to_string()
    }
}
