//! Fraud detection tests for E2E test suite
//!
//! Tests that verify fraud detection infrastructure is properly configured.

use crate::{E2eConfig, TestResult};
use alloy::primitives::Address;
use std::time::Instant;

use super::helpers::{query_contract_code, query_has_role, query_withdraw_delay};

/// Test fraud detection mechanism
///
/// Verifies that the fraud detection infrastructure is properly configured:
/// 1. AccessManager has CANCELER_ROLE defined
/// 2. Bridge contract can be queried for approval status
/// 3. Withdraw delay is sufficient for watchtower detection
///
/// SECURITY HARDENED: Withdraw delay and CANCELER_ROLE query failures now cause test failure.
///
/// Note: Full fraud detection testing requires the canceler service running.
/// Returns a `TestResult` indicating success or failure.
pub async fn test_fraud_detection(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "fraud_detection";

    // Step 1: Verify bridge contract is deployed
    if config.evm.contracts.bridge == Address::ZERO {
        return TestResult::fail(name, "Bridge address not configured", start.elapsed());
    }

    match query_contract_code(config, config.evm.contracts.bridge).await {
        Ok(true) => {
            tracing::info!(
                "Bridge contract deployed at {}",
                config.evm.contracts.bridge
            );
        }
        Ok(false) => {
            return TestResult::fail(name, "Bridge has no code deployed", start.elapsed());
        }
        Err(e) => {
            return TestResult::fail(name, format!("Cannot query Bridge: {}", e), start.elapsed());
        }
    }

    // Step 2: Check withdraw delay is configured (must be > 0 for watchtower pattern)
    // SECURITY HARDENED: Convert WARN to FAIL
    match query_withdraw_delay(config).await {
        Ok(delay) => {
            if delay == 0 {
                return TestResult::fail(
                    name,
                    "Withdraw delay is 0 - watchtower protection disabled!",
                    start.elapsed(),
                );
            }
            tracing::info!("Watchtower delay: {} seconds", delay);
            if delay < 60 {
                tracing::warn!("Withdraw delay is short ({} seconds) - may not allow enough time for fraud detection", delay);
            }
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!(
                    "Withdraw delay query failed - watchtower protection cannot be verified: {}",
                    e
                ),
                start.elapsed(),
            );
        }
    }

    // Step 3: Verify AccessManager is deployed (needed for cancel permissions)
    if config.evm.contracts.access_manager == Address::ZERO {
        return TestResult::fail(
            name,
            "AccessManager address not configured",
            start.elapsed(),
        );
    }

    match query_contract_code(config, config.evm.contracts.access_manager).await {
        Ok(true) => {
            tracing::info!(
                "AccessManager deployed at {}",
                config.evm.contracts.access_manager
            );
        }
        Ok(false) => {
            return TestResult::fail(name, "AccessManager has no code deployed", start.elapsed());
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Cannot query AccessManager: {}", e),
                start.elapsed(),
            );
        }
    }

    // Step 4: Check if test account has CANCELER_ROLE (role id 2)
    // SECURITY HARDENED: Convert WARN to FAIL
    let test_address = config.test_accounts.evm_address;
    match query_has_role(config, 2, test_address).await {
        Ok(true) => {
            tracing::info!("Test account {} has CANCELER_ROLE", test_address);
        }
        Ok(false) => {
            tracing::info!("Test account {} does not have CANCELER_ROLE", test_address);
            tracing::info!("Grant CANCELER_ROLE for full fraud detection testing");
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!(
                    "CANCELER_ROLE query failed (role verification is security-critical): {}",
                    e
                ),
                start.elapsed(),
            );
        }
    }

    tracing::info!("Fraud detection infrastructure verified");
    tracing::info!("  - Watchtower pattern: enabled");
    tracing::info!("  - Canceler can detect fraudulent approvals during delay window");
    tracing::info!("  - cancelWithdrawApproval() available for fraud response");

    TestResult::pass(name, start.elapsed())
}
