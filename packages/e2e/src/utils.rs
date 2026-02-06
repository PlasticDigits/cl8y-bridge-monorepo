//! Utility functions for E2E testing
//!
//! This module provides polling, retry, address encoding, and environment file
//! operations for E2E test infrastructure.

use alloy::primitives::B256;
use chrono::Utc;
use eyre::Result;
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{debug, info, warn};

/// Poll a check function until it returns true or timeout
///
/// # Arguments
/// * `description` - Human-readable description of the service being checked
/// * `check` - Async function that returns `Result<bool>` indicating health
/// * `timeout` - Maximum time to wait for the check to succeed
/// * `interval` - Time to wait between check attempts
///
/// # Example
/// ```no_run
/// # async fn example() -> eyre::Result<()> {
/// use std::time::Duration;
/// use crate::utils::poll_until;
///
/// poll_until(
///     "PostgreSQL",
///     || async { Ok(true) },
///     Duration::from_secs(30),
///     Duration::from_secs(2),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn poll_until<F, Fut>(
    description: &str,
    check: F,
    timeout: Duration,
    interval: Duration,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<bool>>,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        match check().await {
            Ok(true) => {
                info!("{} ready", description);
                return Ok(());
            }
            Ok(false) => {
                debug!("{} not ready, retrying...", description);
            }
            Err(e) => {
                debug!("{} check failed: {}, retrying...", description, e);
            }
        }
        sleep(interval).await;
    }
    eyre::bail!("{} did not become ready within {:?}", description, timeout)
}

/// Retry an operation with exponential backoff
///
/// # Arguments
/// * `operation` - Human-readable description of the operation being retried
/// * `f` - Async function that returns `Result<T>`
/// * `max_attempts` - Maximum number of retry attempts
/// * `initial_delay` - Initial delay between attempts
///
/// # Example
/// ```no_run
/// # async fn example() -> eyre::Result<()> {
/// use std::time::Duration;
/// use crate::utils::retry_with_backoff;
///
/// let result = retry_with_backoff(
///     "Connect to RPC",
///     || async { Ok(true) },
///     5,
///     Duration::from_millis(100),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn retry_with_backoff<F, Fut, T>(
    operation: &str,
    f: F,
    max_attempts: u32,
    initial_delay: Duration,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut delay = initial_delay;
    let mut attempt = 1u32;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt >= max_attempts {
                    eyre::bail!(
                        "Operation '{}' failed after {} attempts: {}",
                        operation,
                        max_attempts,
                        e
                    );
                }

                warn!(
                    "Operation '{}' failed (attempt {}/{}): {}. Retrying in {:?}...",
                    operation, attempt, max_attempts, e, delay
                );

                sleep(delay).await;
                delay = delay.saturating_mul(2);
                attempt += 1;
            }
        }
    }
}

/// Encode a Terra address (bech32) as bytes32 for EVM
///
/// Uses the unified encoding from multichain-rs: bech32 decode → raw 20 bytes → left-pad to 32 bytes.
/// This matches the Terra contract's `encode_terra_address(deps, &addr)` which uses
/// `addr_canonicalize` → left-pad, ensuring consistent hash computation across chains.
///
/// # Arguments
/// * `address` - Terra address in bech32 format
///
/// # Example
/// ```no_run
/// # use alloy::primitives::B256;
/// # use crate::utils::encode_terra_address;
/// let bytes = encode_terra_address("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v")?;
/// # Ok::<(), eyre::Error>(())
/// ```
pub fn encode_terra_address(address: &str) -> Result<B256> {
    let raw = multichain_rs::hash::encode_terra_address_to_bytes32(address)
        .map_err(|e| eyre::eyre!("Failed to encode Terra address: {}", e))?;
    Ok(B256::from(raw))
}

/// Decode bytes32 back to Terra address
///
/// Uses the unified decoding from multichain-rs: extracts the left-padded 20-byte
/// address from bytes32 and encodes it as a bech32 Terra address.
/// This is the inverse of `encode_terra_address`.
///
/// # Arguments
/// * `bytes` - 32-byte representation of a Terra address (left-padded)
pub fn decode_terra_address(bytes: &B256) -> Result<String> {
    let raw: [u8; 32] = bytes.0;
    multichain_rs::hash::decode_bytes32_to_terra_address(&raw)
        .map_err(|e| eyre::eyre!("Failed to decode Terra address: {}", e))
}

/// Write environment variables to a .env file
///
/// # Arguments
/// * `path` - Path where the .env file should be written
/// * `vars` - Map of environment variable names to values
///
/// # Example
/// ```no_run
/// # use std::collections::HashMap;
/// # use std::path::Path;
/// # use crate::utils::write_env_file;
/// let mut vars = HashMap::new();
/// vars.insert("DATABASE_URL".to_string(), "postgres://localhost".to_string());
/// write_env_file(Path::new(".env"), &vars)?;
/// # Ok::<(), eyre::Error>(())
/// ```
pub fn write_env_file(path: &Path, vars: &HashMap<String, String>) -> Result<()> {
    let mut content = String::from("# E2E Test Environment\n");
    content.push_str(&format!("# Generated at {}\n\n", Utc::now()));

    for (key, value) in vars {
        content.push_str(&format!("{}={}\n", key, value));
    }

    std::fs::write(path, content)?;
    Ok(())
}

/// Read environment variables from a .env file
///
/// Lines starting with `#` are treated as comments. Empty lines are ignored.
/// The file format follows the standard `.env` specification.
///
/// # Arguments
/// * `path` - Path to the .env file
///
/// # Example
/// ```no_run
/// # use std::path::Path;
/// # use crate::utils::read_env_file;
/// let vars = read_env_file(Path::new(".env"))?;
/// # Ok::<(), eyre::Error>(())
/// ```
pub fn read_env_file(path: &Path) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)?;
    let mut vars = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            vars.insert(key.to_string(), value.to_string());
        }
    }

    Ok(vars)
}

/// Load .env file into process environment
///
/// Reads a .env file and sets each variable in the current process's
/// environment. This is useful for loading test configuration without
/// restarting the process.
///
/// # Arguments
/// * `path` - Path to the .env file to load
///
/// # Example
/// ```no_run
/// # use std::path::Path;
/// # use crate::utils::load_env_file;
/// load_env_file(Path::new(".env"))?;
/// # Ok::<(), eyre::Error>(())
/// ```
pub fn load_env_file(path: &Path) -> Result<()> {
    let vars = read_env_file(path)?;
    for (key, value) in vars {
        std::env::set_var(key, value);
    }
    Ok(())
}
