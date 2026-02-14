use alloy::primitives::Address;
use eyre::{Result, WrapErr};
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::types::ChainId;

pub mod evm;
pub mod retry;
pub mod terra;

pub use evm::EvmWriter;
pub use retry::{classify_error, RetryConfig};
pub use terra::TerraWriter;

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
/// - EVM → Terra: `terra_writer.process_pending()` — polls Terra PendingWithdrawals
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

        // Add primary EVM chain (if V2 chain ID is configured)
        if let Some(v2_id) = config.evm.this_chain_id {
            let bridge = Address::from_str(&config.evm.bridge_address)
                .wrap_err("Invalid primary EVM bridge address")?;
            source_chain_endpoints.insert(
                ChainId::from_u32(v2_id).0,
                (config.evm.rpc_url.clone(), bridge),
            );
        }

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

        let evm_writer = EvmWriter::new(
            &config.evm,
            Some(&config.terra),
            &config.fees,
            db.clone(),
            source_chain_endpoints.clone(),
        )
        .await?;
        let terra_writer =
            TerraWriter::new(&config.terra, source_chain_endpoints.clone(), db.clone()).await?;

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
        })
    }

    /// Run all writers concurrently
    /// Processes pending approvals and releases
    pub async fn run(&mut self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        let poll_interval = Duration::from_millis(5000);
        let mut cycle_count = 0u64;

        tracing::info!(
            poll_interval_ms = poll_interval.as_millis() as u64,
            "Writer manager starting poll loop"
        );

        loop {
            cycle_count += 1;

            // Log every 12 cycles (~60 seconds) to show the writer is alive
            if cycle_count % 12 == 1 {
                let multi_evm_pending: usize = self
                    .evm_chain_writers
                    .values()
                    .map(|w| w.pending_execution_count())
                    .sum();
                tracing::info!(
                    cycle = cycle_count,
                    evm_failures = self.consecutive_evm_failures,
                    terra_failures = self.consecutive_terra_failures,
                    evm_to_evm_failures = self.consecutive_evm_to_evm_failures,
                    evm_pending_executions = self.evm_writer.pending_execution_count(),
                    terra_pending_executions = self.terra_writer.pending_execution_count(),
                    multi_evm_chains = self.evm_chain_writers.len(),
                    multi_evm_pending = multi_evm_pending,
                    "Writer manager heartbeat"
                );
            }

            tokio::select! {
                _ = self.process_pending() => {}
                _ = shutdown.recv() => {
                    tracing::info!("Shutdown signal received, stopping writers");
                    return Ok(());
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

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
