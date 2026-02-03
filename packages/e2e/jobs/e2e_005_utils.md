---
output_dir: src/
output_file: utils.rs
context_files:
  - src/config.rs
verify: true
depends_on:
  - e2e_002_config
---

# Utility Functions Module

## Requirements

Create utility functions for E2E testing including:
- Polling/waiting utilities
- Address encoding helpers
- Environment file operations

## Polling Utilities

Replace bash `wait_for_service()` pattern:

```rust
use eyre::Result;
use std::future::Future;
use std::time::Duration;
use tokio::time::{sleep, Instant};

/// Poll a check function until it returns true or timeout
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
                tracing::info!("{} ready", description);
                return Ok(());
            }
            Ok(false) => {
                tracing::debug!("{} not ready, retrying...", description);
            }
            Err(e) => {
                tracing::debug!("{} check failed: {}, retrying...", description, e);
            }
        }
        sleep(interval).await;
    }
    eyre::bail!("{} did not become ready within {:?}", description, timeout)
}
```

## Retry Utilities

```rust
/// Retry an operation with exponential backoff
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
    // Implement exponential backoff: delay * 2^attempt
}
```

## Address Encoding

For Terra addresses that need to be encoded as bytes32:

```rust
use alloy::primitives::B256;

/// Encode a Terra address (bech32) as bytes32 for EVM
/// Pads with zeros on the right
pub fn encode_terra_address(address: &str) -> Result<B256> {
    let bytes = address.as_bytes();
    if bytes.len() > 32 {
        eyre::bail!("Terra address too long: {} bytes", bytes.len());
    }
    let mut result = [0u8; 32];
    result[..bytes.len()].copy_from_slice(bytes);
    Ok(B256::from(result))
}

/// Decode bytes32 back to Terra address
pub fn decode_terra_address(bytes: &B256) -> Result<String> {
    let trimmed = bytes.as_slice()
        .iter()
        .take_while(|&&b| b != 0)
        .copied()
        .collect::<Vec<_>>();
    String::from_utf8(trimmed)
        .map_err(|e| eyre::eyre!("Invalid UTF-8 in address: {}", e))
}
```

## Environment File Operations

Replace bash `export_env_file()`:

```rust
use std::path::Path;
use std::collections::HashMap;

/// Write environment variables to a .env file
pub fn write_env_file(path: &Path, vars: &HashMap<String, String>) -> Result<()> {
    let mut content = String::from("# E2E Test Environment\n");
    content.push_str(&format!("# Generated at {}\n\n", chrono::Utc::now()));
    
    for (key, value) in vars {
        content.push_str(&format!("{}={}\n", key, value));
    }
    
    std::fs::write(path, content)?;
    Ok(())
}

/// Read environment variables from a .env file
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
pub fn load_env_file(path: &Path) -> Result<()> {
    let vars = read_env_file(path)?;
    for (key, value) in vars {
        std::env::set_var(key, value);
    }
    Ok(())
}
```

## Constraints

- No `.unwrap()` calls
- Use `eyre::Result` for all errors
- Use `tracing` for logging
- All async functions should be Send + Sync
