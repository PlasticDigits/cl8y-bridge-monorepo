//! Watcher for monitoring approvals across chains and submitting cancellations

#![allow(dead_code)]

use std::collections::HashSet;
use std::time::Duration;

use alloy::primitives::{Address, FixedBytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use base64::Engine as _;
use eyre::{eyre, Result, WrapErr};
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::evm_client::EvmClient;
use crate::hash::bytes32_to_hex;
use crate::terra_client::TerraClient;
use crate::verifier::{ApprovalVerifier, PendingApproval, VerificationResult};

// EVM bridge contract ABI for event queries
sol! {
    #[sol(rpc)]
    contract CL8YBridge {
        event WithdrawApproved(
            bytes32 indexed withdrawHash,
            bytes32 indexed srcChainKey,
            address indexed token,
            address to,
            uint256 amount,
            uint256 nonce,
            uint256 fee,
            address feeRecipient,
            bool deductFromAmount
        );

        function getWithdrawApproval(bytes32 withdrawHash) external view returns (
            uint256 fee,
            address feeRecipient,
            uint64 approvedAt,
            bool isApproved,
            bool deductFromAmount,
            bool cancelled,
            bool executed
        );
    }
}

/// Main watcher that monitors all chains for approvals
pub struct CancelerWatcher {
    config: Config,
    verifier: ApprovalVerifier,
    evm_client: EvmClient,
    terra_client: TerraClient,
    /// Hashes we've already verified
    verified_hashes: HashSet<[u8; 32]>,
    /// Hashes we've cancelled
    cancelled_hashes: HashSet<[u8; 32]>,
    /// Last polled EVM block
    last_evm_block: u64,
    /// Last polled Terra height
    last_terra_height: u64,
}

impl CancelerWatcher {
    pub async fn new(config: &Config) -> Result<Self> {
        let verifier = ApprovalVerifier::new(
            &config.evm_rpc_url,
            &config.evm_bridge_address,
            config.evm_chain_id,
            &config.terra_lcd_url,
            &config.terra_bridge_address,
            &config.terra_chain_id,
        );

        let evm_client = EvmClient::new(
            &config.evm_rpc_url,
            &config.evm_bridge_address,
            &config.evm_private_key,
        )?;

        let terra_client = TerraClient::new(
            &config.terra_lcd_url,
            &config.terra_chain_id,
            &config.terra_bridge_address,
            &config.terra_mnemonic,
        )?;

        info!(
            evm_canceler = %evm_client.address(),
            terra_canceler = %terra_client.address,
            "Canceler watcher initialized"
        );

        Ok(Self {
            config: config.clone(),
            verifier,
            evm_client,
            terra_client,
            verified_hashes: HashSet::new(),
            cancelled_hashes: HashSet::new(),
            last_evm_block: 0,
            last_terra_height: 0,
        })
    }

    /// Main run loop
    pub async fn run(&mut self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        info!("Canceler watcher starting...");

        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
                _ = tokio::time::sleep(poll_interval) => {
                    if let Err(e) = self.poll_approvals().await {
                        error!(error = %e, "Error polling approvals");
                    }
                }
            }
        }

        Ok(())
    }

    /// Poll for new approvals on all chains
    async fn poll_approvals(&mut self) -> Result<()> {
        debug!("Polling for new approvals...");

        // Poll EVM bridge for approvals
        self.poll_evm_approvals().await?;

        // Poll Terra bridge for approvals
        self.poll_terra_approvals().await?;

        Ok(())
    }

    /// Poll EVM bridge for pending approvals
    async fn poll_evm_approvals(&mut self) -> Result<()> {
        debug!("Polling EVM approvals");

        // Create provider
        let provider = ProviderBuilder::new().on_http(
            self.config
                .evm_rpc_url
                .parse()
                .wrap_err("Invalid EVM RPC URL")?,
        );

        // Get current block
        let current_block = provider
            .get_block_number()
            .await
            .map_err(|e| eyre!("Failed to get block number: {}", e))?;

        // If first run, start from current block minus some lookback
        if self.last_evm_block == 0 {
            // Look back 100 blocks on first run
            self.last_evm_block = current_block.saturating_sub(100);
        }

        // Don't query if no new blocks
        if current_block <= self.last_evm_block {
            return Ok(());
        }

        let from_block = self.last_evm_block + 1;
        let to_block = current_block;

        debug!(
            from_block = from_block,
            to_block = to_block,
            "Querying EVM WithdrawApproved events"
        );

        // Parse bridge address
        let bridge_address = Address::from_str(&self.config.evm_bridge_address)
            .wrap_err("Invalid EVM bridge address")?;

        // Query for WithdrawApproved events
        let contract = CL8YBridge::new(bridge_address, &provider);

        // Query event logs
        let filter = contract
            .WithdrawApproved_filter()
            .from_block(from_block)
            .to_block(to_block);

        let logs = filter
            .query()
            .await
            .map_err(|e| eyre!("Failed to query events: {}", e))?;

        info!(
            from_block = from_block,
            to_block = to_block,
            event_count = logs.len(),
            "Found EVM WithdrawApproved events"
        );

        // Process each approval event
        for (event, log) in logs {
            let withdraw_hash: [u8; 32] = event.withdrawHash.0;

            // Skip if already processed
            if self.verified_hashes.contains(&withdraw_hash)
                || self.cancelled_hashes.contains(&withdraw_hash)
            {
                continue;
            }

            info!(
                withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                src_chain_key = %bytes32_to_hex(&event.srcChainKey.0),
                token = %event.token,
                to = %event.to,
                amount = %event.amount,
                nonce = %event.nonce,
                block = ?log.block_number,
                "Processing EVM approval event"
            );

            // Query approval details from contract for full info
            let approval_info = contract
                .getWithdrawApproval(FixedBytes::from(withdraw_hash))
                .call()
                .await;

            let (approved_at, delay) = match approval_info {
                Ok(info) => {
                    // Skip if already cancelled or executed
                    if info.cancelled {
                        debug!(withdraw_hash = %bytes32_to_hex(&withdraw_hash), "Already cancelled, skipping");
                        continue;
                    }
                    if info.executed {
                        debug!(withdraw_hash = %bytes32_to_hex(&withdraw_hash), "Already executed, skipping");
                        continue;
                    }
                    (info.approvedAt, 300u64) // Default delay, would query from contract
                }
                Err(e) => {
                    warn!(error = %e, "Failed to get approval info, skipping");
                    continue;
                }
            };

            // Convert token address to bytes32
            let mut dest_token_address = [0u8; 32];
            dest_token_address[12..32].copy_from_slice(event.token.as_slice());

            // Convert recipient address to bytes32
            let mut dest_account = [0u8; 32];
            dest_account[12..32].copy_from_slice(event.to.as_slice());

            // Get the amount as u128
            let amount: u128 = event.amount.try_into().unwrap_or(0);

            // Create a pending approval for verification
            let approval = PendingApproval {
                withdraw_hash,
                src_chain_key: event.srcChainKey.0,
                dest_chain_key: [0u8; 32], // EVM dest chain key - computed from chain ID
                dest_token_address,
                dest_account,
                amount,
                nonce: event.nonce.try_into().unwrap_or(0),
                approved_at_timestamp: approved_at,
                delay_seconds: delay,
            };

            // Verify and potentially cancel
            if let Err(e) = self.verify_and_cancel(&approval).await {
                error!(
                    error = %e,
                    withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                    "Failed to verify approval"
                );
            }
        }

        // Update last polled block
        self.last_evm_block = to_block;

        Ok(())
    }

    /// Poll Terra bridge for pending approvals
    async fn poll_terra_approvals(&mut self) -> Result<()> {
        debug!("Polling Terra approvals");

        // Query LCD for current height
        let client = reqwest::Client::new();
        let status_url = format!(
            "{}/cosmos/base/tendermint/v1beta1/blocks/latest",
            self.config.terra_lcd_url
        );

        let current_height = match client.get(&status_url).send().await {
            Ok(resp) => {
                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| eyre!("Failed to parse status: {}", e))?;
                json["block"]["header"]["height"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0)
            }
            Err(e) => {
                warn!(error = %e, "Failed to get Terra height");
                return Ok(());
            }
        };

        // If first run, start from current height minus some lookback
        if self.last_terra_height == 0 {
            self.last_terra_height = current_height.saturating_sub(100);
        }

        // Don't query if no new blocks
        if current_height <= self.last_terra_height {
            return Ok(());
        }

        debug!(
            from_height = self.last_terra_height,
            to_height = current_height,
            "Querying Terra pending approvals"
        );

        // Query the bridge contract for pending approvals
        let query = serde_json::json!({
            "pending_approvals": {
                "limit": 50
            }
        });

        let query_b64 =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&query)?);

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            self.config.terra_lcd_url, self.config.terra_bridge_address, query_b64
        );

        match client.get(&url).send().await {
            Ok(resp) => {
                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| eyre!("Failed to parse approvals: {}", e))?;

                // Parse pending approvals from response
                if let Some(approvals) = json["data"]["approvals"].as_array() {
                    info!(
                        approval_count = approvals.len(),
                        "Found pending Terra approvals"
                    );

                    for approval_json in approvals {
                        // Parse withdraw_hash from base64
                        let withdraw_hash_b64 =
                            approval_json["withdraw_hash"].as_str().unwrap_or("");

                        let withdraw_hash_bytes = base64::engine::general_purpose::STANDARD
                            .decode(withdraw_hash_b64)
                            .unwrap_or_default();

                        if withdraw_hash_bytes.len() != 32 {
                            continue;
                        }

                        let mut withdraw_hash = [0u8; 32];
                        withdraw_hash.copy_from_slice(&withdraw_hash_bytes);

                        // Skip if already processed
                        if self.verified_hashes.contains(&withdraw_hash)
                            || self.cancelled_hashes.contains(&withdraw_hash)
                        {
                            continue;
                        }

                        // Parse other fields
                        let src_chain_key =
                            self.parse_bytes32_from_json(&approval_json["src_chain_key"]);
                        let dest_chain_key =
                            self.parse_bytes32_from_json(&approval_json["dest_chain_key"]);
                        let dest_token_address =
                            self.parse_bytes32_from_json(&approval_json["dest_token_address"]);
                        let dest_account =
                            self.parse_bytes32_from_json(&approval_json["dest_account"]);

                        let amount: u128 = approval_json["amount"]
                            .as_str()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);

                        let nonce: u64 = approval_json["nonce"].as_u64().unwrap_or(0);

                        let approved_at_timestamp: u64 =
                            approval_json["approved_at"].as_u64().unwrap_or(0);

                        let delay_seconds: u64 =
                            approval_json["delay_seconds"].as_u64().unwrap_or(300);

                        info!(
                            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                            nonce = nonce,
                            amount = amount,
                            "Processing Terra approval"
                        );

                        let approval = PendingApproval {
                            withdraw_hash,
                            src_chain_key,
                            dest_chain_key,
                            dest_token_address,
                            dest_account,
                            amount,
                            nonce,
                            approved_at_timestamp,
                            delay_seconds,
                        };

                        // Verify and potentially cancel
                        if let Err(e) = self.verify_and_cancel(&approval).await {
                            error!(
                                error = %e,
                                withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                                "Failed to verify Terra approval"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to query Terra approvals");
            }
        }

        // Update last polled height
        self.last_terra_height = current_height;

        Ok(())
    }

    /// Helper to parse bytes32 from JSON (base64 encoded)
    fn parse_bytes32_from_json(&self, value: &serde_json::Value) -> [u8; 32] {
        let b64 = value.as_str().unwrap_or("");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap_or_default();

        let mut result = [0u8; 32];
        if bytes.len() >= 32 {
            result.copy_from_slice(&bytes[..32]);
        } else if !bytes.is_empty() {
            result[..bytes.len()].copy_from_slice(&bytes);
        }
        result
    }

    /// Verify an approval and potentially cancel it
    pub async fn verify_and_cancel(&mut self, approval: &PendingApproval) -> Result<()> {
        // Skip if already verified or cancelled
        if self.verified_hashes.contains(&approval.withdraw_hash) {
            return Ok(());
        }
        if self.cancelled_hashes.contains(&approval.withdraw_hash) {
            return Ok(());
        }

        let result = self.verifier.verify(approval).await?;

        match result {
            VerificationResult::Valid => {
                info!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    "Approval verified as VALID"
                );
                self.verified_hashes.insert(approval.withdraw_hash);
            }
            VerificationResult::Invalid { reason } => {
                warn!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    reason = %reason,
                    "Approval is INVALID - submitting cancellation"
                );

                // Submit cancel transaction
                if let Err(e) = self.submit_cancel(approval).await {
                    error!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        error = %e,
                        "Failed to submit cancellation"
                    );
                } else {
                    self.cancelled_hashes.insert(approval.withdraw_hash);
                }
            }
            VerificationResult::Pending => {
                debug!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    "Verification pending - will retry"
                );
            }
        }

        Ok(())
    }

    /// Submit cancel transaction to the appropriate chain
    async fn submit_cancel(&self, approval: &PendingApproval) -> Result<()> {
        // Determine which chain the approval is on (the destination chain)
        // The dest_chain_key tells us where to submit the cancellation

        let withdraw_hash = approval.withdraw_hash;

        // Check if it's an EVM chain (try EVM first)
        if self
            .evm_client
            .can_cancel(withdraw_hash)
            .await
            .unwrap_or(false)
        {
            info!(
                hash = %bytes32_to_hex(&withdraw_hash),
                "Submitting cancellation to EVM"
            );

            match self
                .evm_client
                .cancel_withdraw_approval(withdraw_hash)
                .await
            {
                Ok(tx_hash) => {
                    info!(
                        tx_hash = %tx_hash,
                        hash = %bytes32_to_hex(&withdraw_hash),
                        "EVM cancellation submitted"
                    );
                    return Ok(());
                }
                Err(e) => {
                    warn!(error = %e, "EVM cancellation failed, trying Terra");
                }
            }
        }

        // Try Terra
        if self
            .terra_client
            .can_cancel(withdraw_hash)
            .await
            .unwrap_or(false)
        {
            info!(
                hash = %bytes32_to_hex(&withdraw_hash),
                "Submitting cancellation to Terra"
            );

            let tx_hash = self
                .terra_client
                .cancel_withdraw_approval(withdraw_hash)
                .await?;
            info!(
                tx_hash = %tx_hash,
                hash = %bytes32_to_hex(&withdraw_hash),
                "Terra cancellation submitted"
            );
            return Ok(());
        }

        warn!(
            hash = %bytes32_to_hex(&withdraw_hash),
            "Could not submit cancellation to any chain"
        );

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> (usize, usize) {
        (self.verified_hashes.len(), self.cancelled_hashes.len())
    }
}
