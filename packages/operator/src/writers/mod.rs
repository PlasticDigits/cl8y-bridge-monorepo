use eyre::Result;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::mpsc;

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

/// Manages transaction writers for both chains
pub struct WriterManager {
    evm_writer: EvmWriter,
    terra_writer: TerraWriter,
    retry_config: RetryConfig,
    circuit_breaker: CircuitBreakerConfig,
    consecutive_evm_failures: u32,
    consecutive_terra_failures: u32,
}

impl WriterManager {
    /// Create a new writer manager
    pub async fn new(config: &crate::config::Config, db: PgPool) -> Result<Self> {
        let evm_writer = EvmWriter::new(&config.evm, &config.fees, db.clone()).await?;
        let terra_writer = TerraWriter::new(&config.terra, &config.evm, db).await?;

        Ok(Self {
            evm_writer,
            terra_writer,
            retry_config: RetryConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            consecutive_evm_failures: 0,
            consecutive_terra_failures: 0,
        })
    }

    /// Run all writers concurrently
    /// Processes pending approvals and releases
    pub async fn run(&mut self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        let poll_interval = Duration::from_millis(5000);

        loop {
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

        // Process pending approvals (Terra -> EVM)
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
                    "Error processing EVM approvals, will retry with backoff"
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

        // Process pending releases (EVM -> Terra)
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

        Ok(())
    }

    /// Get health status
    #[allow(dead_code)]
    pub fn health_status(&self) -> HealthStatus {
        HealthStatus {
            evm_healthy: self.consecutive_evm_failures < self.circuit_breaker.threshold,
            terra_healthy: self.consecutive_terra_failures < self.circuit_breaker.threshold,
            evm_pending_executions: self.evm_writer.pending_execution_count(),
            terra_pending_executions: self.terra_writer.pending_execution_count(),
        }
    }
}

/// Writer health status
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub evm_healthy: bool,
    pub terra_healthy: bool,
    pub evm_pending_executions: usize,
    pub terra_pending_executions: usize,
}
