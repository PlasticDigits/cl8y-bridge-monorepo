//! Terra Writer - Submits Withdrawal Approvals and Executions
//!
//! Implements the watchtower pattern for incoming transfers to Terra.
//!
//! ## V2 Flow (Hash-matching, no recomputation)
//! 1. User calls WithdrawSubmit on Terra (pays gas, hash stored on-chain)
//! 2. Operator polls PendingWithdrawals on Terra for unapproved entries
//! 3. Operator verifies each hash against EVM Bridge's getDeposit(hash)
//! 4. If deposit exists on EVM, operator calls WithdrawApprove(hash) on Terra
//! 5. Cancelers can cancel during the cancel window
//! 6. Anyone can call WithdrawExecuteUnlock/Mint after window

use std::collections::HashMap;
use std::time::{Duration, Instant};

use alloy::primitives::{Address, FixedBytes};
use alloy::providers::ProviderBuilder;
use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use sqlx::PgPool;
use tracing::{debug, info, warn};

use crate::config::TerraConfig;
use crate::contracts::evm_bridge::Bridge as EvmBridge;
use crate::contracts::terra_bridge::{
    build_withdraw_approve_msg_v2, build_withdraw_execute_unlock_msg_v2,
};
use crate::db;
use crate::hash::bytes32_to_hex;
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
///
/// Uses hash-matching: polls Terra PendingWithdrawals, verifies against EVM
/// Bridge deposits, and approves by hash. No hash recomputation needed.
pub struct TerraWriter {
    lcd_url: String,
    #[allow(dead_code)]
    chain_id: String,
    contract_address: String,
    client: Client,
    terra_client: TerraClient,
    #[allow(dead_code)]
    db: PgPool,
    /// Cancel window in seconds
    cancel_window: u64,
    /// Fee recipient for withdrawals
    #[allow(dead_code)]
    fee_recipient: String,
    /// Pending approvals awaiting execution
    pending_executions: HashMap<[u8; 32], PendingExecution>,
    /// This chain's 4-byte chain ID (V2)
    #[allow(dead_code)]
    this_chain_id: ChainId,
    /// Source chain endpoints for cross-chain deposit verification routing.
    /// Maps V2 4-byte chain ID → (rpc_url, bridge_address).
    /// Includes all known EVM chains so we can verify deposits on any source.
    source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
    /// Set of hashes we've already approved (avoid duplicate approvals)
    approved_hashes: HashMap<[u8; 32], Instant>,
}

impl TerraWriter {
    /// Create a new Terra writer
    ///
    /// Requires Terra config and source chain endpoints: the operator verifies
    /// deposits on the correct EVM chain before approving withdrawals on Terra.
    ///
    /// `source_chain_endpoints` maps V2 4-byte chain IDs to (rpc_url, bridge_address)
    /// for all known EVM chains, enabling deposit verification regardless of which
    /// EVM chain the deposit originated from.
    pub async fn new(
        terra_config: &TerraConfig,
        source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
        db: PgPool,
    ) -> Result<Self> {
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

        // Get this chain's V2 ID
        // IMPORTANT: Must use the 4-byte ChainRegistry ID (e.g. 0x00000002),
        // NOT the native chain ID or hardcoded values.
        let this_chain_id = if let Some(id) = terra_config.this_chain_id {
            ChainId::from_u32(id)
        } else {
            match Self::query_this_chain_id(
                &client,
                &terra_config.lcd_url,
                &terra_config.bridge_address,
            )
            .await
            {
                Ok(id) => {
                    info!(
                        v2_chain_id = %id.to_hex(),
                        "Queried V2 chain ID from Terra contract"
                    );
                    id
                }
                Err(e) => {
                    return Err(eyre::eyre!(
                        "Cannot resolve Terra V2 chain ID: contract query failed ({}) and \
                         TERRA_THIS_CHAIN_ID is not set. Set TERRA_THIS_CHAIN_ID to the V2 \
                         chain ID from ChainRegistry (e.g., TERRA_THIS_CHAIN_ID=2).",
                        e
                    ));
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
            source_chains = source_chain_endpoints.len(),
            "Terra writer initialized (V2 hash-matching, multi-EVM verification)"
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
            source_chain_endpoints,
            approved_hashes: HashMap::new(),
        })
    }

    // ========================================================================
    // Main Processing Loop
    // ========================================================================

    /// Process pending withdrawals using hash-matching
    ///
    /// 1. Check if any pending executions are ready (cancel window elapsed)
    /// 2. Poll Terra PendingWithdrawals for unapproved entries
    /// 3. For each unapproved entry, verify the deposit exists on EVM
    /// 4. If verified, call WithdrawApprove(hash) on Terra
    pub async fn process_pending(&mut self) -> Result<()> {
        // First, check if any pending executions are ready
        self.process_pending_executions().await?;

        // Then poll Terra for unapproved withdrawals and verify against EVM
        self.poll_and_approve().await?;

        // Clean up old approved hashes (older than 1 hour)
        let cutoff = Instant::now() - Duration::from_secs(3600);
        self.approved_hashes.retain(|_, t| *t > cutoff);

        Ok(())
    }

    /// Poll Terra PendingWithdrawals and approve verified entries
    async fn poll_and_approve(&mut self) -> Result<()> {
        let mut start_after: Option<String> = None;
        let page_limit = 30u32;
        let mut total_processed = 0u32;
        let mut total_approved = 0u32;
        let mut total_skipped_already_approved = 0u32;
        let mut total_no_evm_deposit = 0u32;
        let mut total_evm_errors = 0u32;

        loop {
            // Query Terra for pending withdrawals (paginated)
            let query = if let Some(ref cursor) = start_after {
                serde_json::json!({
                    "pending_withdrawals": {
                        "start_after": cursor,
                        "limit": page_limit
                    }
                })
            } else {
                serde_json::json!({
                    "pending_withdrawals": {
                        "limit": page_limit
                    }
                })
            };

            let query_b64 = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                serde_json::to_string(&query)?,
            );

            let url = format!(
                "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
                self.lcd_url, self.contract_address, query_b64
            );

            debug!(
                url = %url,
                page_cursor = start_after.as_deref().unwrap_or("(first page)"),
                "Querying Terra PendingWithdrawals via LCD"
            );

            let response: serde_json::Value = match self.client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        warn!(
                            status = %status,
                            body = %body,
                            "Terra LCD returned non-success status for PendingWithdrawals"
                        );
                        return Ok(());
                    }
                    match resp.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(error = %e, "Failed to parse Terra PendingWithdrawals response as JSON");
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        lcd_url = %self.lcd_url,
                        contract = %self.contract_address,
                        "Failed to query Terra PendingWithdrawals (LCD unreachable?)"
                    );
                    return Ok(());
                }
            };

            let withdrawals = match response["data"]["withdrawals"].as_array() {
                Some(arr) => arr.clone(),
                None => {
                    // Enhanced diagnostic: log full response structure for debugging
                    let response_preview = serde_json::to_string(&response)
                        .unwrap_or_default()
                        .chars()
                        .take(500)
                        .collect::<String>();
                    warn!(
                        response_keys = ?response.as_object().map(|o| o.keys().collect::<Vec<_>>()),
                        response_preview = %response_preview,
                        lcd_url = %self.lcd_url,
                        contract = %self.contract_address,
                        "No pending withdrawals data in LCD response. \
                         Expected response[\"data\"][\"withdrawals\"] array. \
                         Check LCD URL and contract address are correct."
                    );
                    return Ok(());
                }
            };

            if withdrawals.is_empty() {
                debug!("No pending withdrawals returned from Terra");
                break;
            }

            // Count entries by status for logging
            let unapproved_count = withdrawals
                .iter()
                .filter(|e| {
                    !e["approved"].as_bool().unwrap_or(false)
                        && !e["cancelled"].as_bool().unwrap_or(false)
                        && !e["executed"].as_bool().unwrap_or(false)
                })
                .count();

            info!(
                total = withdrawals.len(),
                unapproved = unapproved_count,
                page_cursor = start_after.as_deref().unwrap_or("(first page)"),
                "Polled Terra PendingWithdrawals"
            );

            let mut last_hash: Option<String> = None;

            for entry in &withdrawals {
                total_processed += 1;
                let approved = entry["approved"].as_bool().unwrap_or(false);
                let cancelled = entry["cancelled"].as_bool().unwrap_or(false);
                let executed = entry["executed"].as_bool().unwrap_or(false);
                let nonce = entry["nonce"].as_u64().unwrap_or(0);

                // Only process unapproved, non-cancelled, non-executed entries
                if approved || cancelled || executed {
                    // Track the hash for pagination cursor
                    if let Some(h) = entry["withdraw_hash"].as_str() {
                        last_hash = Some(h.to_string());
                    }
                    debug!(
                        nonce = nonce,
                        approved = approved,
                        cancelled = cancelled,
                        executed = executed,
                        "Skipping already-processed withdrawal entry"
                    );
                    continue;
                }

                // Extract withdraw_hash (base64 encoded)
                let hash_b64 = match entry["withdraw_hash"].as_str() {
                    Some(h) => h,
                    None => {
                        warn!(
                            nonce = nonce,
                            entry = %serde_json::to_string(entry).unwrap_or_default(),
                            "Withdrawal entry missing withdraw_hash field"
                        );
                        continue;
                    }
                };

                last_hash = Some(hash_b64.to_string());

                let hash_bytes = match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    hash_b64,
                ) {
                    Ok(b) if b.len() == 32 => {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&b);
                        arr
                    }
                    Ok(b) => {
                        warn!(
                            hash_b64 = hash_b64,
                            decoded_len = b.len(),
                            nonce = nonce,
                            "Invalid withdraw_hash: decoded to {} bytes (expected 32)",
                            b.len()
                        );
                        continue;
                    }
                    Err(e) => {
                        warn!(
                            hash_b64 = hash_b64,
                            nonce = nonce,
                            error = %e,
                            "Failed to base64-decode withdraw_hash"
                        );
                        continue;
                    }
                };

                // Skip if we've already approved this hash
                if self.approved_hashes.contains_key(&hash_bytes) {
                    total_skipped_already_approved += 1;
                    debug!(
                        withdraw_hash = %bytes32_to_hex(&hash_bytes),
                        nonce = nonce,
                        "Skipping already-approved hash (in cache)"
                    );
                    continue;
                }

                // Extract src_chain from the withdrawal entry (V2 4-byte chain ID, base64)
                let src_chain_id: [u8; 4] = entry["src_chain"]
                    .as_str()
                    .and_then(|b64| {
                        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).ok()
                    })
                    .and_then(|b| b.try_into().ok())
                    .unwrap_or([0u8; 4]);

                // Log the entry details for debugging
                let amount = entry["amount"].as_str().unwrap_or("?");
                let token = entry["token"].as_str().unwrap_or("?");
                let recipient = entry["recipient"].as_str().unwrap_or("?");
                info!(
                    withdraw_hash = %bytes32_to_hex(&hash_bytes),
                    src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                    nonce = nonce,
                    amount = amount,
                    token = token,
                    recipient = recipient,
                    "Processing unapproved withdrawal, verifying EVM deposit..."
                );

                // Verify the deposit exists on the correct source EVM chain
                match self.verify_evm_deposit(&hash_bytes, &src_chain_id).await {
                    Ok(true) => {
                        // Deposit verified — approve on Terra
                        info!(
                            withdraw_hash = %bytes32_to_hex(&hash_bytes),
                            nonce = nonce,
                            src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                            "EVM deposit verified, submitting WithdrawApprove on Terra"
                        );

                        match self.submit_approve(&hash_bytes).await {
                            Ok(tx_hash) => {
                                total_approved += 1;
                                info!(
                                    tx_hash = %tx_hash,
                                    withdraw_hash = %bytes32_to_hex(&hash_bytes),
                                    nonce = nonce,
                                    "WithdrawApprove submitted successfully on Terra"
                                );

                                // Update shared DB: mark evm_deposit as processed so both writers
                                // see consistent state (pending_deposits count decreases).
                                // Use src_chain from the withdrawal entry (V2 4-byte) for multi-EVM support.
                                let src_v2_chain_id: Option<[u8; 4]> = entry["src_chain"]
                                    .as_str()
                                    .and_then(|b64| {
                                        base64::Engine::decode(
                                            &base64::engine::general_purpose::STANDARD,
                                            b64,
                                        )
                                        .ok()
                                    })
                                    .and_then(|b| b.try_into().ok());
                                if let Some(src) = src_v2_chain_id {
                                    if let Ok(Some(deposit_id)) =
                                        db::find_evm_deposit_id_by_src_v2_chain_nonce_for_cosmos(
                                            &self.db,
                                            &src,
                                            nonce as i64,
                                        )
                                        .await
                                    {
                                        if let Err(e) = db::update_evm_deposit_status(
                                            &self.db,
                                            deposit_id,
                                            "processed",
                                        )
                                        .await
                                        {
                                            warn!(
                                                deposit_id = deposit_id,
                                                nonce = nonce,
                                                error = %e,
                                                "Failed to update evm_deposit status after Terra approval"
                                            );
                                        } else {
                                            debug!(
                                                deposit_id = deposit_id,
                                                nonce = nonce,
                                                "Marked evm_deposit as processed (shared data source)"
                                            );
                                        }
                                    }
                                } else {
                                    debug!(
                                        nonce = nonce,
                                        "Could not parse src_chain from withdrawal entry, skipping evm_deposit update"
                                    );
                                }

                                // Track for auto-execution after cancel window
                                self.pending_executions.insert(
                                    hash_bytes,
                                    PendingExecution {
                                        withdraw_hash: hash_bytes,
                                        approved_at: Instant::now(),
                                        delay_seconds: self.cancel_window,
                                        attempts: 0,
                                    },
                                );

                                // Remember we approved this hash
                                self.approved_hashes.insert(hash_bytes, Instant::now());
                            }
                            Err(e) => {
                                warn!(
                                    withdraw_hash = %bytes32_to_hex(&hash_bytes),
                                    nonce = nonce,
                                    error = %e,
                                    operator_address = %self.terra_client.address,
                                    contract = %self.contract_address,
                                    "Failed to submit WithdrawApprove on Terra. \
                                     Check: (1) operator is registered on Terra bridge, \
                                     (2) account has sufficient gas, \
                                     (3) withdrawal not already approved."
                                );
                            }
                        }
                    }
                    Ok(false) => {
                        total_no_evm_deposit += 1;
                        // Log at info level (not debug) to make it visible
                        info!(
                            withdraw_hash = %bytes32_to_hex(&hash_bytes),
                            nonce = nonce,
                            src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                            known_evm_chains = self.source_chain_endpoints.len(),
                            "No matching EVM deposit found for withdraw hash. \
                             This withdrawal cannot be approved until the deposit is confirmed on EVM. \
                             Possible causes: (1) hash mismatch between chains, \
                             (2) deposit not yet finalized, (3) source chain not configured."
                        );
                    }
                    Err(e) => {
                        total_evm_errors += 1;
                        warn!(
                            withdraw_hash = %bytes32_to_hex(&hash_bytes),
                            nonce = nonce,
                            error = %e,
                            src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                            "Failed to query EVM deposit on source chain (RPC error). \
                             Will retry on next poll cycle."
                        );
                    }
                }
            }

            // If we got fewer than page_limit results, we're done
            if withdrawals.len() < page_limit as usize {
                break;
            }

            // Set cursor for next page
            start_after = last_hash;
            if start_after.is_none() {
                break;
            }
        }

        // Summary log for the poll cycle
        if total_processed > 0 {
            info!(
                total_processed = total_processed,
                total_approved = total_approved,
                skipped_already_approved = total_skipped_already_approved,
                no_evm_deposit = total_no_evm_deposit,
                evm_errors = total_evm_errors,
                pending_executions = self.pending_executions.len(),
                "Terra poll cycle complete"
            );
        }

        Ok(())
    }

    // ========================================================================
    // EVM Deposit Verification
    // ========================================================================

    /// Verify that a deposit with the given hash exists on the correct EVM source chain.
    ///
    /// Routes verification to the EVM chain identified by `src_chain_id` (V2 4-byte ID)
    /// from the pending withdrawal entry. If the source chain is not in
    /// `source_chain_endpoints`, falls back to trying all known chains.
    ///
    /// Returns `true` if the deposit record has a non-zero timestamp on any chain.
    async fn verify_evm_deposit(
        &self,
        withdraw_hash: &[u8; 32],
        src_chain_id: &[u8; 4],
    ) -> Result<bool> {
        let src_chain_hex = format!("0x{}", hex::encode(src_chain_id));

        // Try the specific source chain first (preferred — O(1) routing)
        if let Some((rpc_url, bridge_address)) = self.source_chain_endpoints.get(src_chain_id) {
            debug!(
                withdraw_hash = %bytes32_to_hex(withdraw_hash),
                src_chain = %src_chain_hex,
                evm_rpc = %rpc_url,
                evm_bridge = %bridge_address,
                "Routing deposit verification to source chain endpoint"
            );

            return self
                .verify_evm_deposit_on_chain(rpc_url, *bridge_address, withdraw_hash)
                .await;
        }

        // Source chain not in endpoints map — try all known chains as fallback.
        // This handles the case where src_chain_id is zero or unrecognized.
        warn!(
            withdraw_hash = %bytes32_to_hex(withdraw_hash),
            src_chain = %src_chain_hex,
            known_chains = self.source_chain_endpoints.len(),
            "Source chain not found in endpoints map, trying all known EVM chains"
        );

        for (chain_id, (rpc_url, bridge_address)) in &self.source_chain_endpoints {
            let chain_hex = format!("0x{}", hex::encode(chain_id));
            debug!(
                withdraw_hash = %bytes32_to_hex(withdraw_hash),
                trying_chain = %chain_hex,
                evm_rpc = %rpc_url,
                "Trying fallback chain for deposit verification"
            );

            match self
                .verify_evm_deposit_on_chain(rpc_url, *bridge_address, withdraw_hash)
                .await
            {
                Ok(true) => {
                    info!(
                        withdraw_hash = %bytes32_to_hex(withdraw_hash),
                        found_on_chain = %chain_hex,
                        expected_src_chain = %src_chain_hex,
                        "Deposit found on fallback chain (src_chain_id mismatch?)"
                    );
                    return Ok(true);
                }
                Ok(false) => continue,
                Err(e) => {
                    debug!(
                        chain = %chain_hex,
                        error = %e,
                        "Fallback chain query failed, continuing"
                    );
                    continue;
                }
            }
        }

        info!(
            withdraw_hash = %bytes32_to_hex(withdraw_hash),
            src_chain = %src_chain_hex,
            chains_checked = self.source_chain_endpoints.len(),
            "EVM deposit NOT found on any known chain"
        );

        Ok(false)
    }

    /// Verify a deposit exists on a specific EVM chain by querying `getDeposit(hash)`.
    ///
    /// Returns `true` if the deposit record has a non-zero timestamp.
    async fn verify_evm_deposit_on_chain(
        &self,
        rpc_url: &str,
        bridge_address: Address,
        withdraw_hash: &[u8; 32],
    ) -> Result<bool> {
        let provider =
            ProviderBuilder::new().on_http(rpc_url.parse().wrap_err("Invalid EVM RPC URL")?);

        let contract = EvmBridge::new(bridge_address, &provider);
        let hash_fixed: FixedBytes<32> = FixedBytes::from(*withdraw_hash);

        let result = match contract.getDeposit(hash_fixed).call().await {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    withdraw_hash = %bytes32_to_hex(withdraw_hash),
                    error = %e,
                    evm_rpc = rpc_url,
                    evm_bridge = %bridge_address,
                    "EVM getDeposit() call failed. Possible causes: \
                     (1) EVM RPC unreachable, (2) wrong bridge address, \
                     (3) contract not deployed at address."
                );
                return Err(e.into());
            }
        };

        // A deposit exists if its timestamp is non-zero
        let exists = result.timestamp != alloy::primitives::U256::ZERO;

        if exists {
            info!(
                withdraw_hash = %bytes32_to_hex(withdraw_hash),
                nonce = result.nonce,
                token = %result.token,
                amount = %result.amount,
                timestamp = %result.timestamp,
                dest_chain = %result.destChain,
                evm_rpc = rpc_url,
                "EVM deposit verified: deposit record found on-chain"
            );
        } else {
            debug!(
                withdraw_hash = %bytes32_to_hex(withdraw_hash),
                evm_bridge = %bridge_address,
                evm_rpc = rpc_url,
                "EVM deposit not found on this chain (zero timestamp)"
            );
        }

        Ok(exists)
    }

    // ========================================================================
    // Terra Transaction Submission
    // ========================================================================

    /// Submit WithdrawApprove(hash) on Terra
    async fn submit_approve(&self, withdraw_hash: &[u8; 32]) -> Result<String> {
        let msg = build_withdraw_approve_msg_v2(*withdraw_hash);

        let msg_json = serde_json::to_string(&msg)?;
        debug!(msg = %msg_json, "WithdrawApprove message (V2)");

        let tx_hash = self
            .terra_client
            .execute_contract(&self.contract_address, &msg, vec![])
            .await
            .map_err(|e| eyre!("Failed to execute WithdrawApprove: {}", e))?;

        Ok(tx_hash)
    }

    /// Submit WithdrawExecuteUnlock on Terra (after cancel window)
    async fn submit_execute_withdraw(&self, withdraw_hash: [u8; 32]) -> Result<String> {
        let msg = build_withdraw_execute_unlock_msg_v2(withdraw_hash);

        let msg_json = serde_json::to_string(&msg)?;
        debug!(msg = %msg_json, "WithdrawExecuteUnlock message (V2)");

        let tx_hash = self
            .terra_client
            .execute_contract(&self.contract_address, &msg, vec![])
            .await
            .map_err(|e| eyre!("Failed to execute WithdrawExecuteUnlock: {}", e))?;

        Ok(tx_hash)
    }

    // ========================================================================
    // Pending Execution Management
    // ========================================================================

    /// Process pending executions (after cancel window has elapsed)
    async fn process_pending_executions(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (hash, pending) in &self.pending_executions {
            let elapsed = now.duration_since(pending.approved_at);

            if elapsed.as_secs() >= pending.delay_seconds {
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
                    }
                }
            }
        }

        for hash in to_remove {
            self.pending_executions.remove(&hash);
        }

        Ok(())
    }

    // ========================================================================
    // Contract Queries
    // ========================================================================

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

    // ========================================================================
    // Utility
    // ========================================================================

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
