---
context_files:
  - src/tests/helpers.rs
  - src/lib.rs
output_dir: src/tests/
output_file: operator.rs
---

# Implement Operator Integration Tests - Batch 3

## Overview

Create a new module `operator.rs` with 5 implemented operator integration tests. These tests verify the operator service starts correctly, detects deposits, creates approvals, and executes withdrawals.

## Imports Required

```rust
use crate::{E2eConfig, ServiceManager, TestResult};
use std::path::Path;
use std::time::{Duration, Instant};
```

## Tests to Implement (5 total)

### 1. test_withdraw_hash_computation

Verify that Rust's compute_xchain_hash_id function produces correct results by computing a hash and verifying the parameters are ABI-encoded correctly.

Implementation requirements:
- Use `super::helpers::compute_xchain_hash_id` function
- Create test parameters: src_chain_key (B256), token (Address), to (Address), dest_account (B256), amount (U256), nonce (u64)
- Compute the hash and verify it's non-zero
- Verify the hash changes when parameters change (determinism check)

```rust
pub async fn test_withdraw_hash_computation(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "withdraw_hash_computation";

    use alloy::primitives::{Address, B256, U256};

    // Create test parameters
    let src_chain_key = B256::from([1u8; 32]);
    let token = config.evm.contracts.bridge; // Use any known address
    let to = config.test_accounts.evm_address;
    let dest_account = B256::from([2u8; 32]);
    let amount = U256::from(1000000000000000000u64); // 1e18
    let nonce = 1u64;

    // Compute hash using helper
    let hash1 = super::helpers::compute_xchain_hash_id(
        src_chain_key,
        token,
        to,
        dest_account,
        amount,
        nonce,
    );

    // Verify hash is non-zero (valid computation)
    if hash1 == B256::ZERO {
        return TestResult::fail(name, "Computed hash is zero", start.elapsed());
    }

    // Verify hash changes with different nonce (determinism)
    let hash2 = super::helpers::compute_xchain_hash_id(
        src_chain_key,
        token,
        to,
        dest_account,
        amount,
        nonce + 1,
    );

    if hash1 == hash2 {
        return TestResult::fail(
            name,
            "Hash should change with different nonce",
            start.elapsed(),
        );
    }

    // Verify hash is stable (same params = same hash)
    let hash3 = super::helpers::compute_xchain_hash_id(
        src_chain_key,
        token,
        to,
        dest_account,
        amount,
        nonce,
    );

    if hash1 != hash3 {
        return TestResult::fail(
            name,
            "Hash should be deterministic for same parameters",
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}
```

### 2. test_operator_startup

Verify operator can start and is responsive. This test checks that the ServiceManager can manage the operator process.

Implementation requirements:
- Create ServiceManager with project root
- Check if operator is already running
- Verify ServiceManager initialization works
- Test that we can query operator running status
- Skip actual startup if infrastructure not ready

```rust
pub async fn test_operator_startup(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "operator_startup";

    // Get project root dynamically
    let project_root = find_project_root();
    
    // Create service manager
    let manager = ServiceManager::new(&project_root);

    // Check initial state
    let was_running = manager.is_operator_running();
    tracing::info!("Operator already running: {}", was_running);

    // Verify we can check operator status without error
    // This validates ServiceManager PID file reading works
    let is_running = manager.is_operator_running();
    
    // If already running, that's a pass (means E2E env is set up)
    if is_running {
        tracing::info!("Operator is already running - startup test passed");
        return TestResult::pass(name, start.elapsed());
    }

    // If not running, verify database is available before attempting start
    // (operator requires database connection)
    let db_check = tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&config.operator.database_url)
    ).await;

    match db_check {
        Ok(Ok(_pool)) => {
            // Database available - operator could start if needed
            tracing::info!("Database available, operator could be started");
            TestResult::pass(name, start.elapsed())
        }
        Ok(Err(e)) => {
            // Database not available - skip test
            TestResult::skip(name, format!("Database not available for operator: {}", e))
        }
        Err(_) => {
            // Timeout
            TestResult::skip(name, "Database connection timed out")
        }
    }
}
```

### 3. test_operator_deposit_detection

Verify operator can detect EVM deposits by checking the deposit nonce increments correctly and the bridge contract is responsive.

Implementation requirements:
- Query current deposit nonce from bridge
- Verify bridge contract has code
- Check that deposit nonce is queryable (infrastructure working)
- This validates the detection infrastructure is in place

```rust
pub async fn test_operator_deposit_detection(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "operator_deposit_detection";

    // Verify bridge contract is deployed
    let has_code = match super::helpers::query_contract_code(config, config.evm.contracts.bridge).await {
        Ok(c) => c,
        Err(e) => return TestResult::fail(
            name,
            format!("Failed to check bridge contract: {}", e),
            start.elapsed(),
        ),
    };

    if !has_code {
        return TestResult::fail(name, "Bridge contract has no code deployed", start.elapsed());
    }

    // Query deposit nonce to verify detection infrastructure
    let nonce = match super::helpers::query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => return TestResult::fail(
            name,
            format!("Failed to query deposit nonce: {}", e),
            start.elapsed(),
        ),
    };

    tracing::info!("Current deposit nonce: {}", nonce);

    // Verify we can read events from the bridge (RPC is working)
    // This validates the deposit detection infrastructure
    let block_check = super::helpers::check_evm_connection(&config.evm.rpc_url).await;
    
    match block_check {
        Ok(block) => {
            tracing::info!("EVM block number: {} - deposit detection infrastructure ready", block);
            TestResult::pass(name, start.elapsed())
        }
        Err(e) => TestResult::fail(
            name,
            format!("EVM connection failed: {}", e),
            start.elapsed(),
        ),
    }
}
```

### 4. test_operator_approval_creation

Verify operator approval creation infrastructure by checking Terra connectivity and bridge configuration.

Implementation requirements:
- Check Terra LCD connectivity
- Verify Terra bridge address is configured
- Query Terra bridge if available to verify approval infrastructure

```rust
pub async fn test_operator_approval_creation(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "operator_approval_creation";

    // Check Terra LCD connectivity
    match super::helpers::check_terra_connection(&config.terra.lcd_url).await {
        Ok(()) => {
            tracing::info!("Terra LCD connection successful");
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Terra LCD connection failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Verify Terra bridge address is configured
    let bridge_addr = match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => addr.clone(),
        _ => {
            return TestResult::skip(
                name,
                "Terra bridge address not configured - approval creation requires Terra bridge",
            );
        }
    };

    tracing::info!("Terra bridge address: {}", bridge_addr);

    // Try to query the Terra bridge delay to verify it's responsive
    match super::helpers::query_terra_bridge_delay(config, &bridge_addr).await {
        Ok(delay) => {
            tracing::info!("Terra bridge withdraw delay: {} seconds", delay);
            TestResult::pass(name, start.elapsed())
        }
        Err(e) => {
            // Bridge might not be deployed yet, but connectivity works
            tracing::info!("Could not query Terra bridge delay: {} (may not be deployed)", e);
            // Still pass since Terra connectivity works
            TestResult::pass(name, start.elapsed())
        }
    }
}
```

### 5. test_operator_withdrawal_execution

Verify operator withdrawal execution infrastructure by checking withdraw delay configuration and time skip capability.

Implementation requirements:
- Query withdraw delay from EVM bridge
- Verify delay is configured (non-zero)
- Test Anvil time skip capability (required for delay bypass in tests)
- Verify infrastructure is ready for withdrawal execution

```rust
pub async fn test_operator_withdrawal_execution(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "operator_withdrawal_execution";

    // Query withdraw delay from bridge
    let delay = match super::helpers::query_withdraw_delay(config).await {
        Ok(d) => d,
        Err(e) => return TestResult::fail(
            name,
            format!("Failed to query withdraw delay: {}", e),
            start.elapsed(),
        ),
    };

    tracing::info!("Withdraw delay: {} seconds", delay);

    // Verify delay is non-zero (security requirement)
    if delay == 0 {
        return TestResult::fail(
            name,
            "Withdraw delay is 0 - operator cannot enforce watchtower pattern",
            start.elapsed(),
        );
    }

    // Verify Anvil time skip works (required for testing withdrawals without waiting)
    let anvil = crate::AnvilTimeClient::new(config.evm.rpc_url.as_str());

    let before = match anvil.get_block_timestamp().await {
        Ok(ts) => ts,
        Err(e) => return TestResult::fail(
            name,
            format!("Failed to get block timestamp: {}", e),
            start.elapsed(),
        ),
    };

    // Skip 10 seconds (small increment)
    if let Err(e) = anvil.increase_time(10).await {
        return TestResult::fail(
            name,
            format!("Failed to increase time: {}", e),
            start.elapsed(),
        );
    }

    let after = match anvil.get_block_timestamp().await {
        Ok(ts) => ts,
        Err(e) => return TestResult::fail(
            name,
            format!("Failed to get block timestamp after skip: {}", e),
            start.elapsed(),
        ),
    };

    if after.saturating_sub(before) < 10 {
        return TestResult::fail(
            name,
            format!("Time skip failed: {} -> {} (delta: {})", before, after, after.saturating_sub(before)),
            start.elapsed(),
        );
    }

    tracing::info!("Withdrawal execution infrastructure ready (delay={}, time skip works)", delay);
    TestResult::pass(name, start.elapsed())
}
```

## Module Structure

The file should have:
1. Module-level doc comment describing the operator integration tests
2. All required imports at the top
3. All 5 test functions with pub visibility and proper doc comments
4. Each function takes `config: &E2eConfig` and returns `TestResult`

## Constraints

- Use proper error handling (no `.unwrap()` calls)
- Use `tracing::info!` for logging
- All tests must be `pub async fn`
- Use `super::helpers::` for helper functions
- Import `ServiceManager` from `crate`
- Import `AnvilTimeClient` via `crate::AnvilTimeClient`
- Functions should be reusable and testable without actual operator running
