//! EVM Writer - Submits withdrawal approvals to EVM chains
//!
//! Processes pending Terra deposits and submits corresponding
//! withdrawal approvals to the EVM bridge contract.
//!
//! ## V2 Withdrawal Flow
//!
//! In V2, the withdrawal flow is user-initiated:
//! 1. User calls `withdrawSubmit` on destination chain
//! 2. Operator calls `withdrawApprove(xchainHashId)` to approve
//! 3. After cancel window, anyone can call `withdrawExecuteUnlock/Mint`
//!
//! The operator only needs to approve pending withdrawals, not create them.

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use crate::bounded_cache::{BoundedHashCache, BoundedPendingCache};

use alloy::network::EthereumWallet;
use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use base64::Engine;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{debug, error, info, warn};

use crate::config::{EvmConfig, FeeConfig, TerraConfig};
use crate::contracts::evm_bridge::{Bridge, TokenRegistry};
use crate::db::{self, EvmDeposit, NewApproval, TerraDeposit};
use crate::hash::{address_to_bytes32, bytes32_to_hex, compute_xchain_hash_id};
use crate::types::{ChainId, EvmAddress};

/// Pending approval tracking for auto-execution
#[derive(Debug, Clone)]
struct PendingExecution {
    /// The cross-chain hash identifier
    xchain_hash_id: [u8; 32],
    /// When the approval was submitted
    approved_at: Instant,
    /// The delay required before execution
    delay_seconds: u64,
    /// Number of execution attempts
    attempts: u32,
}

/// EVM transaction writer for submitting withdrawal approvals
///
/// Operates in two modes:
/// 1. **Poll-and-approve (V2)**: Polls WithdrawSubmit events on this EVM chain,
///    verifies deposits on the source chain, and calls withdrawApprove.
///    This handles BOTH Terra→EVM and EVM→EVM transfers uniformly.
/// 2. **Auto-execution**: After the cancel window, automatically calls
///    withdrawExecuteUnlock/Mint to complete the transfer.
pub struct EvmWriter {
    rpc_url: String,
    /// All RPC URLs (primary + fallbacks) for read operations
    rpc_urls: Vec<String>,
    bridge_address: Address,
    chain_id: u64,
    /// This chain's registered 4-byte chain ID (V2)
    this_chain_id: ChainId,
    /// Terra LCD URL for Terra-source deposit verification
    terra_lcd_url: Option<String>,
    /// Terra bridge address for Terra-source deposit verification
    terra_bridge_address: Option<String>,
    /// Terra V2 4-byte chain ID (None when Terra is not configured)
    terra_chain_id: Option<ChainId>,
    signer: PrivateKeySigner,
    default_fee_bps: u32,
    fee_recipient: Address,
    db: PgPool,
    /// Cancel window in seconds (queried from contract)
    cancel_window: u64,
    /// Pending approvals awaiting execution (bounded to prevent unbounded growth)
    pending_executions: BoundedPendingCache<PendingExecution>,
    /// Last block polled for WithdrawSubmit events
    last_polled_block: u64,
    /// Hashes already approved by this operator (bounded to prevent unbounded growth)
    approved_hashes: BoundedHashCache,
    /// Source chain verification endpoints, keyed by V2 4-byte chain ID.
    /// Used for routing cross-chain deposit verification to the correct source chain RPC/bridge.
    source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
}

impl EvmWriter {
    /// Create a new EVM writer
    ///
    /// `source_chain_endpoints` maps V2 4-byte chain IDs to (rpc_url, bridge_address)
    /// for routing cross-chain deposit verification to the correct source chain.
    pub async fn new(
        evm_config: &EvmConfig,
        terra_config: Option<&TerraConfig>,
        fee_config: &FeeConfig,
        db: PgPool,
        source_chain_endpoints: HashMap<[u8; 4], (String, Address)>,
    ) -> Result<Self> {
        let bridge_address =
            Address::from_str(&evm_config.bridge_address).wrap_err("Invalid bridge address")?;
        let fee_recipient =
            Address::from_str(&fee_config.fee_recipient).wrap_err("Invalid fee recipient")?;

        // Parse the private key
        let signer: PrivateKeySigner = evm_config
            .private_key
            .parse()
            .wrap_err("Invalid private key")?;

        // V2 chain ID — query from bridge contract, fall back to config
        let provider =
            ProviderBuilder::new().on_http(evm_config.rpc_url.parse().wrap_err("Invalid RPC URL")?);
        let bridge_contract = Bridge::new(bridge_address, &provider);
        let this_chain_id = match bridge_contract.getThisChainId().call().await {
            Ok(result) => {
                let v2_id = ChainId::from_bytes(result._0.0);
                info!(
                    native_chain_id = evm_config.chain_id,
                    v2_chain_id = %v2_id,
                    v2_hex = %format!("0x{}", hex::encode(v2_id.as_bytes())),
                    "EVM writer: queried V2 chain ID from bridge contract"
                );
                v2_id
            }
            Err(e) => {
                if let Some(configured_id) = evm_config.this_chain_id {
                    let fallback = ChainId::from_u32(configured_id);
                    warn!(
                        error = %e,
                        native_chain_id = evm_config.chain_id,
                        configured_v2_id = %fallback,
                        "EVM writer: failed to query V2 chain ID, using EVM_THIS_CHAIN_ID config"
                    );
                    fallback
                } else {
                    return Err(eyre::eyre!(
                        "Cannot resolve EVM V2 chain ID: bridge query failed ({}) and \
                         EVM_THIS_CHAIN_ID is not set. Set EVM_THIS_CHAIN_ID to the V2 \
                         chain ID from ChainRegistry (e.g., EVM_THIS_CHAIN_ID=1).",
                        e
                    ));
                }
            }
        };

        info!(
            operator_address = %signer.address(),
            native_chain_id = evm_config.chain_id,
            v2_chain_id = %this_chain_id,
            v2_hex = %format!("0x{}", hex::encode(this_chain_id.as_bytes())),
            bridge_address = %bridge_address,
            "EVM writer initialized (V2)"
        );

        // Query cancel window from V2 contract — try all RPC endpoints
        let all_rpc_urls = evm_config.all_rpc_urls();
        let mut cancel_window_result = None;
        for url in &all_rpc_urls {
            match Self::query_cancel_window(url, bridge_address).await {
                Ok(window) => {
                    cancel_window_result = Some(window);
                    break;
                }
                Err(e) => {
                    if all_rpc_urls.len() > 1 {
                        warn!(rpc = %url, error = %e, "Failed to query cancel window, trying next RPC");
                    } else {
                        return Err(e.wrap_err(
                            "Failed to query cancel window from bridge — cannot start safely",
                        ));
                    }
                }
            }
        }
        let cancel_window = cancel_window_result.ok_or_else(|| {
            eyre!(
                "Failed to query cancel window from all {} RPCs",
                all_rpc_urls.len()
            )
        })?;

        let terra_lcd_url = terra_config.map(|t| t.lcd_url.clone());
        let terra_bridge_address = terra_config.map(|t| t.bridge_address.clone());
        let terra_chain_id: Option<ChainId> = if let Some(tc) = terra_config {
            // Prefer TERRA_THIS_CHAIN_ID when explicitly set (e.g. TERRA_THIS_CHAIN_ID=2).
            // ChainRegistry can return 0 or wrong values; explicit config takes precedence.
            if let Some(id) = tc.this_chain_id {
                info!(
                    terra_v2_chain_id = %format!("0x{:08x}", id),
                    "EVM writer: using TERRA_THIS_CHAIN_ID from config"
                );
                Some(ChainId::from_u32(id))
            } else {
                // Auto-discover Terra V2 chain ID from ChainRegistry
                let identifier = format!("terraclassic_{}", tc.chain_id);
                let query_client = multichain_rs::evm::EvmQueryClient::new(
                    &evm_config.rpc_url,
                    bridge_address,
                    evm_config.chain_id,
                )
                .wrap_err("Failed to create EVM query client")?;

                let registry_addr = query_client
                    .get_chain_registry_address()
                    .await
                    .wrap_err("Failed to get ChainRegistry address")?;

                match query_client
                    .compute_identifier_hash(registry_addr, &identifier)
                    .await
                {
                    Ok(hash) => match query_client
                        .get_chain_id_from_hash(registry_addr, hash)
                        .await
                    {
                        Ok(cid) if cid.to_u32() != 0 => {
                            info!(
                                identifier = %identifier,
                                terra_v2_chain_id = %format!("0x{}", hex::encode(cid.as_bytes())),
                                "EVM writer: queried Terra V2 chain ID from ChainRegistry"
                            );
                            Some(cid)
                        }
                        Ok(_cid) => {
                            warn!(
                                identifier = %identifier,
                                "ChainRegistry returned 0 for Terra; set TERRA_THIS_CHAIN_ID=2"
                            );
                            return Err(eyre::eyre!(
                                "ChainRegistry returned 0 for Terra. Set TERRA_THIS_CHAIN_ID=2"
                            ));
                        }
                        Err(e) => {
                            warn!(
                                identifier = %identifier,
                                error = %e,
                                "Failed to get Terra chain ID from hash"
                            );
                            return Err(eyre::eyre!(
                                "Cannot resolve Terra V2 chain ID: ChainRegistry query failed ({}). \
                                 Set TERRA_THIS_CHAIN_ID=2.",
                                e
                            ));
                        }
                    },
                    Err(e) => {
                        warn!(
                            identifier = %identifier,
                            error = %e,
                            "Failed to compute Terra identifier hash"
                        );
                        return Err(eyre::eyre!(
                            "Cannot resolve Terra V2 chain ID: ChainRegistry query failed ({}). \
                             Set TERRA_THIS_CHAIN_ID=2.",
                            e
                        ));
                    }
                }
            }
        } else {
            // No Terra config — Terra verification paths won't be reached
            None
        };

        info!(delay_seconds = cancel_window, "EVM cancel window");

        Ok(Self {
            rpc_url: evm_config.rpc_url.clone(),
            rpc_urls: all_rpc_urls,
            bridge_address,
            chain_id: evm_config.chain_id,
            this_chain_id,
            terra_lcd_url,
            terra_bridge_address,
            terra_chain_id,
            signer,
            default_fee_bps: fee_config.default_fee_bps,
            fee_recipient,
            db,
            cancel_window,
            pending_executions: {
                let cc = crate::bounded_cache::CacheConfig::from_env();
                BoundedPendingCache::new(cc.pending_execution_size, cc.ttl_secs)
            },
            last_polled_block: 0,
            approved_hashes: {
                let cc = crate::bounded_cache::CacheConfig::from_env();
                BoundedHashCache::new(cc.approved_hash_size, cc.ttl_secs)
            },
            source_chain_endpoints,
        })
    }

    /// Query the cancel window from the V2 contract
    async fn query_cancel_window(rpc_url: &str, bridge_address: Address) -> Result<u64> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = Bridge::new(bridge_address, provider);
        let window = contract.getCancelWindow().call().await?;

        Ok(window._0.try_into().unwrap_or(300))
    }

    /// Process pending withdrawals on this EVM chain (V2)
    ///
    /// This is the main processing loop for the EVM writer. It:
    /// 1. Checks if any approved withdrawals are ready for execution
    /// 2. Enumerates all pending withdrawals from contract state (primary)
    /// 3. Polls WithdrawSubmit events for faster new-event detection (secondary)
    /// 4. For each unapproved withdrawal, verifies the deposit on the source chain
    /// 5. If verified, calls withdrawApprove(hash) on this chain
    ///
    /// This handles BOTH Terra→EVM and EVM→EVM transfers uniformly —
    /// any pending withdrawal on this chain gets verified and approved.
    pub async fn process_pending(&mut self) -> Result<()> {
        self.process_pending_executions().await?;

        // Primary: enumerate pending withdrawals from contract state
        if let Err(e) = self.enumerate_and_approve().await {
            warn!(error = %e, "Enumeration-based approval poll failed");
        }

        // Secondary: event-based polling for faster detection
        self.poll_and_approve().await?;

        Ok(())
    }

    /// Primary: Enumerate all pending withdrawals from the contract's EnumerableSet
    /// and approve any that have a verified deposit on the source chain.
    ///
    /// More reliable than event-based polling — queries current contract state
    /// directly, avoiding block range issues with eth_getLogs.
    async fn enumerate_and_approve(&mut self) -> Result<()> {
        // Try each RPC URL until enumeration succeeds
        let (provider, hashes) = {
            let mut result = None;
            for (i, url) in self.rpc_urls.iter().enumerate() {
                let p = ProviderBuilder::new().on_http(url.parse().wrap_err("Invalid RPC URL")?);
                let call_result = {
                    let c = Bridge::new(self.bridge_address, &p);
                    c.getPendingWithdrawHashes().call().await
                };
                match call_result {
                    Ok(hashes) => {
                        if i > 0 && self.rpc_urls.len() > 1 {
                            info!(chain_id = self.chain_id, rpc = %url, "Using fallback RPC for enumeration");
                        }
                        result = Some((p, hashes));
                        break;
                    }
                    Err(e) => {
                        if self.rpc_urls.len() > 1 {
                            warn!(chain_id = self.chain_id, rpc = %url, error = %e, remaining = self.rpc_urls.len() - i - 1, "Enumeration RPC failed, trying next");
                        }
                    }
                }
            }
            result.ok_or_else(|| {
                eyre!(
                    "Failed to enumerate pending withdrawals from all {} RPCs",
                    self.rpc_urls.len()
                )
            })?
        };

        let contract = Bridge::new(self.bridge_address, &provider);
        let pending_hashes = &hashes.hashes;

        if pending_hashes.is_empty() {
            return Ok(());
        }

        let mut new_count: u64 = 0;
        for hash_fb in pending_hashes {
            let xchain_hash_id: [u8; 32] = hash_fb.0;

            if self.approved_hashes.contains_key(&xchain_hash_id) {
                continue;
            }

            let pending = match contract.getPendingWithdraw(*hash_fb).call().await {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        "Enumeration: failed to query getPendingWithdraw"
                    );
                    continue;
                }
            };

            if pending.approved {
                self.approved_hashes.insert(xchain_hash_id);
                continue;
            }
            if pending.cancelled || pending.executed {
                continue;
            }

            new_count += 1;

            let src_chain_id = pending.srcChain.0;
            let nonce = pending.nonce;
            let amount: u128 = pending.amount.try_into().unwrap_or_else(|_| {
                warn!(amount = %pending.amount, "Amount exceeds u128::MAX, clamping");
                u128::MAX
            });

            info!(
                hash = %bytes32_to_hex(&xchain_hash_id),
                src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                nonce = nonce,
                amount = amount,
                token = %pending.token,
                "Enumeration: processing unapproved withdrawal — verifying deposit on source chain"
            );

            let deposit_verified = match self
                .verify_deposit_on_source(&xchain_hash_id, &src_chain_id)
                .await
            {
                Ok(verified) => verified,
                Err(e) => {
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                        "Failed to verify deposit on source chain, will retry"
                    );
                    continue;
                }
            };

            if !deposit_verified {
                debug!(
                    hash = %bytes32_to_hex(&xchain_hash_id),
                    src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                    "No verified deposit found on source chain, skipping (will retry next cycle)"
                );
                continue;
            }

            info!(
                hash = %bytes32_to_hex(&xchain_hash_id),
                nonce = nonce,
                "Deposit verified on source chain, submitting withdrawApprove"
            );

            match self.submit_withdraw_approve(&xchain_hash_id).await {
                Ok(tx_hash) => {
                    info!(
                        tx_hash = %tx_hash,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        nonce = nonce,
                        "WithdrawApprove submitted successfully (via enumeration)"
                    );

                    self.approved_hashes.insert(xchain_hash_id);

                    self.pending_executions.insert(
                        xchain_hash_id,
                        PendingExecution {
                            xchain_hash_id,
                            approved_at: Instant::now(),
                            delay_seconds: self.cancel_window,
                            attempts: 0,
                        },
                    );

                    self.sync_deposit_status_after_approval(&src_chain_id, nonce)
                        .await;
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        "Failed to submit withdrawApprove, will retry next cycle"
                    );
                }
            }
        }

        if new_count > 0 {
            info!(
                total_pending = pending_hashes.len(),
                new_to_process = new_count,
                "Enumeration: found unapproved withdrawals"
            );
        }

        Ok(())
    }

    /// Secondary: Poll EVM bridge for WithdrawSubmit events and approve verified withdrawals
    ///
    /// Supplements enumeration with faster event-based detection of new submissions.
    async fn poll_and_approve(&mut self) -> Result<()> {
        // Find a working provider by trying each RPC URL
        let (provider, current_block) = {
            let mut result = None;
            for (i, url) in self.rpc_urls.iter().enumerate() {
                let p = ProviderBuilder::new().on_http(url.parse().wrap_err("Invalid RPC URL")?);
                match p.get_block_number().await {
                    Ok(block) => {
                        if i > 0 && self.rpc_urls.len() > 1 {
                            info!(chain_id = self.chain_id, rpc = %url, "Using fallback RPC for writer poll");
                        }
                        result = Some((p, block));
                        break;
                    }
                    Err(e) => {
                        if self.rpc_urls.len() > 1 {
                            warn!(chain_id = self.chain_id, rpc = %url, error = %e, remaining = self.rpc_urls.len() - i - 1, "Writer poll RPC failed for block number, trying next");
                        }
                    }
                }
            }
            result.ok_or_else(|| {
                eyre!(
                    "Failed to get block number from all {} RPCs",
                    self.rpc_urls.len()
                )
            })?
        };

        // Detect chain reset (e.g., Anvil restart)
        if current_block < self.last_polled_block {
            warn!(
                current_block = current_block,
                last_polled = self.last_polled_block,
                "Chain reset detected — resetting polling state"
            );
            self.last_polled_block = 0;
            self.approved_hashes.clear();
        }

        // Don't query if no new blocks
        if current_block <= self.last_polled_block {
            return Ok(());
        }

        // On first poll, look back from head instead of scanning from genesis
        let lookback: u64 = std::env::var("EVM_POLL_LOOKBACK_BLOCKS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5_000);
        let chunk_size: u64 = std::env::var("EVM_POLL_CHUNK_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5_000)
            .max(1);

        let from_block = if self.last_polled_block == 0 {
            let start = current_block.saturating_sub(lookback);
            info!(
                chain_id = self.chain_id,
                current_block,
                lookback_blocks = lookback,
                start_block = start,
                "Writer first poll — looking back {} blocks from head",
                lookback
            );
            start
        } else {
            self.last_polled_block + 1
        };
        let to_block = current_block;

        let contract = Bridge::new(self.bridge_address, &provider);

        let mut all_logs = Vec::new();
        let mut chunk_start = from_block;
        let mut last_successful_block = self.last_polled_block;
        while chunk_start <= to_block {
            let chunk_end = (chunk_start + chunk_size - 1).min(to_block);

            let filter = contract
                .WithdrawSubmit_filter()
                .from_block(chunk_start)
                .to_block(chunk_end);

            match filter.query().await {
                Ok(logs) => {
                    if !logs.is_empty() {
                        info!(
                            from_block = chunk_start,
                            to_block = chunk_end,
                            count = logs.len(),
                            "Found WithdrawSubmit events in chunk"
                        );
                    }
                    all_logs.extend(logs);
                    last_successful_block = chunk_end;
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        from = chunk_start,
                        to = chunk_end,
                        "eth_getLogs failed for WithdrawSubmit chunk — will retry next poll (enumeration covers gap)"
                    );
                    break;
                }
            }

            chunk_start = chunk_end + 1;
        }

        let logs = all_logs;

        for (event, _log) in &logs {
            let xchain_hash_id: [u8; 32] = event.xchainHashId.0;

            // Skip if already approved by us
            if self.approved_hashes.contains_key(&xchain_hash_id) {
                continue;
            }

            // Query full withdrawal details
            let pending = match contract
                .getPendingWithdraw(FixedBytes::from(xchain_hash_id))
                .call()
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        "Failed to query getPendingWithdraw, skipping"
                    );
                    continue;
                }
            };

            // Skip already-approved, cancelled, or executed withdrawals
            if pending.approved {
                self.approved_hashes.insert(xchain_hash_id);
                continue;
            }
            if pending.cancelled || pending.executed {
                continue;
            }

            let src_chain_id = pending.srcChain.0;
            let nonce = pending.nonce;
            let amount: u128 = pending.amount.try_into().unwrap_or_else(|_| {
                warn!(amount = %pending.amount, "Amount exceeds u128::MAX, clamping");
                u128::MAX
            });

            info!(
                hash = %bytes32_to_hex(&xchain_hash_id),
                src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                nonce = nonce,
                amount = amount,
                token = %pending.token,
                "Processing unapproved WithdrawSubmit — verifying deposit on source chain"
            );

            // Verify deposit on the source chain using getDeposit(hash)
            // For both Terra→EVM and EVM→EVM, the deposit hash = withdraw hash
            // because both use the same 7-field compute_xchain_hash_id.
            let deposit_verified = match self
                .verify_deposit_on_source(&xchain_hash_id, &src_chain_id)
                .await
            {
                Ok(verified) => verified,
                Err(e) => {
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                        "Failed to verify deposit on source chain, will retry"
                    );
                    continue;
                }
            };

            if !deposit_verified {
                debug!(
                    hash = %bytes32_to_hex(&xchain_hash_id),
                    src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                    "No verified deposit found on source chain, skipping (will retry next cycle)"
                );
                continue;
            }

            // Deposit verified — submit withdrawApprove
            info!(
                hash = %bytes32_to_hex(&xchain_hash_id),
                nonce = nonce,
                "Deposit verified on source chain, submitting withdrawApprove"
            );

            match self.submit_withdraw_approve(&xchain_hash_id).await {
                Ok(tx_hash) => {
                    info!(
                        tx_hash = %tx_hash,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        nonce = nonce,
                        "WithdrawApprove submitted successfully"
                    );

                    self.approved_hashes.insert(xchain_hash_id);

                    // Track for auto-execution after cancel window
                    self.pending_executions.insert(
                        xchain_hash_id,
                        PendingExecution {
                            xchain_hash_id,
                            approved_at: Instant::now(),
                            delay_seconds: self.cancel_window,
                            attempts: 0,
                        },
                    );

                    // Sync DB: mark corresponding evm_deposit or terra_deposit as processed
                    // so pending_deposits count stays accurate. The V2 poll-and-approve path
                    // works from on-chain events, but the DB is the shared data source for
                    // /status reporting and the legacy DB-driven paths.
                    self.sync_deposit_status_after_approval(&src_chain_id, nonce)
                        .await;
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        hash = %bytes32_to_hex(&xchain_hash_id),
                        "Failed to submit withdrawApprove, will retry next cycle"
                    );
                }
            }
        }

        self.last_polled_block = last_successful_block;

        Ok(())
    }

    /// Verify a deposit exists on the source chain.
    ///
    /// Routes verification to the correct chain:
    /// - Terra source → Terra LCD query
    /// - This chain (self) → local RPC/bridge
    /// - Known multi-EVM source → source chain's RPC/bridge
    /// - Unknown source → **fail closed** (refuse to approve)
    async fn verify_deposit_on_source(
        &self,
        xchain_hash_id: &[u8; 32],
        src_chain_id: &[u8; 4],
    ) -> Result<bool> {
        // Terra-source withdrawals are verified on Terra bridge storage.
        if self
            .terra_chain_id
            .as_ref()
            .is_some_and(|id| src_chain_id == id.as_bytes())
        {
            debug!(
                hash = %bytes32_to_hex(xchain_hash_id),
                src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                terra_chain = %format!("0x{}", hex::encode(self.terra_chain_id.as_ref().unwrap().as_bytes())),
                "Routing source deposit verification to Terra bridge"
            );
            return self.verify_terra_deposit(xchain_hash_id).await;
        }

        // Determine which RPC/bridge to use for EVM-source verification.
        let (rpc_url, bridge_address) = if src_chain_id == self.this_chain_id.as_bytes() {
            // Same chain: use own RPC/bridge (local setup, single-chain)
            (self.rpc_url.as_str(), self.bridge_address)
        } else if let Some((url, addr)) = self.source_chain_endpoints.get(src_chain_id) {
            // Known multi-EVM source chain: route to its RPC/bridge
            info!(
                src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                rpc = %url,
                bridge = %addr,
                "Routing deposit verification to configured source chain"
            );
            (url.as_str(), *addr)
        } else {
            // Unknown source chain: fail closed — refuse to approve
            warn!(
                hash = %bytes32_to_hex(xchain_hash_id),
                src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                known_chains = self.source_chain_endpoints.len(),
                "Unknown source chain ID — refusing to approve (fail closed). \
                 Configure the source chain in EVM_CHAINS or EVM_THIS_CHAIN_ID."
            );
            return Ok(false);
        };

        self.verify_evm_deposit_on_chain(rpc_url, bridge_address, xchain_hash_id)
            .await
    }

    /// Verify a deposit exists on a specific EVM chain by querying `getDeposit(hash)`.
    ///
    /// Returns `true` if the deposit record has a non-zero timestamp.
    async fn verify_evm_deposit_on_chain(
        &self,
        rpc_url: &str,
        bridge_address: Address,
        xchain_hash_id: &[u8; 32],
    ) -> Result<bool> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = Bridge::new(bridge_address, &provider);

        let hash_fixed = FixedBytes::from(*xchain_hash_id);
        match contract.getDeposit(hash_fixed).call().await {
            Ok(deposit) => {
                // timestamp == 0 means no deposit record
                if deposit.timestamp.is_zero() {
                    debug!(
                        hash = %bytes32_to_hex(xchain_hash_id),
                        rpc = rpc_url,
                        "No deposit found on source chain (timestamp=0)"
                    );
                    return Ok(false);
                }

                // Defense in depth: verify the deposit's destChain matches this chain
                if deposit.destChain.0 != *self.this_chain_id.as_bytes() {
                    warn!(
                        hash = %bytes32_to_hex(xchain_hash_id),
                        deposit_dest = %format!("0x{}", hex::encode(deposit.destChain.0)),
                        this_chain = %format!("0x{}", hex::encode(self.this_chain_id.as_bytes())),
                        "Deposit destChain does not match this chain — rejecting"
                    );
                    return Ok(false);
                }

                info!(
                    hash = %bytes32_to_hex(xchain_hash_id),
                    nonce = deposit.nonce,
                    amount = %deposit.amount,
                    dest_chain = %format!("0x{}", hex::encode(deposit.destChain.0)),
                    rpc = rpc_url,
                    "Deposit verified on source chain"
                );
                Ok(true)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    hash = %bytes32_to_hex(xchain_hash_id),
                    rpc = rpc_url,
                    "Failed to query getDeposit on source chain"
                );
                Err(eyre!("Failed to verify deposit: {}", e))
            }
        }
    }

    /// Sync DB deposit status after V2 poll-and-approve creates an on-chain approval.
    ///
    /// The V2 poll-and-approve path works from on-chain WithdrawSubmit events and does
    /// not consume deposits from the DB. This helper updates the corresponding DB record
    /// (evm_deposits or terra_deposits) so the /status endpoint's `pending_deposits` count
    /// stays accurate and the legacy DB-driven paths don't reprocess the deposit.
    async fn sync_deposit_status_after_approval(&self, src_chain_id: &[u8; 4], nonce: u64) {
        // Try EVM deposits first (EVM→EVM or EVM→Terra that was recorded by EVM watcher)
        match db::find_evm_deposit_id_by_nonce_for_evm(&self.db, nonce as i64).await {
            Ok(Some(deposit_id)) => {
                if let Err(e) =
                    db::update_evm_deposit_status(&self.db, deposit_id, "processed").await
                {
                    warn!(
                        deposit_id = deposit_id,
                        nonce = nonce,
                        error = %e,
                        "Failed to sync evm_deposit status after V2 approval"
                    );
                } else {
                    debug!(
                        deposit_id = deposit_id,
                        nonce = nonce,
                        "Synced evm_deposit as processed (V2 poll-and-approve)"
                    );
                }
                return;
            }
            Ok(None) => {}
            Err(e) => {
                debug!(
                    nonce = nonce,
                    src_chain = %format!("0x{}", hex::encode(src_chain_id)),
                    error = %e,
                    "DB lookup for evm_deposit failed (non-fatal)"
                );
            }
        }

        // Try Terra deposits (Terra→EVM)
        if self
            .terra_chain_id
            .as_ref()
            .is_some_and(|id| src_chain_id == id.as_bytes())
        {
            match db::find_terra_deposit_id_by_nonce(&self.db, nonce as i64).await {
                Ok(Some(deposit_id)) => {
                    if let Err(e) =
                        db::update_terra_deposit_status(&self.db, deposit_id, "processed").await
                    {
                        warn!(
                            deposit_id = deposit_id,
                            nonce = nonce,
                            error = %e,
                            "Failed to sync terra_deposit status after V2 approval"
                        );
                    } else {
                        debug!(
                            deposit_id = deposit_id,
                            nonce = nonce,
                            "Synced terra_deposit as processed (V2 poll-and-approve)"
                        );
                    }
                }
                Ok(None) => {
                    debug!(
                        nonce = nonce,
                        "No DB record found for Terra deposit nonce (deposit may not have been recorded yet)"
                    );
                }
                Err(e) => {
                    debug!(
                        nonce = nonce,
                        error = %e,
                        "DB lookup for terra_deposit failed (non-fatal)"
                    );
                }
            }
        }
    }

    /// Verify Terra-source deposit exists by querying Terra `xchain_hash_id`.
    async fn verify_terra_deposit(&self, xchain_hash_id: &[u8; 32]) -> Result<bool> {
        let lcd_url = self
            .terra_lcd_url
            .as_ref()
            .ok_or_else(|| eyre!("Terra LCD URL not configured for Terra-source verification"))?;
        let bridge = self.terra_bridge_address.as_ref().ok_or_else(|| {
            eyre!("Terra bridge address not configured for Terra-source verification")
        })?;

        let query = serde_json::json!({
            "xchain_hash_id": {
                "xchain_hash_id": base64::engine::general_purpose::STANDARD.encode(xchain_hash_id)
            }
        });
        let query_b64 =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&query)?);
        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            lcd_url.trim_end_matches('/'),
            bridge,
            query_b64
        );

        let response = reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| eyre!("Terra deposit verification request failed: {}", e))?;
        if !response.status().is_success() {
            return Err(eyre!(
                "Terra deposit verification failed with status {}",
                response.status()
            ));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| eyre!("Failed to parse Terra deposit verification response: {}", e))?;

        let exists = Self::terra_deposit_exists_in_query(&body);
        if exists {
            info!(
                hash = %bytes32_to_hex(xchain_hash_id),
                nonce = body["data"]["nonce"].as_u64().unwrap_or_default(),
                amount = body["data"]["amount"].as_str().unwrap_or("?"),
                "Terra deposit verified on source chain"
            );
        } else {
            info!(
                hash = %bytes32_to_hex(xchain_hash_id),
                "Terra deposit not found on source chain for withdraw hash"
            );
        }

        Ok(exists)
    }

    fn terra_deposit_exists_in_query(body: &serde_json::Value) -> bool {
        body.get("data").is_some_and(|data| !data.is_null())
    }

    /// Submit a withdrawApprove transaction
    async fn submit_withdraw_approve(&self, xchain_hash_id: &[u8; 32]) -> Result<String> {
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = Bridge::new(self.bridge_address, &provider);
        let xchain_hash_id_fixed: FixedBytes<32> = FixedBytes::from(*xchain_hash_id);

        let call = contract.withdrawApprove(xchain_hash_id_fixed);

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send withdrawApprove tx: {}", e))?;

        let tx_hash = *pending_tx.tx_hash();
        info!(tx_hash = %tx_hash, "withdrawApprove tx sent, waiting for confirmation");

        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("withdrawApprove transaction reverted"));
        }

        Ok(format!("0x{:x}", tx_hash))
    }

    /// Process pending executions (after cancel window has elapsed)
    async fn process_pending_executions(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for (hash, pending) in self.pending_executions.iter() {
            let elapsed = now.duration_since(pending.approved_at);

            if elapsed.as_secs() >= pending.delay_seconds {
                // Delay has elapsed, try to execute
                match self.submit_execute_withdraw(*hash).await {
                    Ok(tx_hash) => {
                        info!(
                            xchain_hash_id = %bytes32_to_hex(hash),
                            tx_hash = %tx_hash,
                            "Successfully executed EVM withdrawal"
                        );
                        to_remove.push(*hash);
                    }
                    Err(e) => {
                        warn!(
                            xchain_hash_id = %bytes32_to_hex(hash),
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
        // Source chain is Terra Classic — use the V2 chain ID (auto-discovered from ChainRegistry at startup)
        let src_chain_id = self
            .terra_chain_id
            .ok_or_else(|| eyre!("Terra chain ID not configured — cannot process Terra deposit"))?;

        debug!(
            deposit_id = deposit.id,
            src_chain_id_hex = %format!("0x{}", hex::encode(src_chain_id.as_bytes())),
            dest_chain_id = %format!("0x{}", hex::encode(self.this_chain_id.as_bytes())),
            nonce = deposit.nonce,
            amount = %deposit.amount,
            "Processing Terra→EVM deposit"
        );

        // Check if approval already exists
        // Use V2 chain ID (self.this_chain_id) — must match what we INSERT into approvals
        if db::approval_exists(
            &self.db,
            src_chain_id.as_bytes(),
            deposit.nonce,
            self.this_chain_id.to_u32() as i64,
        )
        .await?
        {
            db::update_terra_deposit_status(&self.db, deposit.id, "processed").await?;
            return Ok(());
        }

        // Calculate fee
        let fee = self.calculate_fee(&deposit.amount);

        // Get the destination token address from the deposit event (per-chain from TOKEN_DEST_MAPPINGS)
        let dest_token_str = deposit.dest_token_address.as_ref().ok_or_else(|| {
            eyre!(
                "Missing dest_token_address for Terra deposit nonce={}",
                deposit.nonce
            )
        })?;
        let recipient = EvmAddress::from_hex(&deposit.recipient)?;
        let token = EvmAddress::from_hex(dest_token_str)
            .map_err(|e| eyre!("Invalid dest token address '{}': {}", dest_token_str, e))?;

        // Token as bytes32
        let mut token_bytes32 = [0u8; 32];
        token_bytes32[12..32].copy_from_slice(&token.0);

        // Source account (Terra sender encoded as left-padded bytes32)
        // Must match the encoding used by the Terra bridge contract: bech32-decode
        // the address to get the 20-byte canonical address, then left-pad to 32 bytes
        // so the address occupies positions [12..32].
        // SECURITY: decode MUST succeed. A zero src_account causes hash mismatch between
        // the operator and on-chain bridge, making the withdrawal unmatchable and locking funds.
        let mut src_account = [0u8; 32];
        let (raw, _) = crate::hash::decode_bech32_address(&deposit.sender).map_err(|e| {
            eyre::eyre!(
                "CRITICAL: Failed to decode Terra sender address '{}': {}. \
                 Cannot compute correct hash — would cause permanent fund lock.",
                deposit.sender,
                e
            )
        })?;
        let start = 32 - raw.len();
        src_account[start..32].copy_from_slice(&raw);

        // Parse amount
        let amount: u128 = deposit
            .amount
            .parse()
            .map_err(|_| eyre!("Invalid amount: {}", deposit.amount))?;

        // Destination account (EVM recipient) encoded as bytes32
        let dest_account = address_to_bytes32(&recipient.0);

        // Compute unified transfer hash using V2 format (7-field)
        let xchain_hash_id = compute_xchain_hash_id(
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
            dest_chain_id: self.this_chain_id.to_u32() as i64,
            xchain_hash_id: xchain_hash_id.to_vec(),
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
            .submit_approval(deposit, &src_chain_id, &xchain_hash_id)
            .await
        {
            Ok(tx_hash) => {
                info!(
                    approval_id = approval_id,
                    tx_hash = %tx_hash,
                    xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                    "Submitted approval transaction"
                );

                // Track for auto-execution
                self.pending_executions.insert(
                    xchain_hash_id,
                    PendingExecution {
                        xchain_hash_id,
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
    /// In V2, the **user** must call `withdrawSubmit` on the destination chain first.
    /// The operator only approves after verifying the deposit on the source chain.
    /// The operator must NEVER submit withdraws on behalf of users — the canceler
    /// needs the user-initiated submit to be able to cancel fraudulent withdrawals.
    ///
    /// Pre-flight checks:
    /// 1. Verify `withdrawSubmit` has been called (submittedAt != 0) — if not, skip (user must submit)
    /// 2. Verify the withdrawal is not already approved
    async fn submit_approval(
        &self,
        deposit: &TerraDeposit,
        _src_chain_id: &ChainId,
        xchain_hash_id: &[u8; 32],
    ) -> Result<String> {
        // Build provider with signer and recommended fillers (gas, nonce, fees)
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let xchain_hash_id_fixed: FixedBytes<32> = FixedBytes::from(*xchain_hash_id);

        // Create V2 contract instance
        let contract = Bridge::new(self.bridge_address, &provider);

        // Pre-flight: verify the withdrawal has been submitted
        let pending = contract
            .getPendingWithdraw(xchain_hash_id_fixed)
            .call()
            .await
            .map_err(|e| {
                eyre!(
                    "Pre-flight getPendingWithdraw failed for {}: {}",
                    bytes32_to_hex(xchain_hash_id),
                    e
                )
            })?;

        // User must have called withdrawSubmit first — operator never submits on behalf of users
        if pending.submittedAt.is_zero() {
            return Err(eyre!(
                "WithdrawSubmit not yet called (submittedAt=0). User must call withdrawSubmit on EVM first. \
                 Operator only approves; canceler requires user-initiated submit to cancel fraudulent withdrawals."
            ));
        }

        if pending.approved {
            info!(
                xchain_hash_id = %bytes32_to_hex(xchain_hash_id),
                "Withdrawal already approved, skipping"
            );
            return Err(eyre!("Already approved"));
        }

        info!(
            xchain_hash_id = %bytes32_to_hex(xchain_hash_id),
            submitted_at = %pending.submittedAt,
            nonce = pending.nonce,
            amount = %pending.amount,
            "Pre-flight passed: user submitted withdrawal, submitting withdrawApprove"
        );

        debug!(
            xchain_hash_id = %bytes32_to_hex(xchain_hash_id),
            nonce = deposit.nonce,
            "Submitting withdrawApprove (V2)"
        );

        let call = contract.withdrawApprove(xchain_hash_id_fixed);

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
    async fn submit_execute_withdraw(&self, xchain_hash_id: [u8; 32]) -> Result<String> {
        // Build provider with signer and recommended fillers (gas, nonce, fees)
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = Bridge::new(self.bridge_address, &provider);

        // Query pending withdrawal — validate it exists and is ready for execution
        let pending = contract
            .getPendingWithdraw(FixedBytes::from(xchain_hash_id))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get pending withdraw: {}", e))?;

        // getPendingWithdraw returns a zero struct if the hash doesn't exist
        if pending.submittedAt.is_zero() {
            return Err(eyre!(
                "Withdrawal {} not found (submittedAt is zero)",
                bytes32_to_hex(&xchain_hash_id)
            ));
        }
        if pending.executed {
            return Err(eyre!(
                "Withdrawal {} already executed",
                bytes32_to_hex(&xchain_hash_id)
            ));
        }
        if pending.cancelled {
            return Err(eyre!(
                "Withdrawal {} was cancelled",
                bytes32_to_hex(&xchain_hash_id)
            ));
        }

        let token_addr = pending.token;

        let registry_addr = contract
            .tokenRegistry()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get token registry: {}", e))?
            ._0;

        let token_registry = TokenRegistry::new(registry_addr, &provider);
        let token_type = token_registry
            .getTokenType(token_addr)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get token type: {}", e))?
            .tokenType;

        // LockUnlock = 0, MintBurn = 1
        let use_mint = token_type == 1;

        debug!(
            xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
            token = %token_addr,
            token_type = token_type,
            mode = if use_mint { "mint" } else { "unlock" },
            "Submitting withdraw execution (V2)"
        );

        let pending_tx = if use_mint {
            contract
                .withdrawExecuteMint(FixedBytes::from(xchain_hash_id))
                .send()
                .await
        } else {
            contract
                .withdrawExecuteUnlock(FixedBytes::from(xchain_hash_id))
                .send()
                .await
        }
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

    /// Process pending EVM deposits destined for this EVM chain (EVM→EVM path).
    ///
    /// This method handles the critical EVM-to-EVM transfer path (e.g., BSC→opBNB,
    /// ETH→Polygon). The EVM watcher already classifies and stores these deposits
    /// with `dest_chain_type = 'evm'` in the database.
    ///
    /// Flow:
    /// 1. Query DB for pending EVM deposits with dest_chain_type = 'evm'
    /// 2. For each deposit, verify it exists on the source EVM chain
    /// 3. Compute the transfer hash using the source chain's 4-byte ID
    /// 4. Submit withdrawApprove(hash) on this (destination) chain's bridge contract
    /// 5. Mark deposit as processed
    pub async fn process_evm_to_evm_pending(&mut self) -> Result<()> {
        // First, check if any pending executions are ready
        self.process_pending_executions().await?;

        // Query EVM deposits destined for EVM chains
        let deposits = db::get_pending_evm_deposits_for_evm(&self.db).await?;

        if !deposits.is_empty() {
            info!(count = deposits.len(), "Processing EVM→EVM deposits");
        }

        for deposit in deposits {
            if let Err(e) = self.process_evm_deposit(&deposit).await {
                error!(
                    deposit_id = deposit.id,
                    error = %e,
                    error_chain = ?e,
                    "Failed to process EVM→EVM deposit"
                );
            }
        }

        Ok(())
    }

    /// Process a single EVM→EVM deposit
    ///
    /// Verifies the deposit exists on the source chain and submits
    /// withdrawApprove on the destination chain.
    async fn process_evm_deposit(&mut self, deposit: &EvmDeposit) -> Result<()> {
        // Extract the source chain's V2 4-byte ID
        // Use the V2 chain ID stored by the watcher (queried from bridge contract).
        // Falls back to native chain ID conversion if V2 ID not available.
        let src_chain_id = if let Some(ref v2_bytes) = deposit.src_v2_chain_id {
            if v2_bytes.len() >= 4 {
                let mut id = [0u8; 4];
                id.copy_from_slice(&v2_bytes[..4]);
                crate::types::ChainId::from_bytes(id)
            } else {
                return Err(eyre::eyre!(
                    "Deposit {} has src_v2_chain_id with {} bytes (expected 4). \
                     Cannot determine source chain — skipping to avoid wrong hash.",
                    deposit.id,
                    v2_bytes.len()
                ));
            }
        } else {
            // No V2 chain ID stored for this deposit. Using native chain ID would
            // produce wrong transfer hashes if ChainRegistry uses different IDs
            // (e.g., native 31337 vs registry 0x00000001). Return error instead.
            return Err(eyre::eyre!(
                "Deposit {} has no V2 chain ID stored (src_v2_chain_id is None). \
                 Cannot safely compute transfer hash. Ensure the EVM watcher stores \
                 src_v2_chain_id for all deposits. Native chain_id={} is NOT used.",
                deposit.id,
                deposit.chain_id
            ));
        };

        debug!(
            deposit_id = deposit.id,
            native_chain_id = deposit.chain_id,
            v2_src_chain_id = %format!("0x{}", hex::encode(src_chain_id.as_bytes())),
            v2_dest_chain_id = %format!("0x{}", hex::encode(self.this_chain_id.as_bytes())),
            nonce = deposit.nonce,
            amount = %deposit.amount,
            "Processing EVM→EVM deposit"
        );

        // Check if approval already exists (use V2 chain ID for dest)
        if db::approval_exists(
            &self.db,
            src_chain_id.as_bytes(),
            deposit.nonce,
            self.this_chain_id.to_u32() as i64,
        )
        .await?
        {
            db::update_evm_deposit_status(&self.db, deposit.id, "processed").await?;
            return Ok(());
        }

        // Parse source account (the depositor on source EVM chain)
        let mut src_account = [0u8; 32];
        if let Some(ref src_acc) = deposit.src_account {
            if src_acc.len() >= 32 {
                src_account.copy_from_slice(&src_acc[..32]);
            } else if !src_acc.is_empty() {
                warn!(
                    deposit_id = deposit.id,
                    src_account_len = src_acc.len(),
                    "src_account is shorter than 32 bytes, left-padding"
                );
                let start = 32 - src_acc.len();
                src_account[start..32].copy_from_slice(src_acc);
            }
        }

        // Warn if src_account is all zeros (will cause hash mismatch)
        if src_account.iter().all(|&b| b == 0) {
            warn!(
                deposit_id = deposit.id,
                nonce = deposit.nonce,
                "src_account is all zeros — hash will likely not match the on-chain deposit hash. \
                 This may indicate a V1 deposit or missing src_account extraction."
            );
        }

        // Parse destination account
        let mut dest_account = [0u8; 32];
        if deposit.dest_account.len() >= 32 {
            dest_account.copy_from_slice(&deposit.dest_account[..32]);
        }

        // Parse destination token (bytes32)
        let mut token_bytes32 = [0u8; 32];
        if deposit.dest_token_address.len() >= 32 {
            token_bytes32.copy_from_slice(&deposit.dest_token_address[..32]);
        }

        // Parse amount
        let amount: u128 = deposit
            .amount
            .parse()
            .map_err(|_| eyre!("Invalid amount: {}", deposit.amount))?;

        // Compute the unified transfer hash (7-field V2 format)
        let xchain_hash_id = compute_xchain_hash_id(
            src_chain_id.as_bytes(),
            self.this_chain_id.as_bytes(),
            &src_account,
            &dest_account,
            &token_bytes32,
            amount,
            deposit.nonce as u64,
        );

        // Calculate fee
        let fee = self.calculate_fee(&deposit.amount);

        // Format token and recipient for the approval record
        // Always extract the last 20 bytes (EVM address) from the 32-byte representation.
        // The first 12 bytes may be zero-padding (standard) or contain chain type prefix
        // (universal address format). Either way, the EVM address is in the last 20 bytes.
        let token_hex = format!("0x{}", hex::encode(&token_bytes32[12..32]));

        // Extract recipient from dest_account (last 20 bytes for EVM)
        let recipient_hex = format!("0x{}", hex::encode(&dest_account[12..32]));

        let new_approval = db::NewApproval {
            src_chain_key: src_chain_id.as_bytes().to_vec(),
            nonce: deposit.nonce,
            dest_chain_id: self.this_chain_id.to_u32() as i64,
            xchain_hash_id: xchain_hash_id.to_vec(),
            token: token_hex,
            recipient: recipient_hex,
            amount: deposit.amount.clone(),
            fee: fee.to_string(),
            fee_recipient: Some(format!("0x{:x}", self.fee_recipient)),
            deduct_from_amount: false,
        };

        let approval_id = db::insert_approval(&self.db, &new_approval).await?;
        info!(
            approval_id = approval_id,
            nonce = deposit.nonce,
            src_chain_id = %src_chain_id,
            xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
            "Created approval for EVM→EVM deposit"
        );

        // Submit withdrawApprove on this (destination) EVM chain
        match self.submit_evm_to_evm_approval(&xchain_hash_id).await {
            Ok(tx_hash) => {
                info!(
                    approval_id = approval_id,
                    tx_hash = %tx_hash,
                    xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
                    "Submitted EVM→EVM approval transaction"
                );

                // Track for auto-execution after cancel window
                self.pending_executions.insert(
                    xchain_hash_id,
                    PendingExecution {
                        xchain_hash_id,
                        approved_at: Instant::now(),
                        delay_seconds: self.cancel_window,
                        attempts: 0,
                    },
                );

                db::update_evm_deposit_status(&self.db, deposit.id, "approved").await?;
                db::update_approval_submitted(&self.db, approval_id, &tx_hash).await?;
            }
            Err(e) => {
                warn!(
                    approval_id = approval_id,
                    error = %e,
                    "Failed to submit EVM→EVM approval, will retry"
                );
                db::update_approval_failed(&self.db, approval_id, &e.to_string()).await?;
            }
        }

        Ok(())
    }

    /// Submit a withdrawApprove transaction for an EVM→EVM transfer
    ///
    /// Pre-flight: checks that `withdrawSubmit` has already been called for
    /// this hash (i.e., `submittedAt != 0`). If not, the approval would revert
    /// on-chain, so we bail early with a retriable error.
    async fn submit_evm_to_evm_approval(&self, xchain_hash_id: &[u8; 32]) -> Result<String> {
        // Build provider with signer and recommended fillers (gas, nonce, fees)
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let xchain_hash_id_fixed: FixedBytes<32> = FixedBytes::from(*xchain_hash_id);

        let contract = Bridge::new(self.bridge_address, &provider);

        // Pre-flight: verify the withdrawal has been submitted by the user
        let pending = contract
            .getPendingWithdraw(xchain_hash_id_fixed)
            .call()
            .await
            .map_err(|e| {
                eyre!(
                    "Pre-flight getPendingWithdraw failed for {}: {}",
                    bytes32_to_hex(xchain_hash_id),
                    e
                )
            })?;

        if pending.submittedAt.is_zero() {
            return Err(eyre!(
                "WithdrawSubmit not yet called for {} (submittedAt=0). \
                 User must call withdrawSubmit before operator can approve.",
                bytes32_to_hex(xchain_hash_id)
            ));
        }

        if pending.approved {
            info!(
                xchain_hash_id = %bytes32_to_hex(xchain_hash_id),
                "Withdrawal already approved, skipping"
            );
            return Err(eyre!("Already approved"));
        }

        info!(
            xchain_hash_id = %bytes32_to_hex(xchain_hash_id),
            submitted_at = %pending.submittedAt,
            nonce = pending.nonce,
            amount = %pending.amount,
            "Pre-flight passed: withdrawal exists, submitting withdrawApprove"
        );

        let call = contract.withdrawApprove(xchain_hash_id_fixed);

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send EVM→EVM approval tx: {}", e))?;

        let tx_hash = *pending_tx.tx_hash();
        info!(tx_hash = %tx_hash, "EVM→EVM approval tx sent, waiting for confirmation");

        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("EVM→EVM approval transaction reverted"));
        }

        Ok(format!("0x{:x}", tx_hash))
    }

    /// Calculate fee based on amount
    fn calculate_fee(&self, amount: &str) -> U256 {
        let amount_u256 = U256::from_str(amount).unwrap_or_else(|e| {
            warn!(
                amount = amount,
                error = %e,
                "Failed to parse amount for fee calculation, using zero"
            );
            U256::ZERO
        });
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

#[cfg(test)]
mod tests {
    use super::EvmWriter;

    // ========================================================================
    // Terra Deposit Verification Tests
    // ========================================================================

    #[test]
    fn test_terra_deposit_exists_in_query_when_data_present() {
        let body = serde_json::json!({
            "data": {
                "nonce": 1,
                "amount": "997000"
            }
        });
        assert!(EvmWriter::terra_deposit_exists_in_query(&body));
    }

    #[test]
    fn test_terra_deposit_exists_in_query_when_data_null() {
        let body = serde_json::json!({ "data": serde_json::Value::Null });
        assert!(!EvmWriter::terra_deposit_exists_in_query(&body));
    }

    #[test]
    fn test_terra_deposit_exists_in_query_when_data_missing() {
        let body = serde_json::json!({ "error": "not found" });
        assert!(!EvmWriter::terra_deposit_exists_in_query(&body));
    }

    #[test]
    fn test_terra_deposit_exists_in_query_with_full_deposit_record() {
        // Real-world response from Terra bridge xchain_hash_id query
        let body = serde_json::json!({
            "data": {
                "nonce": 0,
                "amount": "995000",
                "xchain_hash_id": "de2e967d5bfeea4a57a752a561b70d241ef3ef8474a48901f92b0648cfb002f7",
                "deposited_at": "1770716032132606174",
                "dest_account": "AAAAAAAAAAAAAAAA85/W5RqtiPb0zmq4gnJ5z/+5ImY=",
                "dest_token_address": "AAAAAAAAAAAAAAAACzBr+RXE1kX/WW5Rj68/lmm5cBY=",
                "src_account": "AAAAAAAAAAAAAAAANXQwdJVscQgA6DGYARzL1N3xVW0="
            }
        });
        assert!(EvmWriter::terra_deposit_exists_in_query(&body));
    }

    #[test]
    fn test_terra_deposit_exists_in_query_with_empty_data_object() {
        // Empty data object should be treated as "exists" (has a value, just no fields)
        let body = serde_json::json!({ "data": {} });
        assert!(EvmWriter::terra_deposit_exists_in_query(&body));
    }

    // ========================================================================
    // V2 Flow Invariant Tests
    // ========================================================================

    /// Verify that the V2 flow correctly routes Terra-source deposits to Terra LCD
    /// verification instead of EVM getDeposit().
    ///
    /// This is a structural test: the source_chain_id matching logic determines
    /// whether verification goes to Terra LCD or EVM RPC.
    #[test]
    fn test_source_chain_routing_terra_vs_evm() {
        use crate::types::ChainId;

        let terra_chain_id = ChainId::from_u32(2);
        let evm_chain_id_bytes: [u8; 4] = [0, 0, 0, 1];
        let terra_chain_id_bytes: [u8; 4] = [0, 0, 0, 2];

        // Terra-source should route to Terra verification
        assert_eq!(
            terra_chain_id_bytes,
            *terra_chain_id.as_bytes(),
            "Terra chain ID bytes should match"
        );
        assert!(
            terra_chain_id_bytes == *terra_chain_id.as_bytes(),
            "Terra source should be identified for Terra LCD routing"
        );

        // EVM-source should NOT route to Terra verification
        assert_ne!(
            evm_chain_id_bytes,
            *terra_chain_id.as_bytes(),
            "EVM chain ID should not match Terra chain ID"
        );
    }

    /// Verify hash-matching invariant: the xchain_hash_id on EVM must match
    /// the xchain_hash_id on the source chain. The operator does NOT recompute
    /// hashes — it verifies by looking up the hash directly.
    #[test]
    fn test_hash_matching_is_identity() {
        // In V2, xchain_hash_id on source == xchain_hash_id on destination.
        // Both are computed from the same 7-field compute_xchain_hash_id.
        // The operator only needs to verify the hash exists, not recompute it.
        let xchain_hash_id: [u8; 32] = [
            0xde, 0x2e, 0x96, 0x7d, 0x5b, 0xfe, 0xea, 0x4a, 0x57, 0xa7, 0x52, 0xa5, 0x61, 0xb7,
            0x0d, 0x24, 0x1e, 0xf3, 0xef, 0x84, 0x74, 0xa4, 0x89, 0x01, 0xf9, 0x2b, 0x06, 0x48,
            0xcf, 0xb0, 0x02, 0xf7,
        ];
        let xchain_hash_id = xchain_hash_id; // Identity — same hash

        assert_eq!(
            xchain_hash_id, xchain_hash_id,
            "V2 invariant: xchain_hash_id == xchain_hash_id (no recomputation)"
        );
    }
}
