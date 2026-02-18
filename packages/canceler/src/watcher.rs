//! Watcher for monitoring approvals across chains and submitting cancellations (V2)
//!
//! This module implements the core polling loop for the canceler service. It monitors
//! both EVM and Terra chains for withdrawal approval events, verifies each approval
//! against the source chain, and submits cancellation transactions for fraudulent
//! approvals (those without corresponding deposits).
//!
//! # V2 Architecture
//!
//! The watcher operates in a continuous polling loop:
//! 1. Polls EVM bridge for `WithdrawApprove` events in the current block range
//! 2. Polls Terra bridge for pending withdrawal approvals
//! 3. For each approval found, uses `ApprovalVerifier` to check if a matching deposit exists
//! 4. Submits cancellation transactions for any fraudulent approvals detected
//!
//! # EVM Event Filtering (V2)
//!
//! Uses the Alloy library to query `WithdrawApprove` events.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use crate::bounded_cache::BoundedHashCache;

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

// EVM bridge contract ABI for event queries (V2)
sol! {
    #[sol(rpc)]
    contract Bridge {
        /// V2 WithdrawApprove event
        event WithdrawApprove(
            bytes32 indexed xchainHashId
        );

        /// Get pending withdrawal info (matches IBridge.PendingWithdraw struct)
        function getPendingWithdraw(bytes32 xchainHashId) external view returns (
            bytes4 srcChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            address recipient,
            uint256 amount,
            uint64 nonce,
            uint256 operatorGas,
            uint256 submittedAt,
            uint256 approvedAt,
            bool approved,
            bool cancelled,
            bool executed
        );

        /// Get cancel window
        function getCancelWindow() external view returns (uint256);

        /// Get this chain's registered V2 chain ID (bytes4)
        function getThisChainId() external view returns (bytes4 chainId);
    }
}

/// Main watcher that monitors all chains for approvals (V2)
/// Per-chain polling state for multi-EVM monitoring
struct EvmChainPollState {
    /// V2 chain ID (4 bytes)
    v2_chain_id: [u8; 4],
    /// Chain name for logging
    name: String,
    /// RPC URL
    rpc_url: String,
    /// Bridge contract address
    bridge_address: String,
    /// Last polled block number
    last_block: u64,
}

pub struct CancelerWatcher {
    config: Config,
    verifier: ApprovalVerifier,
    evm_client: EvmClient,
    terra_client: TerraClient,
    /// Hashes we've already verified (C3: bounded)
    verified_hashes: BoundedHashCache,
    /// Hashes we've cancelled (C3: bounded)
    cancelled_hashes: BoundedHashCache,
    /// Last polled EVM block for the configured base EVM endpoint.
    last_evm_block: u64,
    /// Last polled Terra height
    last_terra_height: u64,
    /// This chain's 4-byte chain ID
    this_chain_id: [u8; 4],
    /// Other configured EVM chain peers to poll (multi-EVM support).
    additional_evm_chains: Vec<EvmChainPollState>,
    /// Shared stats for health endpoint
    stats: SharedStats,
    /// Prometheus metrics
    metrics: SharedMetrics,
    /// C4: Consecutive EVM can_cancel pre-check failures
    evm_precheck_consecutive_failures: AtomicU32,
    /// C4: Circuit breaker open — skip EVM cancel attempts until next success
    evm_precheck_circuit_open: AtomicBool,
}

impl CancelerWatcher {
    pub async fn new(config: &Config, stats: SharedStats, metrics: SharedMetrics) -> Result<Self> {
        // Resolve V2 chain IDs — required for correct operation.
        // Tries config first, then queries bridge contract. Fails if neither works.
        let (evm_v2, terra_v2) = Self::resolve_v2_chain_ids_required(config).await?;

        let mut verifier = ApprovalVerifier::new_v2(
            &config.evm_rpc_url,
            &config.evm_bridge_address,
            &config.terra_lcd_url,
            &config.terra_bridge_address,
            evm_v2,
            terra_v2,
        );

        // Register configured EVM chain peers from multi-EVM config for cross-chain verification.
        if let Some(ref multi) = config.multi_evm {
            let additional_chains: Vec<crate::verifier::KnownEvmChain> = multi
                .enabled_chains()
                .map(|chain| crate::verifier::KnownEvmChain {
                    v2_chain_id: chain.this_chain_id.0,
                    rpc_url: chain.rpc_url.clone(),
                    bridge_address: chain.bridge_address.clone(),
                })
                .collect();
            verifier.register_evm_chains(additional_chains);
        }

        // C6: Startup validation — cross-check resolved chain IDs against bridge contract.
        // If the configured V2 chain ID doesn't match what the bridge contract reports,
        // the canceler would either miss fraud or (worse) cancel valid approvals.
        if let Err(e) = Self::validate_chain_ids_against_bridge(config, &verifier).await {
            error!(
                error = %e,
                "CHAIN ID VALIDATION FAILED — the configured V2 chain IDs may not match \
                 the bridge contract's ChainRegistry. This can cause missed fraud detection \
                 or false-positive cancellations. Verify EVM_V2_CHAIN_ID and TERRA_V2_CHAIN_ID."
            );
            // Continue running (don't abort) — the error log is enough to trigger alerts
            // and the unknown-chain Pending logic (C6) prevents destructive misaction.
        }

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

        // Use the already-resolved V2 chain ID (guaranteed by resolve_v2_chain_ids_required)
        let this_chain_id = evm_v2;

        // Initialize stats with canceler ID
        {
            let mut s = stats.write().await;
            s.canceler_id = config.canceler_id.clone();
        }

        // Build chain poll states from multi-EVM config.
        // The configured base EVM endpoint is polled via poll_evm_approvals();
        // other EVM peers are polled via poll_additional_evm_approvals().
        let additional_evm_chains: Vec<EvmChainPollState> =
            if let Some(ref multi) = config.multi_evm {
                multi
                    .enabled_chains()
                    .filter(|c| c.this_chain_id.0 != this_chain_id) // Skip the configured base chain (already polled)
                    .map(|c| EvmChainPollState {
                        v2_chain_id: c.this_chain_id.0,
                        name: c.name.clone(),
                        rpc_url: c.rpc_url.clone(),
                        bridge_address: c.bridge_address.clone(),
                        last_block: 0,
                    })
                    .collect()
            } else {
                vec![]
            };

        info!(
            canceler_id = %config.canceler_id,
            evm_canceler = %evm_client.address(),
            terra_canceler = %terra_client.address,
            this_chain_id = %hex::encode(this_chain_id),
            additional_evm_chains = additional_evm_chains.len(),
            "Canceler watcher initialized (V2)"
        );

        Ok(Self {
            config: config.clone(),
            verifier,
            evm_client,
            terra_client,
            verified_hashes: BoundedHashCache::new(
                config.dedupe_cache_max_size,
                config.dedupe_cache_ttl_secs,
            ),
            cancelled_hashes: BoundedHashCache::new(
                config.dedupe_cache_max_size,
                config.dedupe_cache_ttl_secs,
            ),
            last_evm_block: 0,
            last_terra_height: 0,
            this_chain_id,
            additional_evm_chains,
            stats,
            metrics,
            evm_precheck_consecutive_failures: AtomicU32::new(0),
            evm_precheck_circuit_open: AtomicBool::new(false),
        })
    }

    /// C6: Validate that the verifier's chain IDs match what the bridge contract reports.
    ///
    /// Queries `getThisChainId()` from the EVM bridge and compares against the
    /// verifier's configured EVM V2 chain ID. A mismatch means the canceler is
    /// using wrong chain IDs, which could cause it to miss fraud or cancel valid
    /// approvals.
    async fn validate_chain_ids_against_bridge(
        config: &Config,
        verifier: &ApprovalVerifier,
    ) -> Result<()> {
        let bridge_chain_id = Self::query_bridge_this_chain_id(config).await?;
        let configured_evm_id = verifier.evm_chain_id();

        if bridge_chain_id != *configured_evm_id {
            return Err(eyre!(
                "EVM V2 chain ID MISMATCH: bridge contract reports 0x{} but canceler \
                 is configured with 0x{}. Set EVM_V2_CHAIN_ID=0x{} to fix.",
                hex::encode(bridge_chain_id),
                hex::encode(configured_evm_id),
                hex::encode(bridge_chain_id)
            ));
        }

        info!(
            bridge_chain_id = %hex::encode(bridge_chain_id),
            configured_evm_id = %hex::encode(configured_evm_id),
            "C6: EVM V2 chain ID validated against bridge contract"
        );

        Ok(())
    }

    /// Resolve V2 chain IDs: use config if set, otherwise query the EVM bridge contract.
    /// Returns an error if either chain ID cannot be resolved — no native-ID fallback.
    ///
    /// Resolution order for EVM:
    /// 1. `EVM_V2_CHAIN_ID` env var (explicit config — highest priority)
    /// 2. `getThisChainId()` query on the EVM bridge contract
    /// 3. Error (refuse to start)
    ///
    /// Resolution order for Terra:
    /// 1. `TERRA_V2_CHAIN_ID` env var (explicit config)
    /// 2. Error (refuse to start — no contract query implemented for Terra yet)
    async fn resolve_v2_chain_ids_required(config: &Config) -> Result<([u8; 4], [u8; 4])> {
        // --- EVM ---
        let evm_v2 = if let Some(id) = config.evm_v2_chain_id {
            info!(
                evm_v2_chain_id = %hex::encode(id),
                "Using EVM V2 chain ID from config (EVM_V2_CHAIN_ID)"
            );
            id
        } else {
            match Self::query_bridge_this_chain_id(config).await {
                Ok(id) => {
                    info!(
                        chain_id = %hex::encode(id),
                        "Queried EVM bridge getThisChainId() — using as EVM V2 chain ID"
                    );
                    id
                }
                Err(e) => {
                    return Err(eyre!(
                        "Cannot resolve EVM V2 chain ID: EVM_V2_CHAIN_ID not set and bridge \
                         query failed ({}). Set EVM_V2_CHAIN_ID to the bytes4 value from \
                         ChainRegistry (e.g., EVM_V2_CHAIN_ID=0x00000001).",
                        e
                    ));
                }
            }
        };

        // --- Terra ---
        let terra_v2 = if let Some(id) = config.terra_v2_chain_id {
            info!(
                terra_v2_chain_id = %hex::encode(id),
                "Using Terra V2 chain ID from config (TERRA_V2_CHAIN_ID)"
            );
            id
        } else {
            return Err(eyre!(
                "Cannot resolve Terra V2 chain ID: TERRA_V2_CHAIN_ID not set. \
                 Set TERRA_V2_CHAIN_ID to the bytes4 value from ChainRegistry \
                 (e.g., TERRA_V2_CHAIN_ID=0x00000002)."
            ));
        };

        Ok((evm_v2, terra_v2))
    }

    /// Query `getThisChainId()` from the EVM bridge contract to get its V2 chain ID.
    async fn query_bridge_this_chain_id(config: &Config) -> Result<[u8; 4]> {
        let provider = ProviderBuilder::new()
            .on_http(config.evm_rpc_url.parse().wrap_err("Invalid EVM RPC URL")?);

        let bridge_address =
            Address::from_str(&config.evm_bridge_address).wrap_err("Invalid EVM bridge address")?;

        let contract = Bridge::new(bridge_address, &provider);
        let result = contract
            .getThisChainId()
            .call()
            .await
            .map_err(|e| eyre!("getThisChainId() call failed: {}", e))?;

        Ok(result.chainId.0)
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

        // Poll the configured base EVM bridge for approvals.
        self.poll_evm_approvals().await?;

        // Poll other configured EVM peers (multi-EVM support).
        if !self.additional_evm_chains.is_empty() {
            self.poll_additional_evm_approvals().await;
        }

        // Poll Terra bridge for approvals
        self.poll_terra_approvals().await?;

        // C3: Update dedupe cache size gauges
        self.metrics
            .dedupe_verified_size
            .set(self.verified_hashes.len() as i64);
        self.metrics
            .dedupe_cancelled_size
            .set(self.cancelled_hashes.len() as i64);

        // C6: Update unknown source chain metric
        self.metrics
            .unknown_source_chain_total
            .set(self.verifier.unknown_source_chain_count() as i64);

        Ok(())
    }

    /// Poll EVM bridge for pending approvals (V2)
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
        if self.last_evm_block == 0 {
            self.last_evm_block = 0;
            info!(
                current_block = current_block,
                lookback_start = self.last_evm_block,
                "First poll - starting from genesis to catch all events"
            );
        }

        // Detect chain reset
        if current_block < self.last_evm_block {
            warn!(
                current_block = current_block,
                last_polled = self.last_evm_block,
                "Chain reset detected - resetting polling state to scan from genesis"
            );
            self.last_evm_block = 0;
            self.verified_hashes.clear();
            self.cancelled_hashes.clear();
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
            "Querying EVM WithdrawApprove events (V2)"
        );

        // Parse bridge address
        let bridge_address = Address::from_str(&self.config.evm_bridge_address)
            .wrap_err("Invalid EVM bridge address")?;

        // Query for WithdrawApprove events (V2)
        let contract = Bridge::new(bridge_address, &provider);

        let filter = contract
            .WithdrawApprove_filter()
            .address(bridge_address)
            .from_block(from_block)
            .to_block(to_block);

        // Log event topic for debugging
        let event_signature = "WithdrawApprove(bytes32)";
        let expected_topic = compute_event_topic(event_signature);
        debug!(
            bridge_address = %bridge_address,
            from_block = from_block,
            to_block = to_block,
            event_topic = %format!("0x{}", hex::encode(expected_topic)),
            "Querying WithdrawApprove events"
        );

        let logs = filter
            .query()
            .await
            .map_err(|e| eyre!("Failed to query events: {}", e))?;

        if !logs.is_empty() {
            info!(
                from_block = from_block,
                to_block = to_block,
                event_count = logs.len(),
                "Found EVM WithdrawApprove events - processing for fraud detection"
            );
        }

        // Process each approval event
        for (event, log) in logs {
            let xchain_hash_id: [u8; 32] = event.xchainHashId.0;

            // Skip if already processed
            if self.verified_hashes.contains(&xchain_hash_id)
                || self.cancelled_hashes.contains(&xchain_hash_id)
            {
                continue;
            }

            info!(
                xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                block = ?log.block_number,
                "Processing EVM approval event"
            );

            // Query withdrawal details from contract using getPendingWithdraw
            let withdrawal_info = contract
                .getPendingWithdraw(FixedBytes::from(xchain_hash_id))
                .call()
                .await;

            match withdrawal_info {
                Ok(info) => {
                    // Skip if already cancelled or executed
                    if info.cancelled {
                        debug!(xchain_hash_id = %bytes32_to_hex(&xchain_hash_id), "Already cancelled, skipping");
                        continue;
                    }
                    if info.executed {
                        debug!(xchain_hash_id = %bytes32_to_hex(&xchain_hash_id), "Already executed, skipping");
                        continue;
                    }

                    // Get cancel window
                    let cancel_window: u64 = contract
                        .getCancelWindow()
                        .call()
                        .await
                        .map(|w| {
                            let val: u64 = w._0.try_into().unwrap_or(300);
                            val
                        })
                        .unwrap_or(300);

                    // Convert token address to bytes32 (left-padded)
                    let mut token_bytes32 = [0u8; 32];
                    token_bytes32[12..32].copy_from_slice(info.token.as_slice());

                    // Create a pending approval for verification
                    let approval = PendingApproval {
                        xchain_hash_id,
                        src_chain_id: info.srcChain.0,
                        dest_chain_id: self.this_chain_id,
                        src_account: info.srcAccount.0,
                        dest_token: token_bytes32,
                        dest_account: info.destAccount.0,
                        amount: info.amount.try_into().unwrap_or_else(|_| {
                            warn!(
                                xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                                amount = %info.amount,
                                "Approval amount exceeds u128::MAX, clamping"
                            );
                            u128::MAX
                        }),
                        nonce: info.nonce,
                        approved_at_timestamp: info.approvedAt.try_into().unwrap_or(0),
                        cancel_window,
                    };

                    // Detailed diagnostic before verification
                    info!(
                        xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                        src_chain_id = %format!("0x{}", hex::encode(info.srcChain.0)),
                        dest_chain_id = %format!("0x{}", hex::encode(self.this_chain_id)),
                        nonce = approval.nonce,
                        amount = approval.amount,
                        cancel_window_secs = cancel_window,
                        approved_at = approval.approved_at_timestamp,
                        token = %format!("0x{}", hex::encode(info.token.as_slice())),
                        src_account = %format!("0x{}", hex::encode(&info.srcAccount.0[..8])),
                        dest_account = %format!("0x{}", hex::encode(&info.destAccount.0[..8])),
                        "Calling verify_and_cancel for EVM approval"
                    );

                    if let Err(e) = self.verify_and_cancel(&approval).await {
                        error!(
                            error = %e,
                            xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                            "Failed to verify approval"
                        );
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to get withdrawal info, skipping");
                    continue;
                }
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

    /// Poll configured EVM peers for WithdrawApprove events (multi-EVM).
    ///
    /// Collects pending approvals from all additional chains first, then
    /// processes them to avoid borrow-checker issues with `&mut self`.
    async fn poll_additional_evm_approvals(&mut self) {
        // Phase 1: Collect events from all additional chains
        let mut collected_approvals: Vec<PendingApproval> = Vec::new();
        let mut block_updates: Vec<(usize, u64)> = Vec::new();

        for (idx, chain_state) in self.additional_evm_chains.iter().enumerate() {
            let chain_name = &chain_state.name;
            let rpc_url = &chain_state.rpc_url;
            let bridge_addr_str = &chain_state.bridge_address;
            let chain_v2_id = chain_state.v2_chain_id;

            let provider = match rpc_url.parse() {
                Ok(url) => ProviderBuilder::new().on_http(url),
                Err(e) => {
                    warn!(chain = %chain_name, error = %e, "Invalid RPC URL for additional chain");
                    continue;
                }
            };

            let current_block = match provider.get_block_number().await {
                Ok(b) => b,
                Err(e) => {
                    warn!(chain = %chain_name, error = %e, "Failed to get block number");
                    continue;
                }
            };

            if current_block <= chain_state.last_block {
                continue;
            }

            let from_block =
                if chain_state.last_block == 0 || current_block < chain_state.last_block {
                    0
                } else {
                    chain_state.last_block + 1
                };
            let to_block = current_block;

            let bridge_address = match Address::from_str(bridge_addr_str) {
                Ok(a) => a,
                Err(e) => {
                    warn!(chain = %chain_name, error = %e, "Invalid bridge address");
                    continue;
                }
            };

            let contract = Bridge::new(bridge_address, &provider);
            let filter = contract
                .WithdrawApprove_filter()
                .address(bridge_address)
                .from_block(from_block)
                .to_block(to_block);

            let logs = match filter.query().await {
                Ok(l) => l,
                Err(e) => {
                    warn!(chain = %chain_name, error = %e, "Failed to query events");
                    continue;
                }
            };

            if !logs.is_empty() {
                info!(
                    chain = %chain_name,
                    v2_id = %hex::encode(chain_v2_id),
                    event_count = logs.len(),
                    from_block = from_block,
                    to_block = to_block,
                    "Found WithdrawApprove events on additional chain"
                );
            }

            for (event, _log) in &logs {
                let xchain_hash_id: [u8; 32] = event.xchainHashId.0;

                if self.verified_hashes.contains(&xchain_hash_id)
                    || self.cancelled_hashes.contains(&xchain_hash_id)
                {
                    continue;
                }

                // Get withdrawal details
                let info_result = contract
                    .getPendingWithdraw(FixedBytes::from(xchain_hash_id))
                    .call()
                    .await;

                match info_result {
                    Ok(info) => {
                        if info.cancelled || info.executed {
                            continue;
                        }

                        let cancel_window: u64 = contract
                            .getCancelWindow()
                            .call()
                            .await
                            .map(|w| w._0.try_into().unwrap_or(300))
                            .unwrap_or(300);

                        let mut token_bytes32 = [0u8; 32];
                        token_bytes32[12..32].copy_from_slice(info.token.as_slice());

                        collected_approvals.push(PendingApproval {
                            xchain_hash_id,
                            src_chain_id: info.srcChain.0,
                            dest_chain_id: chain_v2_id,
                            src_account: info.srcAccount.0,
                            dest_token: token_bytes32,
                            dest_account: info.destAccount.0,
                            amount: info.amount.try_into().unwrap_or(u128::MAX),
                            nonce: info.nonce,
                            approved_at_timestamp: info.approvedAt.try_into().unwrap_or(0),
                            cancel_window,
                        });
                    }
                    Err(e) => {
                        warn!(chain = %chain_name, error = %e, "Failed to get withdrawal info");
                    }
                }
            }

            block_updates.push((idx, to_block));
        }

        // Phase 2: Update block heights
        for (idx, block) in block_updates {
            self.additional_evm_chains[idx].last_block = block;
        }

        // Phase 3: Verify and cancel collected approvals
        for approval in collected_approvals {
            if let Err(e) = self.verify_and_cancel(&approval).await {
                error!(
                    error = %e,
                    xchain_hash_id = %bytes32_to_hex(&approval.xchain_hash_id),
                    "Failed to verify approval on additional chain"
                );
            }
        }
    }

    /// Poll Terra bridge for pending approvals (V2)
    async fn poll_terra_approvals(&mut self) -> Result<()> {
        debug!("Polling Terra approvals");

        // Query LCD for current height — use explicit timeout to avoid blocking
        // the poll loop if Terra LCD is unresponsive (security review C3)
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| eyre!("Failed to build HTTP client: {}", e))?;
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

        // Detect chain reset
        if current_height < self.last_terra_height {
            warn!(
                current_height = current_height,
                last_polled = self.last_terra_height,
                "Terra chain reset detected - resetting polling state"
            );
            self.last_terra_height = 0;
        }

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
            "Querying Terra pending approvals (V2)"
        );

        // C2: Paginate until exhaustion or page cap
        let page_size = self.config.terra_poll_page_size;
        let max_pages = self.config.terra_poll_max_pages;
        let mut all_approvals: Vec<PendingApproval> = Vec::new();
        let mut total_seen: u64 = 0;
        let mut start_after_b64: Option<String> = None;
        let mut pages_fetched: u32 = 0;
        let mut unprocessed: u64 = 0;
        let mut last_page_count: usize = 0;

        loop {
            if pages_fetched >= max_pages {
                if last_page_count >= page_size as usize {
                    unprocessed = page_size as u64;
                }
                warn!(
                    max_pages,
                    total_seen,
                    unprocessed,
                    "Terra pagination hit page cap; some approvals may be unprocessed"
                );
                break;
            }

            let mut query_obj = serde_json::json!({
                "pending_withdrawals": {
                    "limit": page_size
                }
            });
            if let Some(ref cursor) = start_after_b64 {
                query_obj["pending_withdrawals"]["start_after"] = serde_json::json!(cursor);
            }

            let query_b64 = base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_string(&query_obj)?);

            let url = format!(
                "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
                self.config.terra_lcd_url, self.config.terra_bridge_address, query_b64
            );

            let resp = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    warn!(error = %e, "Failed to query Terra withdrawals");
                    break;
                }
            };

            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| eyre!("Failed to parse withdrawals: {}", e))?;

            let withdrawals = match json["data"]["withdrawals"].as_array() {
                Some(arr) => arr,
                None => break,
            };

            let count = withdrawals.len();
            last_page_count = count;
            pages_fetched += 1;
            total_seen += count as u64;

            if count == 0 {
                break;
            }

            info!(
                page = pages_fetched,
                count, total_seen, "Fetched Terra pending withdrawals page"
            );

            let mut last_hash_b64: Option<String> = None;
            for withdrawal_json in withdrawals {
                let xchain_hash_id_b64 = withdrawal_json["xchain_hash_id"].as_str().unwrap_or("");
                last_hash_b64 = Some(xchain_hash_id_b64.to_string());

                let xchain_hash_id_bytes = base64::engine::general_purpose::STANDARD
                    .decode(xchain_hash_id_b64)
                    .unwrap_or_default();

                if xchain_hash_id_bytes.len() != 32 {
                    continue;
                }

                let mut xchain_hash_id = [0u8; 32];
                xchain_hash_id.copy_from_slice(&xchain_hash_id_bytes);

                if self.verified_hashes.contains(&xchain_hash_id)
                    || self.cancelled_hashes.contains(&xchain_hash_id)
                {
                    continue;
                }

                let src_chain_id = match self.parse_bytes4_from_json(&withdrawal_json["src_chain"])
                {
                    Some(id) => id,
                    None => {
                        warn!(
                            xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                            raw = ?withdrawal_json["src_chain"],
                            "Skipping: failed to parse src_chain bytes4"
                        );
                        continue;
                    }
                };
                let dest_chain_id =
                    match self.parse_bytes4_from_json(&withdrawal_json["dest_chain"]) {
                        Some(id) => id,
                        None => {
                            warn!(
                                xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                                raw = ?withdrawal_json["dest_chain"],
                                "Skipping: failed to parse dest_chain bytes4"
                            );
                            continue;
                        }
                    };
                let dest_token = self.parse_bytes32_from_json(&withdrawal_json["token"]);
                let src_account = self.parse_bytes32_from_json(&withdrawal_json["src_account"]);
                let dest_account = self.parse_bytes32_from_json(&withdrawal_json["dest_account"]);

                let amount: u128 = match withdrawal_json["amount"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                {
                    Some(a) => a,
                    None => {
                        warn!(xchain_hash_id = %bytes32_to_hex(&xchain_hash_id), "Skipping: failed to parse amount");
                        continue;
                    }
                };

                let nonce: u64 = match withdrawal_json["nonce"].as_u64() {
                    Some(n) => n,
                    None => {
                        warn!(xchain_hash_id = %bytes32_to_hex(&xchain_hash_id), "Skipping: failed to parse nonce");
                        continue;
                    }
                };

                let approved_at_timestamp: u64 =
                    withdrawal_json["approved_at"].as_u64().unwrap_or(0);

                let cancel_window: u64 = withdrawal_json["cancel_window"].as_u64().unwrap_or(300);

                let approval = PendingApproval {
                    xchain_hash_id,
                    src_chain_id,
                    dest_chain_id,
                    src_account,
                    dest_token,
                    dest_account,
                    amount,
                    nonce,
                    approved_at_timestamp,
                    cancel_window,
                };

                all_approvals.push(approval);
            }

            if count < page_size as usize {
                break; // Exhausted
            }

            start_after_b64 = last_hash_b64;
        }

        // C2: Update Terra queue metrics
        self.metrics
            .terra_pending_queue_depth
            .set(total_seen as i64);
        self.metrics
            .terra_unprocessed_approvals
            .set(unprocessed as i64);

        // C2: Sort by approved_at ascending (oldest first)
        all_approvals.sort_by_key(|a| a.approved_at_timestamp);

        for approval in &all_approvals {
            info!(
                xchain_hash_id = %bytes32_to_hex(&approval.xchain_hash_id),
                nonce = approval.nonce,
                amount = approval.amount,
                "Processing Terra withdrawal"
            );

            if let Err(e) = self.verify_and_cancel(approval).await {
                error!(
                    error = %e,
                    xchain_hash_id = %bytes32_to_hex(&approval.xchain_hash_id),
                    "Failed to verify Terra withdrawal"
                );
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

    /// Helper to parse bytes4 from JSON (base64 encoded).
    /// Returns None if the value is missing, empty, or cannot be decoded.
    fn parse_bytes4_from_json(&self, value: &serde_json::Value) -> Option<[u8; 4]> {
        let b64 = value.as_str()?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
        if bytes.len() < 4 {
            return None;
        }
        let mut result = [0u8; 4];
        result.copy_from_slice(&bytes[..4]);
        Some(result)
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
        if self.verified_hashes.contains(&approval.xchain_hash_id) {
            return Ok(());
        }
        if self.cancelled_hashes.contains(&approval.xchain_hash_id) {
            return Ok(());
        }

        info!(
            hash = %bytes32_to_hex(&approval.xchain_hash_id),
            src_chain = %hex::encode(approval.src_chain_id),
            dest_chain = %hex::encode(approval.dest_chain_id),
            nonce = approval.nonce,
            amount = approval.amount,
            "Verifying approval against source chain"
        );

        let result = self.verifier.verify(approval).await?;

        match result {
            VerificationResult::Valid => {
                info!(
                    hash = %bytes32_to_hex(&approval.xchain_hash_id),
                    nonce = approval.nonce,
                    "Approval verified as VALID — deposit found on source chain"
                );
                self.verified_hashes.insert(approval.xchain_hash_id);
                self.maybe_warn_dedupe_capacity("verified");

                // Update stats and metrics
                {
                    let mut stats = self.stats.write().await;
                    stats.verified_valid += 1;
                }
                self.metrics.verified_valid_total.inc();
            }
            VerificationResult::Invalid { reason } => {
                warn!(
                    hash = %bytes32_to_hex(&approval.xchain_hash_id),
                    reason = %reason,
                    nonce = approval.nonce,
                    src_chain = %hex::encode(approval.src_chain_id),
                    "FRAUD DETECTED — Approval is INVALID, submitting cancellation"
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
                        hash = %bytes32_to_hex(&approval.xchain_hash_id),
                        error = %e,
                        "Failed to submit cancellation"
                    );
                } else {
                    self.cancelled_hashes.insert(approval.xchain_hash_id);
                    self.maybe_warn_dedupe_capacity("cancelled");

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
                    hash = %bytes32_to_hex(&approval.xchain_hash_id),
                    "Verification pending - will retry"
                );
            }
        }

        Ok(())
    }

    /// Submit cancel transaction to the appropriate chain (C4: EVM pre-check safety)
    async fn submit_cancel(&self, approval: &PendingApproval) -> Result<()> {
        let xchain_hash_id = approval.xchain_hash_id;
        let dest_chain = approval.dest_chain_id;

        info!(
            hash = %bytes32_to_hex(&xchain_hash_id),
            dest_chain = %hex::encode(dest_chain),
            "Attempting to submit cancellation transaction"
        );

        // Route EVM cancellation by destination V2 chain ID.
        // Base configured EVM chain uses the shared client + circuit breaker.
        // Peer EVM chains use an on-demand client bound to that chain's RPC/bridge.
        if dest_chain == self.this_chain_id {
            // C4: If circuit breaker is open, skip EVM cancel attempts
            if self.evm_precheck_circuit_open.load(Ordering::Relaxed) {
                debug!(
                    hash = %bytes32_to_hex(&xchain_hash_id),
                    "EVM pre-check circuit breaker is OPEN — skipping EVM cancel path"
                );
            } else {
                // C4: Retry can_cancel with exponential backoff; on error set can_cancel_evm = false
                let mut can_cancel_evm = false;
                let mut last_err = None;
                for attempt in 0..=self.config.evm_precheck_max_retries {
                    match self.evm_client.can_cancel(xchain_hash_id).await {
                        Ok(can) => {
                            can_cancel_evm = can;
                            self.evm_precheck_consecutive_failures
                                .store(0, Ordering::Relaxed);
                            if self
                                .evm_precheck_circuit_open
                                .swap(false, Ordering::Relaxed)
                            {
                                info!(
                                    hash = %bytes32_to_hex(&xchain_hash_id),
                                    "EVM pre-check circuit breaker CLOSED"
                                );
                            }
                            debug!(
                                hash = %bytes32_to_hex(&xchain_hash_id),
                                can_cancel = can,
                                "Checked EVM can_cancel status on base configured chain"
                            );
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e);
                            if attempt < self.config.evm_precheck_max_retries {
                                let delay_ms = 500 * 2u64.pow(attempt);
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            }
                        }
                    }
                }

                if let Some(e) = last_err {
                    let prev = self
                        .evm_precheck_consecutive_failures
                        .fetch_add(1, Ordering::Relaxed);
                    let count = prev + 1;
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        consecutive_failures = count,
                        "EVM can_cancel pre-check failed; skipping cancel attempt this cycle (will retry)"
                    );
                    if count >= self.config.evm_precheck_circuit_breaker_threshold {
                        self.evm_precheck_circuit_open
                            .store(true, Ordering::Relaxed);
                        self.metrics.evm_precheck_circuit_breaker_trips_total.inc();
                        error!(
                            hash = %bytes32_to_hex(&xchain_hash_id),
                            threshold = self.config.evm_precheck_circuit_breaker_threshold,
                            "EVM pre-check circuit breaker OPEN — skipping all EVM cancel attempts \
                             until a successful pre-check"
                        );
                    }
                }

                if can_cancel_evm {
                    info!(
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        canceler_address = %self.evm_client.address(),
                        "Submitting withdrawCancel transaction to base configured EVM chain"
                    );

                    match self
                        .evm_client
                        .cancel_withdraw_approval(xchain_hash_id)
                        .await
                    {
                        Ok(tx_hash) => {
                            info!(
                                tx_hash = %tx_hash,
                                hash = %bytes32_to_hex(&xchain_hash_id),
                                "EVM cancellation transaction SUCCEEDED"
                            );
                            return Ok(());
                        }
                        Err(e) => {
                            warn!(
                                error = %e,
                                hash = %bytes32_to_hex(&xchain_hash_id),
                                canceler_address = %self.evm_client.address(),
                                "EVM cancellation FAILED - check if canceler has CANCELER_ROLE"
                            );
                        }
                    }
                }
            }
        } else if let Some(peer) = self
            .additional_evm_chains
            .iter()
            .find(|c| c.v2_chain_id == dest_chain)
        {
            let peer_client = EvmClient::new(
                &peer.rpc_url,
                &peer.bridge_address,
                &self.config.evm_private_key,
            )?;
            if peer_client
                .can_cancel(xchain_hash_id)
                .await
                .unwrap_or(false)
            {
                info!(
                    hash = %bytes32_to_hex(&xchain_hash_id),
                    chain = %peer.name,
                    "Submitting withdrawCancel transaction to EVM peer chain"
                );
                match peer_client.cancel_withdraw_approval(xchain_hash_id).await {
                    Ok(tx_hash) => {
                        info!(
                            tx_hash = %tx_hash,
                            hash = %bytes32_to_hex(&xchain_hash_id),
                            chain = %peer.name,
                            "EVM peer-chain cancellation transaction SUCCEEDED"
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            hash = %bytes32_to_hex(&xchain_hash_id),
                            chain = %peer.name,
                            "EVM peer-chain cancellation FAILED"
                        );
                    }
                }
            }
        }

        // Try Terra
        if self
            .terra_client
            .can_cancel(xchain_hash_id)
            .await
            .unwrap_or(false)
        {
            info!(
                hash = %bytes32_to_hex(&xchain_hash_id),
                "Submitting cancellation to Terra"
            );

            let tx_hash = self
                .terra_client
                .cancel_withdraw_approval(xchain_hash_id)
                .await?;
            info!(
                tx_hash = %tx_hash,
                hash = %bytes32_to_hex(&xchain_hash_id),
                "Terra cancellation submitted"
            );
            return Ok(());
        }

        warn!(
            hash = %bytes32_to_hex(&xchain_hash_id),
            "Could not submit cancellation to any chain"
        );

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> (usize, usize) {
        (self.verified_hashes.len(), self.cancelled_hashes.len())
    }

    /// C3: Warn when dedupe cache reaches 80% capacity
    fn maybe_warn_dedupe_capacity(&self, which: &str) {
        let (len, max) = match which {
            "verified" => self.verified_hashes.capacity_info(),
            "cancelled" => self.cancelled_hashes.capacity_info(),
            _ => return,
        };
        if max > 0 && len * 100 / max >= 80 {
            warn!(
                cache = %which,
                len,
                max,
                "Dedupe cache at or above 80% capacity"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::verifier::PendingApproval;

    /// C2: Terra approvals are processed oldest-first by approved_at_timestamp
    #[test]
    fn test_terra_approval_sort_order() {
        let make_approval = |approved_at: u64, nonce: u64| PendingApproval {
            xchain_hash_id: [nonce as u8; 32],
            src_chain_id: [0, 0, 0, 1],
            dest_chain_id: [0, 0, 0, 2],
            src_account: [0u8; 32],
            dest_account: [0u8; 32],
            dest_token: [0u8; 32],
            amount: 1000,
            nonce,
            approved_at_timestamp: approved_at,
            cancel_window: 300,
        };

        let mut approvals = vec![
            make_approval(2000, 3),
            make_approval(1000, 1),
            make_approval(1500, 2),
        ];
        approvals.sort_by_key(|a| a.approved_at_timestamp);

        assert_eq!(approvals[0].approved_at_timestamp, 1000);
        assert_eq!(approvals[0].nonce, 1);
        assert_eq!(approvals[1].approved_at_timestamp, 1500);
        assert_eq!(approvals[2].approved_at_timestamp, 2000);
    }
}
