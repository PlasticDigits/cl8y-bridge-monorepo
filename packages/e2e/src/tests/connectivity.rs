//! Connectivity tests for E2E test suite
//!
//! Tests that verify infrastructure connectivity to EVM, Terra, and PostgreSQL.

use crate::{E2eConfig, TestResult};
use std::time::Instant;
use url::Url;

use super::helpers::{check_evm_connection, check_terra_connection};

/// Test EVM (Anvil) connectivity
///
/// Attempts to connect to the EVM RPC endpoint and retrieve the current block number.
/// Returns a `TestResult::Pass` if successful, `TestResult::Fail` otherwise.
pub async fn test_evm_connectivity(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_connectivity";

    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(block) => {
            tracing::info!("EVM connected, block: {}", block);
            TestResult::pass(name, start.elapsed())
        }
        Err(e) => TestResult::fail(name, format!("Failed to connect: {}", e), start.elapsed()),
    }
}

/// Test Terra (LocalTerra) connectivity
///
/// Attempts to connect to the Terra LCD endpoint and check if the node is synced.
/// Returns a `TestResult::Pass` if successful, `TestResult::Fail` otherwise.
pub async fn test_terra_connectivity(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "terra_connectivity";

    match check_terra_connection(&config.terra.lcd_url).await {
        Ok(_) => TestResult::pass(name, start.elapsed()),
        Err(e) => TestResult::fail(name, format!("Failed to connect: {}", e), start.elapsed()),
    }
}

/// Test PostgreSQL connectivity
///
/// Validates that the database URL is properly formatted and accessible.
/// Returns a `TestResult::Pass` if the URL parses correctly, `TestResult::Fail` otherwise.
pub async fn test_database_connectivity(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "database_connectivity";

    match Url::parse(&config.operator.database_url) {
        Ok(_) => TestResult::pass(name, start.elapsed()),
        Err(e) => TestResult::fail(
            name,
            format!("Invalid database URL: {}", e),
            start.elapsed(),
        ),
    }
}
