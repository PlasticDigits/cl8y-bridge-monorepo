//! Watcher for monitoring approvals across chains and submitting cancellations
//!
//! This module implements the core polling loop for the canceler service. It monitors
//! both EVM and Terra chains for `WithdrawApproved` events, verifies each approval
//! against the source chain, and submits cancellation transactions for fraudulent
//! approvals (those without corresponding deposits).
//!
//! # Architecture
//!
//! The watcher operates in a continuous polling loop:
//! 1. Polls EVM bridge for `WithdrawApproved` events in the current block range
//! 2. Polls Terra bridge for pending withdrawal approvals
//! 3. For each approval found, uses `ApprovalVerifier` to check if a matching deposit exists
//! 4. Submits cancellation transactions for any fraudulent approvals detected
//!
//! # EVM Event Filtering
//!
//! Uses the Alloy library to query `WithdrawApproved` events with explicit address
//! filtering. The event signature hash is:
//! ```text
//! keccak256("WithdrawApproved(bytes32,bytes32,address,bytes,uint256,uint256)")
//! = 0xe9c6fcc209e99f20220bb87e197c5584cdd23cee08c7919db0d702ee7dc2c8a2
//! ```
//!
//! # E2E Test Integration
//!
//! When run as a subprocess in E2E tests, the canceler must be spawned using
//! `setsid --fork` to create a fully detached process. Direct subprocess spawning
//! causes the process to die during async operations due to signal inheritance
//! issues. See `packages/e2e/src/services.rs::start_canceler()` for the spawn
//! implementation.

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
use crate::server::{SharedMetrics, SharedStats};
use crate::terra_client::TerraClient;
use crate::verifier::{ApprovalVerifier, PendingApproval, VerificationResult};

/// Compute keccak256 hash of event signature for debugging
fn compute_event_topic(signature: &str) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};
    let mut hasher = Keccak::v256();
    hasher.update(signature.as_bytes());
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

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
    /// Shared stats for health endpoint
    stats: SharedStats,
    /// Prometheus metrics
    metrics: SharedMetrics,
}

impl CancelerWatcher {
    pub async fn new(config: &Config, stats: SharedStats, metrics: SharedMetrics) -> Result<Self> {
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

        // Initialize stats with canceler ID
        {
            let mut s = stats.write().await;
            s.canceler_id = config.canceler_id.clone();
        }

        info!(
            canceler_id = %config.canceler_id,
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
            stats,
            metrics,
        })
    }

    /// Main run loop
    pub async fn run(&mut self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        info!("Canceler watcher starting...");

        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);
        info!(
            poll_interval_ms = self.config.poll_interval_ms,
            "Entering main poll loop"
        );

        loop {
            debug!("Poll loop iteration starting");
            tokio::select! {
                result = shutdown.recv() => {
                    if result.is_some() {
                        info!("Shutdown signal received");
                    } else {
                        warn!("Shutdown channel closed unexpectedly (sender dropped)");
                    }
                    break;
                }
                _ = tokio::time::sleep(poll_interval) => {
                    debug!("Sleep completed, starting poll");
                    if let Err(e) = self.poll_approvals().await {
                        error!(error = %e, "Error polling approvals");
                    }
                    debug!("Poll completed");
                }
            }
        }

        info!("Canceler watcher exiting main loop");
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
    ///
    /// This function queries WithdrawApproved events from the EVM bridge contract
    /// and verifies each approval against the source chain. If an approval is
    /// found to be fraudulent (no matching deposit), it submits a cancel transaction.
    async fn poll_evm_approvals(&mut self) -> Result<()> {
        debug!(
            canceler_address = %self.evm_client.address(),
            bridge_address = %self.config.evm_bridge_address,
            "Polling EVM approvals"
        );

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

        // If first run, start from genesis to catch all events
        // This is important for testing scenarios where approvals may have been
        // created before the canceler started
        if self.last_evm_block == 0 {
            // Start from block 0 on first run to ensure we catch all events
            // For production, this could be set to a more recent block
            self.last_evm_block = 0;
            info!(
                current_block = current_block,
                lookback_start = self.last_evm_block,
                "First poll - starting from genesis to catch all events"
            );
        }

        // Don't query if no new blocks
        if current_block <= self.last_evm_block {
            debug!(
                current_block = current_block,
                last_polled = self.last_evm_block,
                "No new blocks to poll"
            );
            return Ok(());
        }

        let from_block = self.last_evm_block + 1;
        let to_block = current_block;

        info!(
            from_block = from_block,
            to_block = to_block,
            block_range = to_block - from_block + 1,
            "Querying EVM WithdrawApproved events"
        );

        // Parse bridge address
        let bridge_address = Address::from_str(&self.config.evm_bridge_address)
            .wrap_err("Invalid EVM bridge address")?;

        // Query for WithdrawApproved events
        let contract = CL8YBridge::new(bridge_address, &provider);

        // Query event logs - explicitly add address filter to ensure it's included
        let filter = contract
            .WithdrawApproved_filter()
            .address(bridge_address)
            .from_block(from_block)
            .to_block(to_block);

        // Compute and log the expected event topic for debugging
        let event_signature = "WithdrawApproved(bytes32,bytes32,address,address,uint256,uint256,uint256,address,bool)";
        let expected_topic = compute_event_topic(event_signature);
        debug!(
            bridge_address = %bridge_address,
            from_block = from_block,
            to_block = to_block,
            event_topic = %format!("0x{}", hex::encode(expected_topic)),
            "Querying WithdrawApproved events with explicit address filter"
        );

        let logs = filter
            .query()
            .await
            .map_err(|e| eyre!("Failed to query events: {}", e))?;

        // If no logs found via alloy, try raw RPC query for debugging
        if logs.is_empty() {
            // Try a raw eth_getLogs call to verify events exist
            let client = reqwest::Client::new();
            let topic0 = format!("0x{}", hex::encode(expected_topic));
            let raw_filter = serde_json::json!({
                "address": format!("{}", bridge_address),
                "topics": [topic0],
                "fromBlock": format!("0x{:x}", from_block),
                "toBlock": format!("0x{:x}", to_block)
            });

            debug!(
                raw_filter = %raw_filter,
                "Trying raw eth_getLogs query for debugging"
            );

            let raw_response = client
                .post(self.config.evm_rpc_url.as_str())
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_getLogs",
                    "params": [raw_filter],
                    "id": 1
                }))
                .send()
                .await;

            match raw_response {
                Ok(resp) => {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        if let Some(result) = body.get("result") {
                            let log_count = result.as_array().map(|a| a.len()).unwrap_or(0);
                            if log_count > 0 {
                                warn!(
                                    log_count = log_count,
                                    "Raw RPC found events but alloy filter did not! Possible filter issue."
                                );
                                // Log the raw events for debugging
                                debug!(raw_logs = %result, "Raw event logs from eth_getLogs");
                            } else {
                                info!(
                                    bridge_address = %bridge_address,
                                    from_block = from_block,
                                    to_block = to_block,
                                    "No WithdrawApproved events found (confirmed by raw RPC)"
                                );
                            }
                        }
                        if let Some(error) = body.get("error") {
                            warn!(error = %error, "Raw RPC eth_getLogs returned error");
                        }
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Failed to make raw RPC call");
                }
            }
        } else {
            info!(
                from_block = from_block,
                to_block = to_block,
                event_count = logs.len(),
                "Found EVM WithdrawApproved events - processing each for fraud detection"
            );
        }

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
            // Compute dest_chain_key from the EVM chain ID (this is the chain we're monitoring)
            let dest_chain_key = crate::hash::evm_chain_key(self.config.evm_chain_id);

            let approval = PendingApproval {
                withdraw_hash,
                src_chain_key: event.srcChainKey.0,
                dest_chain_key,
                dest_token_address,
                dest_account,
                amount,
                nonce: event.nonce.try_into().unwrap_or(0),
                approved_at_timestamp: approved_at,
                delay_seconds: delay,
            };

            // Verify and potentially cancel
            info!(
                withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                nonce = approval.nonce,
                amount = approval.amount,
                "Calling verify_and_cancel for EVM approval"
            );

            if let Err(e) = self.verify_and_cancel(&approval).await {
                error!(
                    error = %e,
                    withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                    "Failed to verify approval"
                );
            } else {
                debug!(
                    withdraw_hash = %bytes32_to_hex(&withdraw_hash),
                    "verify_and_cancel completed"
                );
            }
        }

        // Update last polled block
        self.last_evm_block = to_block;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.last_evm_block = to_block;
        }

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

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.last_terra_height = current_height;
        }

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

                // Update stats and metrics
                {
                    let mut stats = self.stats.write().await;
                    stats.verified_valid += 1;
                }
                self.metrics.verified_valid_total.inc();
            }
            VerificationResult::Invalid { reason } => {
                warn!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    reason = %reason,
                    "Approval is INVALID - submitting cancellation"
                );

                // Update stats and metrics for invalid detection
                {
                    let mut stats = self.stats.write().await;
                    stats.verified_invalid += 1;
                }
                self.metrics.verified_invalid_total.inc();

                // Submit cancel transaction
                if let Err(e) = self.submit_cancel(approval).await {
                    error!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        error = %e,
                        "Failed to submit cancellation"
                    );
                } else {
                    self.cancelled_hashes.insert(approval.withdraw_hash);

                    // Update cancelled count and metrics
                    {
                        let mut stats = self.stats.write().await;
                        stats.cancelled_count += 1;
                    }
                    self.metrics.cancelled_total.inc();
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
    ///
    /// This function attempts to submit a cancel transaction to the EVM chain first,
    /// then falls back to Terra if the EVM cancel fails or is not applicable.
    async fn submit_cancel(&self, approval: &PendingApproval) -> Result<()> {
        // Determine which chain the approval is on (the destination chain)
        // The dest_chain_key tells us where to submit the cancellation

        let withdraw_hash = approval.withdraw_hash;

        info!(
            hash = %bytes32_to_hex(&withdraw_hash),
            canceler_address = %self.evm_client.address(),
            "Attempting to submit cancellation transaction"
        );

        // Check if it's an EVM chain (try EVM first)
        let can_cancel_evm = match self.evm_client.can_cancel(withdraw_hash).await {
            Ok(can) => {
                debug!(
                    hash = %bytes32_to_hex(&withdraw_hash),
                    can_cancel = can,
                    "Checked EVM can_cancel status"
                );
                can
            }
            Err(e) => {
                warn!(
                    error = %e,
                    hash = %bytes32_to_hex(&withdraw_hash),
                    "Failed to check can_cancel on EVM, will try anyway"
                );
                true // Try anyway
            }
        };

        if can_cancel_evm {
            info!(
                hash = %bytes32_to_hex(&withdraw_hash),
                canceler_address = %self.evm_client.address(),
                "Submitting cancelWithdrawApproval transaction to EVM"
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
                        "EVM cancellation transaction SUCCEEDED"
                    );
                    return Ok(());
                }
                Err(e) => {
                    // Log detailed error for debugging CANCELER_ROLE issues
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&withdraw_hash),
                        canceler_address = %self.evm_client.address(),
                        "EVM cancellation FAILED - check if canceler has CANCELER_ROLE"
                    );
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
