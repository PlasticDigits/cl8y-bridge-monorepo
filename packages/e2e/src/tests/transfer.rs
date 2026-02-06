//! Transfer tests for E2E test suite
//!
//! Tests that verify EVM <-> Terra transfer infrastructure.

use crate::{E2eConfig, TestResult};
use alloy::primitives::Address;
use std::time::Instant;

use super::helpers::{
    check_terra_connection, query_cancel_window, query_contract_code, query_deposit_nonce,
    query_terra_bridge_delay,
};

/// Test EVM to Terra transfer
///
/// Verifies EVM -> Terra transfer flow by checking:
/// 1. Bridge contracts are configured
/// 2. Deposit nonce can be read
/// 3. Terra bridge address is configured (if Terra enabled)
///
/// SECURITY HARDENED: Withdraw delay and Terra bridge failures now cause test failure.
///
/// Note: Full transfer execution requires tokens and operator running.
/// Returns a `TestResult` indicating success or failure.
pub async fn test_evm_to_terra_transfer(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_to_terra_transfer";

    // Step 1: Verify EVM contracts are deployed
    if config.evm.contracts.bridge == Address::ZERO {
        return TestResult::fail(name, "Bridge address not configured", start.elapsed());
    }

    // Step 2: Check deposit nonce is queryable
    match query_deposit_nonce(config).await {
        Ok(nonce) => {
            tracing::info!("EVM deposit nonce: {}", nonce);
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Cannot query deposit nonce: {}", e),
                start.elapsed(),
            );
        }
    }

    // Step 3: Check cancel window configuration
    // SECURITY HARDENED: Convert WARN to FAIL
    match query_cancel_window(config).await {
        Ok(window) => {
            tracing::info!("EVM cancel window: {} seconds", window);
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Cancel window query failed (security-critical): {}", e),
                start.elapsed(),
            );
        }
    }

    // Step 4: Check Terra bridge configuration
    // SECURITY HARDENED: Convert WARN to FAIL
    match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => {
            tracing::info!("Terra bridge address configured: {}", addr);
        }
        _ => {
            return TestResult::fail(
                name,
                "Terra bridge address not configured (required for cross-chain security)",
                start.elapsed(),
            );
        }
    }

    // Step 5: Verify LockUnlock adapter is deployed (used for EVM deposits)
    if config.evm.contracts.lock_unlock != Address::ZERO {
        match query_contract_code(config, config.evm.contracts.lock_unlock).await {
            Ok(true) => {
                tracing::info!(
                    "LockUnlock adapter deployed at {}",
                    config.evm.contracts.lock_unlock
                );
            }
            Ok(false) => {
                return TestResult::fail(name, "LockUnlock has no code", start.elapsed());
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Cannot query LockUnlock: {}", e),
                    start.elapsed(),
                );
            }
        }
    }

    tracing::info!("EVM -> Terra transfer infrastructure verified");
    tracing::info!("  Bridge: {}", config.evm.contracts.bridge);
    tracing::info!("  LockUnlock: {}", config.evm.contracts.lock_unlock);

    TestResult::pass(name, start.elapsed())
}

/// Test Terra to EVM transfer
///
/// Verifies Terra -> EVM transfer flow by checking:
/// 1. Terra connectivity
/// 2. Terra bridge address configuration
/// 3. EVM bridge can receive approvals
///
/// SECURITY HARDENED: Terra bridge delay and MintBurn failures now cause test failure.
///
/// Note: Full transfer execution requires Terra tokens and operator running.
/// Returns a `TestResult` indicating success or failure.
pub async fn test_terra_to_evm_transfer(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "terra_to_evm_transfer";

    // Step 1: Check if Terra bridge is configured
    let terra_bridge = match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => addr.clone(),
        _ => {
            tracing::info!("Terra bridge not configured - skipping Terra->EVM test");
            return TestResult::skip(name, "Terra bridge address not configured");
        }
    };

    // Step 2: Check Terra connectivity
    match check_terra_connection(&config.terra.lcd_url).await {
        Ok(_) => {
            tracing::info!("Terra LCD is accessible");
        }
        Err(e) => {
            tracing::warn!("Terra LCD not accessible: {}", e);
            return TestResult::skip(name, format!("Terra not accessible: {}", e));
        }
    }

    // Step 3: Verify EVM bridge can receive approvals
    if config.evm.contracts.bridge == Address::ZERO {
        return TestResult::fail(name, "EVM Bridge address not configured", start.elapsed());
    }

    // Step 4: Query Terra bridge configuration (if accessible)
    // SECURITY HARDENED: Convert WARN to FAIL
    match query_terra_bridge_delay(config, &terra_bridge).await {
        Ok(delay) => {
            tracing::info!("Terra bridge withdraw delay: {} seconds", delay);
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Terra bridge delay query failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Step 5: Check MintBurn adapter is deployed (used for Terra->EVM)
    // SECURITY HARDENED: Convert WARN to FAIL
    if config.evm.contracts.mint_burn != Address::ZERO {
        match query_contract_code(config, config.evm.contracts.mint_burn).await {
            Ok(true) => {
                tracing::info!(
                    "MintBurn adapter deployed at {}",
                    config.evm.contracts.mint_burn
                );
            }
            Ok(false) => {
                return TestResult::fail(
                    name,
                    "MintBurn adapter has no code deployed",
                    start.elapsed(),
                );
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("MintBurn query failed: {}", e),
                    start.elapsed(),
                );
            }
        }
    }

    tracing::info!("Terra -> EVM transfer infrastructure verified");
    tracing::info!("  Terra Bridge: {}", terra_bridge);
    tracing::info!("  EVM Bridge: {}", config.evm.contracts.bridge);
    tracing::info!("  MintBurn: {}", config.evm.contracts.mint_burn);

    TestResult::pass(name, start.elapsed())
}
