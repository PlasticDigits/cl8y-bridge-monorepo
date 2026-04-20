//! Fraud detection tests for E2E test suite
//!
//! Tests that verify fraud detection infrastructure is properly configured.

use crate::{E2eConfig, TestResult};
use alloy::primitives::Address;
use std::time::Instant;

use super::helpers::{query_bridge_is_canceler, query_cancel_window, query_contract_code};

/// Test fraud detection mechanism
///
/// Verifies that the fraud detection infrastructure is properly configured:
/// 1. Bridge `isCanceler` allows the test EOA to cancel (owner or `addCanceler`)
/// 2. Bridge contract can be queried for approval status
/// 3. Withdraw delay is sufficient for watchtower detection
///
/// SECURITY HARDENED: Withdraw delay and `isCanceler` query failures now cause test failure.
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

    // Step 2: Check cancel window is configured (must be > 0 for watchtower pattern)
    // SECURITY HARDENED: Convert WARN to FAIL
    match query_cancel_window(config).await {
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

    // Step 3: Verify a Bridge cancel path exists (owner or registered canceler — not AccessManager)
    let test_address = config.test_accounts.evm_address;
    match query_bridge_is_canceler(config, test_address).await {
        Ok(true) => {
            tracing::info!(
                "Test account {} passes Bridge.isCanceler (may withdrawCancel)",
                test_address
            );
        }
        Ok(false) => {
            return TestResult::fail(
                name,
                "Test account cannot cancel on Bridge — use Bridge.addCanceler or deploy as owner",
                start.elapsed(),
            );
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!(
                    "Bridge isCanceler query failed (needed to verify fraud response path): {}",
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
