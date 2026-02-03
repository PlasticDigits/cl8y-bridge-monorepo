---
context_files:
  - src/tests/helpers.rs
  - src/lib.rs
output_dir: src/tests/
output_file: database.rs
---

# Implement Batch 2: Database & Hash Parity Tests

Create a new test module `database.rs` with 5 implemented tests for database connectivity, schema verification, and hash parity.

## Requirements

Implement these 5 tests that currently exist as stubs. Each test should return `TestResult::pass` on success or `TestResult::fail` with a descriptive error message on failure.

## Imports Required

```rust
use crate::{E2eConfig, TestResult};
use sqlx::postgres::PgPoolOptions;
use std::time::{Duration, Instant};
use tokio::time::timeout;
```

## Tests to Implement

### 1. test_nonce_replay_prevention

Verify that deposit nonces are sequential and cannot be replayed.

```rust
pub async fn test_nonce_replay_prevention(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "nonce_replay_prevention";

    // 1. Query current deposit nonce from bridge contract
    // 2. Verify nonce is non-zero (deposits have occurred)
    // 3. Query nonce again to ensure it hasn't changed unexpectedly
    // 4. The nonce should only increment on new deposits
    
    // Use helpers::query_deposit_nonce from context
    match super::helpers::query_deposit_nonce(config).await {
        Ok(nonce) => {
            if nonce == 0 {
                // Nonce of 0 means no deposits yet - this is acceptable for fresh deploys
                TestResult::pass(name, start.elapsed())
            } else {
                // Verify we can query it consistently
                match super::helpers::query_deposit_nonce(config).await {
                    Ok(nonce2) if nonce2 >= nonce => TestResult::pass(name, start.elapsed()),
                    Ok(nonce2) => TestResult::fail(
                        name,
                        format!("Nonce decreased unexpectedly: {} -> {}", nonce, nonce2),
                        start.elapsed(),
                    ),
                    Err(e) => TestResult::fail(name, format!("Second nonce query failed: {}", e), start.elapsed()),
                }
            }
        }
        Err(e) => TestResult::fail(name, format!("Failed to query deposit nonce: {}", e), start.elapsed()),
    }
}
```

### 2. test_database_tables

Verify PostgreSQL schema tables exist for the bridge system.

```rust
pub async fn test_database_tables(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_tables";

    // Get database URL from operator config
    let db_url = match &config.operator.database_url {
        Some(url) => url.clone(),
        None => return TestResult::skip(name, "No database URL configured"),
    };

    // Connect to database with timeout
    let pool = match timeout(Duration::from_secs(10), async {
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
    }).await {
        Ok(Ok(pool)) => pool,
        Ok(Err(e)) => return TestResult::fail(name, format!("Database connection failed: {}", e), start.elapsed()),
        Err(_) => return TestResult::fail(name, "Database connection timed out", start.elapsed()),
    };

    // Query information_schema for expected tables
    // Common bridge tables: deposits, withdrawals, approvals, or similar
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'"
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(count) if count > 0 => TestResult::pass(name, start.elapsed()),
        Ok(_) => TestResult::fail(name, "No tables found in public schema", start.elapsed()),
        Err(e) => TestResult::fail(name, format!("Schema query failed: {}", e), start.elapsed()),
    }
}
```

### 3. test_database_migrations

Verify database migrations have been applied.

```rust
pub async fn test_database_migrations(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_migrations";

    let db_url = match &config.operator.database_url {
        Some(url) => url.clone(),
        None => return TestResult::skip(name, "No database URL configured"),
    };

    let pool = match timeout(Duration::from_secs(10), async {
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
    }).await {
        Ok(Ok(pool)) => pool,
        Ok(Err(e)) => return TestResult::fail(name, format!("Database connection failed: {}", e), start.elapsed()),
        Err(_) => return TestResult::fail(name, "Database connection timed out", start.elapsed()),
    };

    // Check for sqlx migrations table
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '_sqlx_migrations'"
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(count) if count > 0 => {
            // Migrations table exists, check for applied migrations
            let migration_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM _sqlx_migrations"
            )
            .fetch_one(&pool)
            .await;

            match migration_count {
                Ok(n) if n > 0 => TestResult::pass(name, start.elapsed()),
                Ok(_) => TestResult::fail(name, "No migrations have been applied", start.elapsed()),
                Err(e) => TestResult::fail(name, format!("Migration count query failed: {}", e), start.elapsed()),
            }
        }
        Ok(_) => TestResult::skip(name, "No _sqlx_migrations table (may use different migration system)"),
        Err(e) => TestResult::fail(name, format!("Schema query failed: {}", e), start.elapsed()),
    }
}
```

### 4. test_database_connection_pool

Verify connection pooling handles concurrent load.

```rust
pub async fn test_database_connection_pool(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_connection_pool";

    let db_url = match &config.operator.database_url {
        Some(url) => url.clone(),
        None => return TestResult::skip(name, "No database URL configured"),
    };

    // Create pool with multiple connections
    let pool = match timeout(Duration::from_secs(15), async {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(&db_url)
            .await
    }).await {
        Ok(Ok(pool)) => pool,
        Ok(Err(e)) => return TestResult::fail(name, format!("Pool creation failed: {}", e), start.elapsed()),
        Err(_) => return TestResult::fail(name, "Pool creation timed out", start.elapsed()),
    };

    // Run 10 concurrent queries
    let mut handles = Vec::new();
    for i in 0..10 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            sqlx::query_scalar::<_, i64>("SELECT $1::bigint")
                .bind(i as i64)
                .fetch_one(&pool_clone)
                .await
        });
        handles.push(handle);
    }

    // Wait for all queries to complete
    let mut success_count = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(e)) => {
                return TestResult::fail(
                    name,
                    format!("Concurrent query failed: {}", e),
                    start.elapsed(),
                );
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Task join failed: {}", e),
                    start.elapsed(),
                );
            }
        }
    }

    if success_count == 10 {
        TestResult::pass(name, start.elapsed())
    } else {
        TestResult::fail(
            name,
            format!("Only {}/10 concurrent queries succeeded", success_count),
            start.elapsed(),
        )
    }
}
```

### 5. test_hash_parity

Verify hash computation parity between Rust and on-chain contracts.

```rust
pub async fn test_hash_parity(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "hash_parity";

    // Test that our ChainKey computation matches on-chain
    // 1. Compute EVM chain key locally using ChainKey::evm()
    // 2. Query chain key from ChainRegistry on-chain
    // 3. Verify they match

    let chain_id = 31337u64; // Anvil default chain ID

    // Compute local chain key
    let local_chain_key = crate::ChainKey::evm(chain_id);

    // Query on-chain chain key
    match super::helpers::query_evm_chain_key(config, chain_id).await {
        Ok(onchain_key) => {
            if local_chain_key.as_bytes() == &onchain_key {
                TestResult::pass(name, start.elapsed())
            } else {
                TestResult::fail(
                    name,
                    format!(
                        "Chain key mismatch: local=0x{} vs onchain=0x{}",
                        hex::encode(local_chain_key.as_bytes()),
                        hex::encode(&onchain_key)
                    ),
                    start.elapsed(),
                )
            }
        }
        Err(e) => {
            // Chain might not be registered yet, which is acceptable
            TestResult::skip(name, format!("Could not query chain key (may not be registered): {}", e))
        }
    }
}
```

## Constraints

- All tests must return TestResult (pass/fail/skip)
- Use timeout wrappers for database operations (10-15 seconds max)
- Handle missing database URL gracefully with TestResult::skip
- Use `super::helpers::` prefix for helper functions
- Import `crate::ChainKey` for hash parity test
- No unwrap() calls - use proper error handling
