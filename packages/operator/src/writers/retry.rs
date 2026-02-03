//! Retry and error recovery utilities for transaction submission
//!
//! Provides exponential backoff, gas bumping, and dead letter queue functionality.

use chrono::{DateTime, Utc};
use eyre::{eyre, Result};
use std::time::Duration;
use tracing::{debug, warn};

/// Transaction retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier for exponential growth
    pub backoff_multiplier: f64,
    /// Gas price bump percentage per retry
    pub gas_bump_percent: u32,
    /// Maximum gas price multiplier (e.g., 3 = 3x original)
    pub max_gas_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::from_secs(2),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            gas_bump_percent: 20, // 20% gas increase per retry
            max_gas_multiplier: 3.0,
        }
    }
}

impl RetryConfig {
    /// Calculate backoff duration for a given attempt (0-indexed)
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let backoff_secs =
            self.initial_backoff.as_secs_f64() * self.backoff_multiplier.powi(attempt as i32);
        let capped = backoff_secs.min(self.max_backoff.as_secs_f64());
        Duration::from_secs_f64(capped)
    }

    /// Check if we should retry based on attempt count
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    /// Calculate gas price for a given attempt
    pub fn gas_price_for_attempt(&self, base_gas_price: u128, attempt: u32) -> u128 {
        if attempt == 0 {
            return base_gas_price;
        }

        let multiplier = 1.0 + (self.gas_bump_percent as f64 / 100.0) * (attempt as f64);
        let capped_multiplier = multiplier.min(self.max_gas_multiplier);

        (base_gas_price as f64 * capped_multiplier) as u128
    }

    /// Calculate the next retry time
    pub fn next_retry_after(&self, attempt: u32) -> DateTime<Utc> {
        let backoff = self.backoff_for_attempt(attempt);
        Utc::now() + chrono::Duration::from_std(backoff).unwrap_or(chrono::Duration::seconds(60))
    }

    /// Check if a transaction is ready for retry based on retry_after
    pub fn is_ready_for_retry(&self, retry_after: Option<DateTime<Utc>>) -> bool {
        match retry_after {
            Some(time) => Utc::now() >= time,
            None => true,
        }
    }
}

/// Classifies errors for retry decisions
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorClass {
    /// Temporary failure - should retry (RPC timeout, network issues)
    Transient,
    /// Transaction underpriced - retry with higher gas
    Underpriced,
    /// Nonce too low - skip (already processed)
    NonceTooLow,
    /// Nonce too high - wait for pending transactions
    NonceTooHigh,
    /// Permanent failure - do not retry (invalid params, contract error)
    Permanent,
    /// Unknown error - may retry with backoff
    Unknown,
}

/// Classify an error for retry decisions
pub fn classify_error(error: &str) -> ErrorClass {
    let error_lower = error.to_lowercase();

    // Transient errors
    if error_lower.contains("timeout")
        || error_lower.contains("connection")
        || error_lower.contains("network")
        || error_lower.contains("rate limit")
        || error_lower.contains("too many requests")
        || error_lower.contains("503")
        || error_lower.contains("502")
        || error_lower.contains("temporarily unavailable")
    {
        return ErrorClass::Transient;
    }

    // Gas price errors
    if error_lower.contains("underpriced")
        || error_lower.contains("replacement transaction")
        || error_lower.contains("gas price too low")
        || error_lower.contains("max fee per gas less than")
    {
        return ErrorClass::Underpriced;
    }

    // Nonce errors
    if error_lower.contains("nonce too low")
        || error_lower.contains("already known")
        || error_lower.contains("already been processed")
    {
        return ErrorClass::NonceTooLow;
    }

    if error_lower.contains("nonce too high") {
        return ErrorClass::NonceTooHigh;
    }

    // Permanent errors
    if error_lower.contains("reverted")
        || error_lower.contains("execution reverted")
        || error_lower.contains("invalid signature")
        || error_lower.contains("insufficient funds")
        || error_lower.contains("out of gas")
        || error_lower.contains("invalid parameters")
        || error_lower.contains("approval not found")
        || error_lower.contains("already cancelled")
        || error_lower.contains("already executed")
    {
        return ErrorClass::Permanent;
    }

    ErrorClass::Unknown
}

/// Retry context for a transaction
#[derive(Debug, Clone)]
pub struct RetryContext {
    pub config: RetryConfig,
    pub attempt: u32,
    pub last_error: Option<String>,
    pub last_gas_price: Option<u128>,
}

impl RetryContext {
    pub fn new() -> Self {
        Self::with_config(RetryConfig::default())
    }

    pub fn with_config(config: RetryConfig) -> Self {
        Self {
            config,
            attempt: 0,
            last_error: None,
            last_gas_price: None,
        }
    }

    /// Record a failed attempt
    pub fn record_failure(&mut self, error: String, gas_price: Option<u128>) {
        self.attempt += 1;
        self.last_error = Some(error);
        self.last_gas_price = gas_price;
    }

    /// Get the decision for the next attempt
    pub fn next_action(&self) -> RetryAction {
        let error = self.last_error.as_deref().unwrap_or("");
        let error_class = classify_error(error);

        match error_class {
            ErrorClass::Permanent => {
                warn!(error = %error, "Permanent error - adding to dead letter queue");
                RetryAction::DeadLetter
            }
            ErrorClass::NonceTooLow => {
                debug!("Nonce too low - transaction already processed, skipping");
                RetryAction::Skip
            }
            ErrorClass::NonceTooHigh => {
                // Wait longer for pending transactions to clear
                let backoff = self.config.max_backoff;
                debug!(
                    ?backoff,
                    "Nonce too high - waiting for pending transactions"
                );
                RetryAction::RetryAfter(backoff)
            }
            ErrorClass::Underpriced => {
                if !self.config.should_retry(self.attempt) {
                    return RetryAction::DeadLetter;
                }
                let new_gas = self.config.gas_price_for_attempt(
                    self.last_gas_price.unwrap_or(1_000_000_000), // 1 gwei default
                    self.attempt,
                );
                let backoff = Duration::from_secs(1); // Retry quickly with bumped gas
                debug!(new_gas, "Underpriced - retrying with bumped gas");
                RetryAction::RetryWithGas {
                    backoff,
                    gas_price: new_gas,
                }
            }
            ErrorClass::Transient | ErrorClass::Unknown => {
                if !self.config.should_retry(self.attempt) {
                    return RetryAction::DeadLetter;
                }
                let backoff = self.config.backoff_for_attempt(self.attempt);
                debug!(
                    ?backoff,
                    attempt = self.attempt,
                    "Transient error - retrying"
                );
                RetryAction::RetryAfter(backoff)
            }
        }
    }

    /// Reset for a new transaction
    pub fn reset(&mut self) {
        self.attempt = 0;
        self.last_error = None;
        self.last_gas_price = None;
    }
}

impl Default for RetryContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Action to take after a failed attempt
#[derive(Debug, Clone)]
pub enum RetryAction {
    /// Retry after a backoff period
    RetryAfter(Duration),
    /// Retry with a new gas price
    RetryWithGas { backoff: Duration, gas_price: u128 },
    /// Skip this transaction (already processed)
    Skip,
    /// Move to dead letter queue (permanent failure)
    DeadLetter,
}

/// Execute with retry logic
pub async fn with_retry<F, T, Fut>(config: &RetryConfig, mut operation: F) -> Result<T>
where
    F: FnMut(u32, Option<u128>) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut ctx = RetryContext::with_config(config.clone());

    loop {
        let gas_price = ctx.last_gas_price;

        match operation(ctx.attempt, gas_price).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let error_str = e.to_string();
                ctx.record_failure(error_str.clone(), gas_price);

                match ctx.next_action() {
                    RetryAction::RetryAfter(backoff) => {
                        warn!(
                            attempt = ctx.attempt,
                            max = config.max_retries,
                            ?backoff,
                            error = %error_str,
                            "Retrying after backoff"
                        );
                        tokio::time::sleep(backoff).await;
                    }
                    RetryAction::RetryWithGas {
                        backoff,
                        gas_price: new_gas,
                    } => {
                        warn!(
                            attempt = ctx.attempt,
                            old_gas = ?gas_price,
                            new_gas,
                            "Retrying with bumped gas"
                        );
                        ctx.last_gas_price = Some(new_gas);
                        tokio::time::sleep(backoff).await;
                    }
                    RetryAction::Skip => {
                        debug!("Skipping transaction");
                        return Err(eyre!("Skipped: {}", error_str));
                    }
                    RetryAction::DeadLetter => {
                        warn!(error = %error_str, "Moving to dead letter queue");
                        return Err(eyre!("Dead letter: {}", error_str));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_calculation() {
        let config = RetryConfig::default();

        assert_eq!(config.backoff_for_attempt(0), Duration::from_secs(2));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_secs(4));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_secs(8));
        assert_eq!(config.backoff_for_attempt(3), Duration::from_secs(16));
        assert_eq!(config.backoff_for_attempt(4), Duration::from_secs(32));
        assert_eq!(config.backoff_for_attempt(5), Duration::from_secs(60)); // capped
    }

    #[test]
    fn test_gas_bump() {
        let config = RetryConfig::default();
        let base = 1_000_000_000u128; // 1 gwei

        assert_eq!(config.gas_price_for_attempt(base, 0), base);
        assert_eq!(config.gas_price_for_attempt(base, 1), 1_200_000_000); // +20%
        assert_eq!(config.gas_price_for_attempt(base, 2), 1_400_000_000); // +40%
        assert_eq!(config.gas_price_for_attempt(base, 10), 3_000_000_000); // capped at 3x
    }

    #[test]
    fn test_error_classification() {
        assert_eq!(classify_error("connection timeout"), ErrorClass::Transient);
        assert_eq!(
            classify_error("replacement transaction underpriced"),
            ErrorClass::Underpriced
        );
        assert_eq!(classify_error("nonce too low"), ErrorClass::NonceTooLow);
        assert_eq!(classify_error("execution reverted"), ErrorClass::Permanent);
        assert_eq!(classify_error("some unknown error"), ErrorClass::Unknown);
    }
}
