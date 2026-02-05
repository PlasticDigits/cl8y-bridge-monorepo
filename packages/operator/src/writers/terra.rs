//! Terra Writer - Submits Withdrawal Approvals and Executions
//!
//! Implements the watchtower pattern for incoming transfers to Terra.
//!
//! ## V2 Flow (User-initiated)
//! 1. User calls WithdrawSubmit on Terra (pays gas)
//! 2. Operator calls WithdrawApprove with just the hash
//! 3. Cancelers can cancel during the cancel window
//! 4. Anyone can call WithdrawExecuteUnlock/Mint after window

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use sqlx::PgPool;
use tracing::{debug, error, info, warn};

use crate::config::TerraConfig;
use crate::contracts::terra_bridge::{
    build_withdraw_approve_msg_v2, build_withdraw_execute_unlock_msg_v2,
};
use crate::db::{self, EvmDeposit, NewRelease};
use crate::hash::{address_to_bytes32, bytes32_to_hex, compute_withdraw_hash, parse_evm_address};
use crate::terra_client::TerraClient;
use crate::types::ChainId;

/// Pending approval tracking for auto-execution
#[allow(dead_code)]
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
    #[allow(dead_code)]
    lcd_url: String,
    chain_id: String,
    contract_address: String,
    #[allow(dead_code)]
    client: Client,
    terra_client: TerraClient,
    db: PgPool,
    /// Cancel window in seconds
    cancel_window: u64,
    /// Fee recipient for withdrawals
    fee_recipient: String,
    /// Pending approvals awaiting execution
    pending_executions: HashMap<[u8; 32], PendingExecution>,
    /// This chain's 4-byte chain ID (V2)
    this_chain_id: ChainId,
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

        // Get this chain's ID (V2)
        // Default chain IDs: 4 = terraclassic_columbus-5, 5 = terraclassic_localterra
        let this_chain_id = if let Some(id) = terra_config.this_chain_id {
            ChainId::from_u32(id)
        } else {
            // Try to query from contract
            match Self::query_this_chain_id(
                &client,
                &terra_config.lcd_url,
                &terra_config.bridge_address,
            )
            .await
            {
                Ok(id) => id,
                Err(e) => {
                    warn!(error = %e, "Failed to query chain ID from contract, using default");
                    // Default based on chain_id string
                    if terra_config.chain_id == "localterra" {
                        ChainId::from_u32(5) // terraclassic_localterra
                    } else {
                        ChainId::from_u32(4) // terraclassic_columbus-5
                    }
                }
            }
        };

        // Query cancel window from contract
        let cancel_window =
            Self::query_cancel_window(&client, &terra_config.lcd_url, &terra_config.bridge_address)
                .await
                .unwrap_or(60);

        info!(
            delay_seconds = cancel_window,
            operator_address = %terra_client.address,
            this_chain_id = %this_chain_id.to_hex(),
            "Terra writer initialized (V2)"
        );

        Ok(Self {
            lcd_url: terra_config.lcd_url.clone(),
            chain_id: terra_config.chain_id.clone(),
            contract_address: terra_config.bridge_address.clone(),
            client,
            terra_client,
            db,
            cancel_window,
            fee_recipient: terra_config.fee_recipient.clone().unwrap_or_default(),
            pending_executions: HashMap::new(),
            this_chain_id,
        })
    }

    /// Query the cancel window from the contract (V2)
    async fn query_cancel_window(client: &Client, lcd_url: &str, contract: &str) -> Result<u64> {
        let query = serde_json::json!({
            "cancel_window": {}
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

        // Try V2 field name first, fall back to V1 field name
        let delay = response["data"]["cancel_window_seconds"]
            .as_u64()
            .or_else(|| response["data"]["delay_seconds"].as_u64())
            .ok_or_else(|| eyre!("Invalid cancel window response"))?;

        Ok(delay)
    }

    /// Query this chain's ID from the contract (V2)
    async fn query_this_chain_id(
        client: &Client,
        lcd_url: &str,
        contract: &str,
    ) -> Result<ChainId> {
        let query = serde_json::json!({
            "this_chain_id": {}
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

        // Chain ID is returned as base64-encoded 4 bytes
        let chain_id_b64 = response["data"]["chain_id"]
            .as_str()
            .ok_or_else(|| eyre!("Invalid chain ID response"))?;

        let chain_id_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, chain_id_b64)?;

        if chain_id_bytes.len() != 4 {
            return Err(eyre!(
                "Invalid chain ID length: expected 4 bytes, got {}",
                chain_id_bytes.len()
            ));
        }

        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&chain_id_bytes);
        Ok(ChainId::from_bytes(bytes))
    }

    /// Process pending EVM deposits and create approvals
    pub async fn process_pending(&mut self) -> Result<()> {
        // First, check if any pending executions are ready
        self.process_pending_executions().await?;

        // Then process new deposits (only those destined for Cosmos/Terra)
        let deposits = db::get_pending_evm_deposits_for_cosmos(&self.db).await?;

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
        // Source chain ID (4-byte V2 format)
        let src_chain_id = ChainId::from_u32(deposit.chain_id as u32);

        // Check if release already exists
        if db::release_exists(&self.db, src_chain_id.as_bytes(), deposit.nonce).await? {
            db::update_evm_deposit_status(&self.db, deposit.id, "processed").await?;
            return Ok(());
        }

        // Decode destination account from bytes
        let recipient = self.decode_terra_address(&deposit.dest_account)?;

        // Create release record
        let new_release = NewRelease {
            src_chain_key: src_chain_id.as_bytes().to_vec(),
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

        // Submit WithdrawApprove to Terra
        match self
            .submit_approve_withdraw(deposit, &recipient, &src_chain_id)
            .await
        {
            Ok((tx_hash, withdraw_hash)) => {
                info!(
                    release_id = release_id,
                    tx_hash = %tx_hash,
                    withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                    "Submitted WithdrawApprove transaction"
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

                db::update_evm_deposit_status(&self.db, deposit.id, "approved").await?;
                db::update_release_submitted(&self.db, release_id, &tx_hash).await?;
            }
            Err(e) => {
                warn!(
                    release_id = release_id,
                    error = %e,
                    "Failed to submit WithdrawApprove, will retry"
                );
                db::update_release_failed(&self.db, release_id, &e.to_string()).await?;
            }
        }

        Ok(())
    }

    /// Submit a WithdrawApprove transaction to Terra (V2)
    ///
    /// In V2, the user already called WithdrawSubmit. Operator just approves the hash.
    async fn submit_approve_withdraw(
        &self,
        deposit: &EvmDeposit,
        _recipient: &str,
        src_chain_id: &ChainId,
    ) -> Result<(String, [u8; 32])> {
        // Get destination account as 32 bytes (universal address format)
        let dest_account: [u8; 32] = deposit.dest_account.clone().try_into().map_err(|_| {
            eyre!(
                "Invalid dest_account length: expected 32 bytes, got {}",
                deposit.dest_account.len()
            )
        })?;

        // Parse amount
        let amount: u128 = deposit
            .amount
            .parse()
            .map_err(|e| eyre!("Failed to parse amount: {}", e))?;

        // Encode token address as bytes32 for V2 hash computation
        let dest_token: [u8; 32] = if deposit.token.starts_with("0x") || deposit.token.len() == 40 {
            // Parse EVM address string to bytes20, then pad to bytes32
            let raw_addr = parse_evm_address(&deposit.token)
                .map_err(|e| eyre!("Failed to parse token address: {}", e))?;
            address_to_bytes32(&raw_addr)
        } else {
            // Native denom - use keccak256 of the denom string
            crate::hash::keccak256(deposit.token.as_bytes())
        };

        // Compute withdraw hash using V2 algorithm (abi.encodePacked with 4-byte chain IDs)
        let withdraw_hash = compute_withdraw_hash(
            src_chain_id.as_bytes(),
            self.this_chain_id.as_bytes(),
            &dest_token,
            &dest_account,
            amount,
            deposit.nonce as u64,
        );

        debug!(
            src_chain_id = %src_chain_id.to_hex(),
            dest_chain_id = %self.this_chain_id.to_hex(),
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            "Computed V2 withdraw hash"
        );

        // Build the message (V2 - just the hash)
        let msg = build_withdraw_approve_msg_v2(withdraw_hash);

        // Serialize to JSON for logging
        let msg_json = serde_json::to_string(&msg)?;
        debug!(msg = %msg_json, "WithdrawApprove message (V2)");

        // Sign and broadcast the transaction
        let tx_hash = self
            .terra_client
            .execute_contract(&self.contract_address, &msg, vec![])
            .await
            .map_err(|e| eyre!("Failed to execute WithdrawApprove: {}", e))?;

        Ok((tx_hash, withdraw_hash))
    }

    /// Submit a WithdrawExecuteUnlock transaction to Terra (V2)
    ///
    /// For lock/unlock tokens. Use WithdrawExecuteMint for mintable tokens.
    async fn submit_execute_withdraw(&self, withdraw_hash: [u8; 32]) -> Result<String> {
        // Default to unlock mode
        // TODO: Query token type to determine if we should use mint mode
        let msg = build_withdraw_execute_unlock_msg_v2(withdraw_hash);

        // Serialize to JSON for logging
        let msg_json = serde_json::to_string(&msg)?;
        debug!(msg = %msg_json, "WithdrawExecuteUnlock message (V2)");

        // Sign and broadcast the transaction
        let tx_hash = self
            .terra_client
            .execute_contract(&self.contract_address, &msg, vec![])
            .await
            .map_err(|e| eyre!("Failed to execute WithdrawExecuteUnlock: {}", e))?;

        Ok(tx_hash)
    }

    /// Decode Terra address from bytes32
    ///
    /// Supports three formats:
    /// 1. V2 Universal Address: [chain_type(4) | raw_address(20) | reserved(8)]
    ///    - Chain type 0x00000002 = Cosmos/Terra
    /// 2. Raw 20-byte address (left-padded with zeros in bytes32) - decoded to bech32
    /// 3. Legacy ASCII format (UTF-8 bytes of "terra1..." string) - used as-is
    fn decode_terra_address(&self, bytes: &[u8]) -> Result<String> {
        if bytes.len() != 32 {
            return Err(eyre!(
                "Invalid address length: expected 32 bytes, got {}",
                bytes.len()
            ));
        }

        // Check for V2 universal address format
        // Chain type is in first 4 bytes (big-endian)
        let chain_type = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        if chain_type == 2 {
            // V2 Cosmos/Terra address format
            // Raw address is in bytes 4-23
            let mut raw_address = [0u8; 20];
            raw_address.copy_from_slice(&bytes[4..24]);

            // Encode as bech32 Terra address
            match crate::hash::encode_bech32_address(&raw_address, "terra") {
                Ok(addr) => {
                    tracing::debug!(
                        chain_type = chain_type,
                        raw_hex = hex::encode(&raw_address),
                        bech32 = %addr,
                        "Decoded V2 universal address to Terra address"
                    );
                    return Ok(addr);
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        raw_hex = hex::encode(bytes),
                        "Failed to decode V2 address, trying other formats"
                    );
                }
            }
        }

        // Try raw 20-byte address (left-padded with zeros in bytes32)
        // Check if first 12 bytes are zeros (indicating left-padded raw address)
        if bytes[..12].iter().all(|&b| b == 0) {
            // Extract raw 20 bytes and encode as bech32
            match crate::hash::decode_bytes32_to_terra_address(bytes) {
                Ok(addr) => {
                    tracing::debug!(
                        raw_hex = hex::encode(&bytes[12..32]),
                        bech32 = %addr,
                        "Decoded raw bytes to Terra address"
                    );
                    return Ok(addr);
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        raw_hex = hex::encode(bytes),
                        "Failed to decode as raw address, trying legacy format"
                    );
                }
            }
        }

        // Try to decode as UTF-8 string (legacy format)
        if let Ok(s) = String::from_utf8(bytes.to_vec()) {
            let s = s.trim_end_matches('\0');
            if s.starts_with("terra") && s.len() == 44 {
                tracing::debug!(
                    address = %s,
                    "Decoded legacy ASCII Terra address"
                );
                return Ok(s.to_string());
            }
        }

        Err(eyre!(
            "Unable to decode Terra address from bytes: {} (len={}). \
             Expected V2 universal address, raw 20-byte address, or ASCII bech32 string.",
            hex::encode(bytes),
            bytes.len()
        ))
    }

    /// Get count of pending executions
    #[allow(dead_code)]
    pub fn pending_execution_count(&self) -> usize {
        self.pending_executions.len()
    }

    /// Get the operator's Terra address
    #[allow(dead_code)]
    pub fn operator_address(&self) -> String {
        self.terra_client.address.to_string()
    }
}
