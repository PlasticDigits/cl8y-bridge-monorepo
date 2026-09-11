use alloy::primitives::Address;
use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

use crate::poll_config::{jittered_exponential_backoff, WriterScheduleConfig};
use crate::types::ChainId;

pub mod evm;
pub mod execute_queue;
pub mod negative_retry;
pub mod poll_cursor;
pub mod retry;
pub mod solana;
pub mod terra;
pub mod terra_list;

pub use evm::EvmWriter;
pub use retry::{classify_error, RetryConfig};
pub use solana::SolanaWriter;
pub use terra::TerraWriter;

/// Seconds remaining until `withdrawExecute*` is allowed after an on-chain approval.
///
/// `0` means the cancel window has elapsed (or `approved_at_unix` is unset). The
/// EVM contract uses an exclusive boundary (`timestamp > approvedAt + window`);
/// a zero delay may still revert `CancelWindowActive` for one poll, then retry.
pub(crate) fn remaining_cancel_window_secs(
    approved_at_unix: u64,
    cancel_window: u64,
    now_unix: u64,
) -> u64 {
    if approved_at_unix == 0 {
        return 0;
    }
    approved_at_unix
        .saturating_add(cancel_window)
        .saturating_sub(now_unix)
}

pub(crate) fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Solidity `BelowMinPerTransaction(uint256,uint256)` selector (INV-OP-W11).
pub(crate) const BELOW_MIN_PER_TX_SELECTOR: &str = "0x1913498b";
/// `WithdrawAlreadyExecuted(bytes32)` — execute tx race after a concurrent execute.
pub(crate) const WITHDRAW_ALREADY_EXECUTED_SELECTOR: &str = "0xe744b3ba";
/// `WithdrawCancelled(bytes32)` — execute tx race after a user cancel.
pub(crate) const WITHDRAW_CANCELLED_SELECTOR: &str = "0xa4d52f4c";

/// True when execute should not be retried or re-queued.
///
/// Covers on-chain terminal states (executed / cancelled / missing) and
/// `BelowMinPerTransaction` / Terra `BelowMinimumAmount` (amount can never clear).
pub(crate) fn is_terminal_execute_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("already executed")
        || lower.contains("withdrawalreadyexecuted")
        || lower.contains("was cancelled")
        || lower.contains("withdrawcancelled")
        || lower.contains("submittedat is zero")
        || lower.contains(BELOW_MIN_PER_TX_SELECTOR)
        || lower.contains(WITHDRAW_ALREADY_EXECUTED_SELECTOR)
        || lower.contains(WITHDRAW_CANCELLED_SELECTOR)
        || lower.contains("belowminpertransaction")
        || lower.contains("belowminimumamount")
}

/// Solana source chain configuration for deposit verification.
/// Used by EVM and Terra writers to verify deposits originating from Solana.
#[derive(Debug, Clone)]
pub struct SolanaSourceConfig {
    /// Same ordered list as the Solana watcher/writer (`SOLANA_RPC_URL` primary + fallbacks).
    pub rpc_urls: Vec<String>,
    pub program_id: [u8; 32],
    /// All SVM V2 chain IDs that share this RPC + program (multi-SVM / forks).
    pub chain_ids: Vec<[u8; 4]>,
}

fn solana_verify_http_status_retry_next(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status == reqwest::StatusCode::FORBIDDEN
        || status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
}

/// Verify a deposit exists on Solana by querying the DepositRecord PDA.
///
/// Derives the PDA from seeds `["deposit", nonce.to_le_bytes()]`, fetches account
/// data via JSON-RPC, and checks that the stored transfer_hash (bytes 8..40 of the
/// Anchor account) matches `xchain_hash_id`.
///
/// Uses the same ordered RPC list as tx submission / the watcher: retries on transient
/// HTTP / JSON-RPC failures against the next endpoint.
pub(crate) async fn verify_deposit_on_solana_source(
    http: &reqwest::Client,
    config: &SolanaSourceConfig,
    xchain_hash_id: &[u8; 32],
    nonce: u64,
) -> eyre::Result<bool> {
    use base64::Engine;
    use solana_sdk::pubkey::Pubkey;
    use tracing::{info, warn};

    if config.rpc_urls.is_empty() {
        return Err(eyre!("Solana source verification: no RPC URLs configured"));
    }

    let nonce_bytes = nonce.to_le_bytes();
    let program_id = Pubkey::new_from_array(config.program_id);
    let (deposit_pda, _bump) =
        Pubkey::find_program_address(&[b"deposit", &nonce_bytes], &program_id);
    let deposit_pda_str = deposit_pda.to_string();

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getAccountInfo",
        "params": [
            deposit_pda_str.clone(),
            {"encoding": "base64", "commitment": "finalized"}
        ]
    });

    let mut last_err: Option<eyre::Report> = None;

    for url in &config.rpc_urls {
        let resp = match http.post(url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    error = %crate::rpc_fallback::log_rpc_error(&e),
                    rpc = %crate::rpc_fallback::log_rpc(url),
                    "Solana RPC request failed during deposit verification, trying next"
                );
                last_err = Some(eyre::eyre!(
                    "Solana RPC request failed: {}",
                    crate::rpc_fallback::log_rpc_error(&e)
                ));
                continue;
            }
        };

        let status = resp.status();
        if !status.is_success() {
            warn!(
                status = %status,
                rpc = %crate::rpc_fallback::log_rpc(url),
                "Solana RPC HTTP error during deposit verification"
            );
            let err = eyre::eyre!("Solana RPC returned {}", status);
            if solana_verify_http_status_retry_next(status) {
                last_err = Some(err);
                continue;
            }
            return Err(err);
        }

        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(e) => {
                warn!(
                    error = %crate::rpc_fallback::log_rpc_error(&e),
                    rpc = %crate::rpc_fallback::log_rpc(url),
                    "Solana RPC JSON decode failed during deposit verification, trying next"
                );
                last_err = Some(eyre::eyre!(
                    "Solana RPC JSON decode failed: {}",
                    crate::rpc_fallback::log_rpc_error(&e)
                ));
                continue;
            }
        };

        if !json["error"].is_null() {
            warn!(
                error = %json["error"],
                rpc = %crate::rpc_fallback::log_rpc(url),
                "Solana JSON-RPC error during deposit verification, trying next endpoint"
            );
            last_err = Some(eyre::eyre!("Solana JSON-RPC error: {}", json["error"]));
            continue;
        }

        let result = &json["result"]["value"];

        if result.is_null() {
            info!(
                hash = %hex::encode(xchain_hash_id),
                nonce = nonce,
                pda = %deposit_pda_str,
                rpc = %crate::rpc_fallback::log_rpc(url),
                "No deposit PDA found on Solana source chain"
            );
            return Ok(false);
        }

        if let Some(data_arr) = result["data"].as_array() {
            if let Some(data_b64) = data_arr.first().and_then(|v| v.as_str()) {
                if let Ok(data_bytes) = base64::engine::general_purpose::STANDARD.decode(data_b64) {
                    if data_bytes.len() >= 40 {
                        let stored_hash = &data_bytes[8..40];
                        if stored_hash == xchain_hash_id {
                            info!(
                                hash = %hex::encode(xchain_hash_id),
                                nonce = nonce,
                                rpc = %crate::rpc_fallback::log_rpc(url),
                                "Deposit verified on Solana source chain"
                            );
                            return Ok(true);
                        } else {
                            warn!(
                                expected = %hex::encode(xchain_hash_id),
                                got = %hex::encode(stored_hash),
                                "Solana deposit exists but hash mismatch"
                            );
                            return Ok(false);
                        }
                    }
                }
            }
        }

        warn!(
            hash = %hex::encode(xchain_hash_id),
            rpc = %crate::rpc_fallback::log_rpc(url),
            "Solana deposit PDA data could not be parsed"
        );
        return Ok(false);
    }

    Err(last_err
        .unwrap_or_else(|| eyre!("All Solana RPC endpoints failed for deposit verification")))
}

/// Circuit breaker configuration for writer managers
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures before pausing
    pub threshold: u32,
    /// How long to pause when circuit breaker trips
    pub pause_duration: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            threshold: 10,
            pause_duration: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Manages transaction writers for all chain paths
///
/// Supports three transfer paths:
/// - Terra → EVM: `evm_writer.process_pending()` — processes Terra deposits
/// - EVM → Terra: `terra_writer.process_pending()` — polls Terra ActiveWithdrawals
/// - EVM → EVM:   per-chain EVM writers from `MultiEvmConfig` process EVM→EVM deposits
///
/// When `MultiEvmConfig` is provided, additional `EvmWriter` instances are created
/// for each enabled chain, enabling EVM-to-EVM bridging across multiple chains
/// (e.g., BSC→opBNB, ETH→Polygon).
pub struct WriterManager {
    /// Primary EVM writer for Terra→EVM approvals
    evm_writer: EvmWriter,
    terra_writer: TerraWriter,
    /// Per-chain EVM writers for EVM→EVM bridging, keyed by native chain ID.
    /// Each writer submits approvals to its respective chain's bridge contract.
    evm_chain_writers: HashMap<u64, EvmWriter>,
    retry_config: RetryConfig,
    circuit_breaker: CircuitBreakerConfig,
    consecutive_evm_failures: u32,
    consecutive_terra_failures: u32,
    consecutive_evm_to_evm_failures: u32,
    schedule: WriterScheduleConfig,
}

impl WriterManager {
    /// Create a new writer manager
    ///
    /// If `config.multi_evm` is set, creates additional `EvmWriter` instances
    /// for each enabled chain in the multi-EVM configuration. These writers
    /// handle EVM→EVM transfers by submitting approvals on the destination chain.
    pub async fn new(config: &crate::config::Config, db: PgPool) -> Result<Self> {
        // Build source chain endpoints for cross-chain deposit verification routing (O1).
        // Each EvmWriter gets this map so it can verify deposits on any known source chain,
        // routing to the correct RPC/bridge instead of always using its own.
        let mut source_chain_endpoints: HashMap<[u8; 4], (String, Address)> = HashMap::new();

        // Add the configured EVM chain using its required V2 chain ID.
        let v2_id = config
            .evm
            .this_chain_id
            .ok_or_else(|| eyre!("EVM_THIS_CHAIN_ID is required for source-chain routing"))?;
        let bridge = Address::from_str(&config.evm.bridge_address)
            .wrap_err("Invalid configured EVM bridge address")?;
        source_chain_endpoints.insert(
            ChainId::from_u32(v2_id).0,
            (config.evm.rpc_url.clone(), bridge),
        );

        // Add multi-EVM chains
        if let Some(ref multi) = config.multi_evm {
            for chain in multi.enabled_chains() {
                let bridge = Address::from_str(&chain.bridge_address)
                    .wrap_err_with(|| format!("Invalid bridge address for chain {}", chain.name))?;
                source_chain_endpoints
                    .insert(chain.this_chain_id.0, (chain.rpc_url.clone(), bridge));
            }
        }

        tracing::info!(
            source_chains = source_chain_endpoints.len(),
            "Built source chain verification endpoints for deposit routing"
        );

        let solana_source_config = config.solana.as_ref().map(|sol| {
            let program_id: [u8; 32] = sol
                .program_id
                .parse::<solana_sdk::pubkey::Pubkey>()
                .expect("SOLANA_PROGRAM_ID already validated in config")
                .to_bytes();
            let mut rpc_urls: Vec<String> = Vec::with_capacity(1 + sol.rpc_fallback_urls.len());
            rpc_urls.push(sol.rpc_url.clone());
            rpc_urls.extend(sol.rpc_fallback_urls.iter().cloned());
            SolanaSourceConfig {
                rpc_urls,
                program_id,
                chain_ids: sol.bytes4_chain_ids.clone(),
            }
        });

        let evm_writer = EvmWriter::new(
            &config.evm,
            Some(&config.terra),
            &config.fees,
            db.clone(),
            source_chain_endpoints.clone(),
            solana_source_config.clone(),
        )
        .await?;
        let terra_writer = TerraWriter::new(
            &config.terra,
            source_chain_endpoints.clone(),
            solana_source_config.clone(),
            db.clone(),
        )
        .await?;

        // Create per-chain EVM writers from MultiEvmConfig
        let mut evm_chain_writers = HashMap::new();
        if let Some(ref multi) = config.multi_evm {
            for chain in multi.enabled_chains() {
                use crate::multi_evm::EvmChainConfigExt;
                let chain_evm_config = chain.to_operator_evm_config(multi.private_key());
                match EvmWriter::new(
                    &chain_evm_config,
                    Some(&config.terra),
                    &config.fees,
                    db.clone(),
                    source_chain_endpoints.clone(),
                    solana_source_config.clone(),
                )
                .await
                {
                    Ok(writer) => {
                        tracing::info!(
                            chain_name = %chain.name,
                            chain_id = chain.chain_id,
                            bridge = %chain.bridge_address,
                            "Created EVM writer for multi-chain EVM→EVM bridging"
                        );
                        evm_chain_writers.insert(chain.chain_id, writer);
                    }
                    Err(e) => {
                        tracing::warn!(
                            chain_name = %chain.name,
                            chain_id = chain.chain_id,
                            error = %e,
                            "Failed to create EVM writer for chain, skipping"
                        );
                    }
                }
            }
            tracing::info!(
                active_chains = evm_chain_writers.len(),
                "Multi-EVM writers initialized"
            );
        }

        Ok(Self {
            evm_writer,
            terra_writer,
            evm_chain_writers,
            retry_config: RetryConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            consecutive_evm_failures: 0,
            consecutive_terra_failures: 0,
            consecutive_evm_to_evm_failures: 0,
            schedule: WriterScheduleConfig::from_env()?,
        })
    }

    /// Run each chain writer on its own schedule so a degraded RPC cannot stall
    /// healthy chains or hold shutdown for a full backoff (INV-OP-W5).
    pub async fn run(self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        let schedule = self.schedule;
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

        tracing::info!(
            poll_interval_ms = schedule.poll_interval.as_millis() as u64,
            rpc_backoff_initial_ms = schedule.rpc_backoff_initial.as_millis() as u64,
            rpc_backoff_max_ms = schedule.rpc_backoff_max.as_millis() as u64,
            isolated_evm_writers = 1 + self.evm_chain_writers.len(),
            "Writer manager starting isolated per-chain loops"
        );

        let mut handles = Vec::new();
        handles.push(tokio::spawn(run_evm_writer_loop(
            "evm-primary".to_string(),
            self.evm_writer,
            schedule,
            stop_rx.clone(),
        )));
        handles.push(tokio::spawn(run_terra_writer_loop(
            self.terra_writer,
            schedule,
            stop_rx.clone(),
        )));
        for (chain_id, writer) in self.evm_chain_writers {
            handles.push(tokio::spawn(run_evm_writer_loop(
                format!("evm-{chain_id}"),
                writer,
                schedule,
                stop_rx.clone(),
            )));
        }

        let _ = shutdown.recv().await;
        tracing::info!("Shutdown signal received, stopping writers");
        let _ = stop_tx.send(true);

        match tokio::time::timeout(Duration::from_secs(5), futures::future::join_all(handles)).await
        {
            Ok(results) => {
                for result in results {
                    if let Err(e) = result {
                        tracing::warn!(error = %e, "writer loop task join error");
                    }
                }
            }
            Err(_) => {
                tracing::warn!("writer loops did not all exit within 5s; continuing shutdown")
            }
        }
        Ok(())
    }

    /// Sequential process path retained for tests / debug; production uses isolated loops.
    #[allow(dead_code)]
    async fn process_pending(&mut self) -> Result<()> {
        // Check EVM circuit breaker
        if self.consecutive_evm_failures >= self.circuit_breaker.threshold {
            tracing::warn!(
                failures = self.consecutive_evm_failures,
                pause_secs = self.circuit_breaker.pause_duration.as_secs(),
                "EVM circuit breaker tripped, pausing EVM writer"
            );
            tokio::time::sleep(self.circuit_breaker.pause_duration).await;
            self.consecutive_evm_failures = 0;
        }

        // Process pending withdrawals on EVM (V2 poll-and-approve)
        // This handles BOTH Terra→EVM and EVM→EVM transfers:
        // polls WithdrawSubmit events, verifies deposits on source chain, approves.
        match self.evm_writer.process_pending().await {
            Ok(()) => {
                self.consecutive_evm_failures = 0;
            }
            Err(e) => {
                self.consecutive_evm_failures += 1;
                let error_class = classify_error(&e.to_string());
                let backoff = self
                    .retry_config
                    .backoff_for_attempt(self.consecutive_evm_failures);
                tracing::error!(
                    error = %e,
                    ?error_class,
                    consecutive_failures = self.consecutive_evm_failures,
                    next_backoff_secs = backoff.as_secs(),
                    "Error processing EVM approvals (poll-and-approve), will retry with backoff"
                );
                tokio::time::sleep(backoff).await;
            }
        }

        // Check Terra circuit breaker
        if self.consecutive_terra_failures >= self.circuit_breaker.threshold {
            tracing::warn!(
                failures = self.consecutive_terra_failures,
                pause_secs = self.circuit_breaker.pause_duration.as_secs(),
                "Terra circuit breaker tripped, pausing Terra writer"
            );
            tokio::time::sleep(self.circuit_breaker.pause_duration).await;
            self.consecutive_terra_failures = 0;
        }

        // Process pending withdrawals on Terra (poll-and-approve)
        // Handles EVM→Terra transfers: polls Terra PendingWithdrawals,
        // verifies deposits on EVM, approves on Terra.
        match self.terra_writer.process_pending().await {
            Ok(()) => {
                self.consecutive_terra_failures = 0;
            }
            Err(e) => {
                self.consecutive_terra_failures += 1;
                let error_class = classify_error(&e.to_string());
                let backoff = self
                    .retry_config
                    .backoff_for_attempt(self.consecutive_terra_failures);
                tracing::error!(
                    error = %e,
                    ?error_class,
                    consecutive_failures = self.consecutive_terra_failures,
                    next_backoff_secs = backoff.as_secs(),
                    "Error processing Terra releases, will retry with backoff"
                );
                tokio::time::sleep(backoff).await;
            }
        }

        // Multi-chain EVM writers: each per-chain writer polls its own chain
        // for WithdrawSubmit events and approves verified ones
        if !self.evm_chain_writers.is_empty() {
            let mut had_failure = false;
            for (chain_id, writer) in self.evm_chain_writers.iter_mut() {
                match writer.process_pending().await {
                    Ok(()) => {}
                    Err(e) => {
                        had_failure = true;
                        tracing::error!(
                            chain_id = chain_id,
                            error = %e,
                            "Error processing EVM approvals for chain"
                        );
                    }
                }
            }
            if had_failure {
                self.consecutive_evm_to_evm_failures += 1;
                let backoff = self
                    .retry_config
                    .backoff_for_attempt(self.consecutive_evm_to_evm_failures);
                tokio::time::sleep(backoff).await;
            } else {
                self.consecutive_evm_to_evm_failures = 0;
            }
        }

        Ok(())
    }

    /// Get health status
    #[allow(dead_code)]
    pub fn health_status(&self) -> HealthStatus {
        HealthStatus {
            evm_healthy: self.consecutive_evm_failures < self.circuit_breaker.threshold,
            terra_healthy: self.consecutive_terra_failures < self.circuit_breaker.threshold,
            evm_to_evm_healthy: self.consecutive_evm_to_evm_failures
                < self.circuit_breaker.threshold,
            evm_pending_executions: self.evm_writer.pending_execution_count(),
            terra_pending_executions: self.terra_writer.pending_execution_count(),
            multi_evm_chains: self.evm_chain_writers.len(),
        }
    }
}

/// Delay until the next writer cycle. Failures use capped jittered exponential backoff
/// so one chain cannot occupy the shared runtime with a tight retry loop.
pub fn next_writer_delay(
    success: bool,
    consecutive_failures: u32,
    schedule: &WriterScheduleConfig,
    seed: u64,
) -> Duration {
    if success {
        schedule.poll_interval
    } else {
        jittered_exponential_backoff(
            consecutive_failures.saturating_sub(1),
            schedule.rpc_backoff_initial,
            schedule.rpc_backoff_max,
            schedule.jitter_bps,
            seed,
        )
    }
}

async fn run_evm_writer_loop(
    name: String,
    mut writer: EvmWriter,
    schedule: WriterScheduleConfig,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let chain = format!("evm-{}", writer.native_chain_id());
    let mut consecutive = 0u32;
    let mut next_ready = Instant::now();
    tracing::info!(writer = %name, chain = %chain, "EVM writer loop started");
    loop {
        let wait = next_ready.saturating_duration_since(Instant::now());
        tokio::select! {
            _ = stop.changed() => {
                if *stop.borrow() {
                    tracing::info!(writer = %name, chain = %chain, "EVM writer loop shutting down");
                    return;
                }
            }
            _ = tokio::time::sleep(wait) => {
                match writer.process_pending().await {
                    Ok(()) => {
                        consecutive = 0;
                        next_ready = Instant::now() + next_writer_delay(true, 0, &schedule, 0);
                    }
                    Err(e) => {
                        consecutive = consecutive.saturating_add(1);
                        let seed = writer.native_chain_id() ^ consecutive as u64;
                        let delay = next_writer_delay(false, consecutive, &schedule, seed);
                        tracing::error!(
                            writer = %name,
                            chain = %chain,
                            error = %crate::rpc_fallback::log_rpc_error(&e),
                            consecutive,
                            backoff_ms = delay.as_millis() as u64,
                            "EVM writer cycle failed; backing off this chain only"
                        );
                        next_ready = Instant::now() + delay;
                    }
                }
            }
        }
    }
}

async fn run_terra_writer_loop(
    mut writer: TerraWriter,
    schedule: WriterScheduleConfig,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let mut consecutive = 0u32;
    let mut next_ready = Instant::now();
    tracing::info!("Terra writer loop started");
    loop {
        let wait = next_ready.saturating_duration_since(Instant::now());
        tokio::select! {
            _ = stop.changed() => {
                if *stop.borrow() {
                    tracing::info!("Terra writer loop shutting down");
                    return;
                }
            }
            _ = tokio::time::sleep(wait) => {
                match writer.process_pending().await {
                    Ok(()) => {
                        consecutive = 0;
                        next_ready = Instant::now() + next_writer_delay(true, 0, &schedule, 0);
                    }
                    Err(e) => {
                        consecutive = consecutive.saturating_add(1);
                        let delay = next_writer_delay(false, consecutive, &schedule, 0x54EA ^ consecutive as u64);
                        tracing::error!(
                            error = %e,
                            consecutive,
                            backoff_ms = delay.as_millis() as u64,
                            "Terra writer cycle failed; backing off this chain only"
                        );
                        next_ready = Instant::now() + delay;
                    }
                }
            }
        }
    }
}

/// Writer health status
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub evm_healthy: bool,
    pub terra_healthy: bool,
    pub evm_to_evm_healthy: bool,
    pub evm_pending_executions: usize,
    pub terra_pending_executions: usize,
    pub multi_evm_chains: usize,
}

#[cfg(test)]
mod schedule_tests {
    use super::*;
    use std::time::Duration;

    fn sched() -> WriterScheduleConfig {
        WriterScheduleConfig {
            poll_interval: Duration::from_secs(5),
            rpc_backoff_initial: Duration::from_secs(2),
            rpc_backoff_max: Duration::from_secs(8),
            jitter_bps: 0,
            negative_retry_initial: Duration::from_secs(5),
            negative_retry_max: Duration::from_secs(300),
            negative_retry_ttl: Duration::from_secs(86400),
            negative_retry_cache_size: 16,
            max_verify_per_cycle: 64,
        }
    }

    #[test]
    fn success_uses_poll_interval() {
        assert_eq!(
            next_writer_delay(true, 0, &sched(), 1),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn failure_uses_exponential_backoff_capped() {
        let s = sched();
        assert_eq!(next_writer_delay(false, 1, &s, 1), Duration::from_secs(2));
        assert_eq!(next_writer_delay(false, 2, &s, 1), Duration::from_secs(4));
        assert_eq!(next_writer_delay(false, 10, &s, 1), Duration::from_secs(8));
    }

    #[test]
    fn remaining_cancel_window_elapsed_is_zero() {
        assert_eq!(remaining_cancel_window_secs(1_000, 300, 1_300), 0);
        assert_eq!(remaining_cancel_window_secs(1_000, 300, 1_400), 0);
        assert_eq!(remaining_cancel_window_secs(0, 300, 5_000), 0);
    }

    #[test]
    fn remaining_cancel_window_still_active() {
        assert_eq!(remaining_cancel_window_secs(1_000, 300, 1_100), 200);
    }

    #[test]
    fn terminal_execute_already_executed() {
        assert!(is_terminal_execute_error(
            "Withdrawal 0xabc already executed"
        ));
        assert!(is_terminal_execute_error(
            "Failed to execute withdraw (lock_unlock): WithdrawAlreadyExecuted"
        ));
        assert!(is_terminal_execute_error("Withdrawal 0xabc was cancelled"));
        assert!(is_terminal_execute_error(
            "Withdrawal 0xabc not found (submittedAt is zero)"
        ));
    }

    #[test]
    fn terminal_execute_below_min_selector() {
        assert!(is_terminal_execute_error(
            "Failed to send withdraw tx: server returned an error response: error code 3: execution reverted, data: \"0x1913498b000000000000000000000000000000000000000000000000016345785d8a0000\""
        ));
        assert!(is_terminal_execute_error(
            "execution reverted, data: \"0xe744b3ba0000000000000000000000000000000000000000000000000000000000000000\""
        ));
        assert!(is_terminal_execute_error(
            "ContractError::BelowMinimumAmount { min_amount: \"100000\" }"
        ));
    }

    #[test]
    fn retryable_execute_rpc_and_cancel_window_are_not_terminal() {
        assert!(!is_terminal_execute_error(
            "Failed to send withdraw tx: HTTP error 429"
        ));
        assert!(!is_terminal_execute_error("CancelWindowActive"));
        assert!(!is_terminal_execute_error(
            "Failed to get pending withdraw: timeout"
        ));
        assert!(!is_terminal_execute_error("Terra deposit not found"));
        assert!(!is_terminal_execute_error("RateLimitExceededPerPeriod"));
        assert!(!is_terminal_execute_error(
            "Failed to send withdraw tx: RateLimitExceededPerTx"
        ));
    }
}
