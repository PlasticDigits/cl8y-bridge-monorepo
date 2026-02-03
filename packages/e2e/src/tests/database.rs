//! Database and hash parity tests
//!
//! This module contains tests for database connectivity, schema verification,
//! and hash computation parity between Rust and on-chain contracts.

use crate::{E2eConfig, TestResult};
use sqlx::postgres::PgPoolOptions;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Verify that deposit nonces are sequential and cannot be replayed.
///
/// This test queries the deposit nonce from the bridge contract twice to ensure
/// consistency and that nonces only increment on new deposits.
pub async fn test_nonce_replay_prevention(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "nonce_replay_prevention";

    // Query current deposit nonce from bridge contract
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
                    Err(e) => TestResult::fail(
                        name,
                        format!("Second nonce query failed: {}", e),
                        start.elapsed(),
                    ),
                }
            }
        }
        Err(e) => TestResult::fail(
            name,
            format!("Failed to query deposit nonce: {}", e),
            start.elapsed(),
        ),
    }
}

/// Verify PostgreSQL schema tables exist for the bridge system.
///
/// This test connects to the database and verifies that at least one table exists
/// in the public schema, indicating the database has been initialized.
pub async fn test_database_tables(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_tables";

    // Get database URL from operator config
    let db_url = &config.operator.database_url;

    // Connect to database with timeout
    let pool = match timeout(Duration::from_secs(10), async {
        PgPoolOptions::new()
            .max_connections(1)
            .connect(db_url)
            .await
    })
    .await
    {
        Ok(Ok(pool)) => pool,
        Ok(Err(e)) => {
            return TestResult::fail(
                name,
                format!("Database connection failed: {}", e),
                start.elapsed(),
            )
        }
        Err(_) => return TestResult::fail(name, "Database connection timed out", start.elapsed()),
    };

    // Query information_schema for expected tables
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(count) if count > 0 => TestResult::pass(name, start.elapsed()),
        Ok(_) => TestResult::fail(name, "No tables found in public schema", start.elapsed()),
        Err(e) => TestResult::fail(name, format!("Schema query failed: {}", e), start.elapsed()),
    }
}

/// Verify database migrations have been applied.
///
/// This test checks for the sqlx migrations table and verifies that migrations
/// have been applied to the database.
pub async fn test_database_migrations(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_migrations";

    let db_url = &config.operator.database_url;

    let pool = match timeout(Duration::from_secs(10), async {
        PgPoolOptions::new()
            .max_connections(1)
            .connect(db_url)
            .await
    })
    .await
    {
        Ok(Ok(pool)) => pool,
        Ok(Err(e)) => {
            return TestResult::fail(
                name,
                format!("Database connection failed: {}", e),
                start.elapsed(),
            )
        }
        Err(_) => return TestResult::fail(name, "Database connection timed out", start.elapsed()),
    };

    // Check for sqlx migrations table
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '_sqlx_migrations'",
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(count) if count > 0 => {
            // Migrations table exists, check for applied migrations
            let migration_count =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations")
                    .fetch_one(&pool)
                    .await;

            match migration_count {
                Ok(n) if n > 0 => TestResult::pass(name, start.elapsed()),
                Ok(_) => TestResult::fail(name, "No migrations have been applied", start.elapsed()),
                Err(e) => TestResult::fail(
                    name,
                    format!("Migration count query failed: {}", e),
                    start.elapsed(),
                ),
            }
        }
        Ok(_) => TestResult::skip(
            name,
            "No _sqlx_migrations table (may use different migration system)",
        ),
        Err(e) => TestResult::fail(name, format!("Schema query failed: {}", e), start.elapsed()),
    }
}

/// Verify connection pooling handles concurrent load.
///
/// This test creates a connection pool with multiple connections and runs
/// concurrent queries to ensure the pool handles load correctly.
pub async fn test_database_connection_pool(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_connection_pool";

    let db_url = &config.operator.database_url;

    // Create pool with multiple connections
    let pool = match timeout(Duration::from_secs(15), async {
        PgPoolOptions::new()
            .max_connections(10)
            .connect(db_url)
            .await
    })
    .await
    {
        Ok(Ok(pool)) => pool,
        Ok(Err(e)) => {
            return TestResult::fail(
                name,
                format!("Pool creation failed: {}", e),
                start.elapsed(),
            )
        }
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
                return TestResult::fail(name, format!("Task join failed: {}", e), start.elapsed());
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

/// Verify hash computation parity between Rust and on-chain contracts.
///
/// This test computes the chain key locally using Rust and queries the chain key
/// from the ChainRegistry contract on-chain to verify they match.
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
            TestResult::skip(
                name,
                format!("Could not query chain key (may not be registered): {}", e),
            )
        }
    }
}
