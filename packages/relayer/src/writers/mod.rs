use eyre::Result;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::mpsc;

pub mod evm;
pub mod terra;

pub use evm::EvmWriter;
pub use terra::TerraWriter;

/// Retry configuration for transaction submission
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier for exponential growth
    pub backoff_multiplier: f64,
    /// Circuit breaker: consecutive failures before pausing
    pub circuit_breaker_threshold: u32,
    /// How long to pause when circuit breaker trips
    pub circuit_breaker_pause: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            circuit_breaker_threshold: 10,
            circuit_breaker_pause: Duration::from_secs(300), // 5 minutes
        }
    }
}

#[allow(dead_code)]
impl RetryConfig {
    /// Calculate backoff duration for a given attempt
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let backoff_secs = self.initial_backoff.as_secs_f64()
            * self.backoff_multiplier.powi(attempt as i32);
        let capped = backoff_secs.min(self.max_backoff.as_secs_f64());
        Duration::from_secs_f64(capped)
    }

    /// Check if we should retry based on attempt count
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Manages transaction writers for both chains
pub struct WriterManager {
    evm_writer: EvmWriter,
    terra_writer: TerraWriter,
    retry_config: RetryConfig,
    consecutive_evm_failures: u32,
    consecutive_terra_failures: u32,
}

impl WriterManager {
    /// Create a new writer manager
    pub async fn new(config: &crate::config::Config, db: PgPool) -> Result<Self> {
        let evm_writer = EvmWriter::new(&config.evm, &config.fees, db.clone()).await?;
        let terra_writer = TerraWriter::new(&config.terra, db).await?;
        
        Ok(Self {
            evm_writer,
            terra_writer,
            retry_config: RetryConfig::default(),
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
        if self.consecutive_evm_failures >= self.retry_config.circuit_breaker_threshold {
            tracing::warn!(
                failures = self.consecutive_evm_failures,
                pause_secs = self.retry_config.circuit_breaker_pause.as_secs(),
                "EVM circuit breaker tripped, pausing EVM writer"
            );
            tokio::time::sleep(self.retry_config.circuit_breaker_pause).await;
            self.consecutive_evm_failures = 0;
        }
        
        // Process pending approvals (Terra -> EVM)
        match self.evm_writer.process_pending().await {
            Ok(()) => {
                self.consecutive_evm_failures = 0;
            }
            Err(e) => {
                self.consecutive_evm_failures += 1;
                let backoff = self.retry_config.backoff_for_attempt(self.consecutive_evm_failures);
                tracing::error!(
                    error = %e,
                    consecutive_failures = self.consecutive_evm_failures,
                    next_backoff_secs = backoff.as_secs(),
                    "Error processing EVM approvals, will retry with backoff"
                );
                tokio::time::sleep(backoff).await;
            }
        }
        
        // Check Terra circuit breaker
        if self.consecutive_terra_failures >= self.retry_config.circuit_breaker_threshold {
            tracing::warn!(
                failures = self.consecutive_terra_failures,
                pause_secs = self.retry_config.circuit_breaker_pause.as_secs(),
                "Terra circuit breaker tripped, pausing Terra writer"
            );
            tokio::time::sleep(self.retry_config.circuit_breaker_pause).await;
            self.consecutive_terra_failures = 0;
        }
        
        // Process pending releases (EVM -> Terra)
        match self.terra_writer.process_pending().await {
            Ok(()) => {
                self.consecutive_terra_failures = 0;
            }
            Err(e) => {
                self.consecutive_terra_failures += 1;
                let backoff = self.retry_config.backoff_for_attempt(self.consecutive_terra_failures);
                tracing::error!(
                    error = %e,
                    consecutive_failures = self.consecutive_terra_failures,
                    next_backoff_secs = backoff.as_secs(),
                    "Error processing Terra releases, will retry with backoff"
                );
                tokio::time::sleep(backoff).await;
            }
        }
        
        Ok(())
    }
}