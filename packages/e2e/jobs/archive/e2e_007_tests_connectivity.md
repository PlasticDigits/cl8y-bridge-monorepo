---
output_dir: src/
output_file: tests.rs
context_files:
  - src/lib.rs
  - src/config.rs
verify: true
depends_on:
  - e2e_002_config
  - e2e_003_types
---

# E2E Test Cases Module

## Requirements

Create test functions that replace bash test functions from `scripts/e2e-test.sh`.
Each test returns a `TestResult`.

## Module Structure

```rust
use crate::{E2eConfig, TestResult};
use alloy::primitives::{Address, U256};
use eyre::Result;
use std::time::{Duration, Instant};
```

## Connectivity Tests

Replace bash `test_evm_connectivity()` and `test_terra_connectivity()`:

```rust
/// Test EVM (Anvil) connectivity
pub async fn test_evm_connectivity(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_connectivity";
    
    // Try to get block number
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(block) => {
            tracing::info!("EVM connected, block: {}", block);
            TestResult::pass(name, start.elapsed())
        }
        Err(e) => {
            TestResult::fail(name, format!("Failed to connect: {}", e), start.elapsed())
        }
    }
}

/// Test Terra (LocalTerra) connectivity
pub async fn test_terra_connectivity(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "terra_connectivity";
    
    // Try to get status from LCD
    match check_terra_connection(&config.terra.lcd_url).await {
        Ok(_) => TestResult::pass(name, start.elapsed()),
        Err(e) => TestResult::fail(name, format!("Failed to connect: {}", e), start.elapsed())
    }
}

/// Test PostgreSQL connectivity
pub async fn test_database_connectivity(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_connectivity";
    
    // This is a placeholder - actual implementation would use sqlx
    // For now, just check if the URL parses
    match url::Url::parse(&config.operator.database_url) {
        Ok(_) => TestResult::pass(name, start.elapsed()),
        Err(e) => TestResult::fail(name, format!("Invalid database URL: {}", e), start.elapsed())
    }
}

async fn check_evm_connection(rpc_url: &url::Url) -> Result<u64> {
    let client = reqwest::Client::new();
    let response = client
        .post(rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        }))
        .send()
        .await?;
    
    let body: serde_json::Value = response.json().await?;
    let hex_block = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;
    
    let block = u64::from_str_radix(hex_block.trim_start_matches("0x"), 16)?;
    Ok(block)
}

async fn check_terra_connection(lcd_url: &url::Url) -> Result<()> {
    let client = reqwest::Client::new();
    let status_url = format!("{}/cosmos/base/tendermint/v1beta1/syncing", lcd_url);
    
    let response = client.get(&status_url).send().await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        eyre::bail!("Terra LCD returned status: {}", response.status())
    }
}
```

## Contract Deployment Tests

```rust
/// Test that EVM contracts are deployed
pub async fn test_evm_contracts_deployed(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_contracts_deployed";
    
    // Check that contract addresses are not zero
    let contracts = &config.evm.contracts;
    
    if contracts.bridge == Address::ZERO {
        return TestResult::fail(name, "Bridge address is zero", start.elapsed());
    }
    if contracts.router == Address::ZERO {
        return TestResult::fail(name, "Router address is zero", start.elapsed());
    }
    if contracts.access_manager == Address::ZERO {
        return TestResult::fail(name, "AccessManager address is zero", start.elapsed());
    }
    
    // TODO: Actually call the contracts to verify they exist
    
    TestResult::pass(name, start.elapsed())
}
```

## Quick Test Suite

```rust
/// Run quick connectivity tests only
pub async fn run_quick_tests(config: &E2eConfig) -> Vec<TestResult> {
    vec![
        test_evm_connectivity(config).await,
        test_terra_connectivity(config).await,
        test_database_connectivity(config).await,
    ]
}
```

## Full Test Suite

```rust
/// Run all E2E tests
pub async fn run_all_tests(config: &E2eConfig, skip_terra: bool) -> Vec<TestResult> {
    let mut results = Vec::new();
    
    // Connectivity tests
    results.push(test_evm_connectivity(config).await);
    if !skip_terra {
        results.push(test_terra_connectivity(config).await);
    }
    results.push(test_database_connectivity(config).await);
    
    // Contract tests
    results.push(test_evm_contracts_deployed(config).await);
    
    // TODO: Add more tests
    // - test_evm_to_terra_transfer
    // - test_terra_to_evm_transfer
    // - test_fraud_detection
    
    results
}
```

## Constraints

- Each test function returns `TestResult`
- Use `Instant::now()` to measure duration
- Log test progress with `tracing`
- Handle all errors gracefully - never panic
- Tests should be independent and idempotent
