//! E2E test cases for the bridge system
//!
//! This module provides test functions organized by category:
//!
//! - **connectivity**: Verify infrastructure connectivity to EVM, Terra, and PostgreSQL
//! - **configuration**: Verify contracts and accounts are properly configured
//! - **transfer**: Verify EVM <-> Terra transfer infrastructure
//! - **fraud**: Verify fraud detection infrastructure
//! - **integration**: Real token transfer tests with balance verification
//! - **canceler**: Canceler security tests (fee collection, fraud detection, health)
//! - **edge_cases**: Edge cases and observability tests (restart recovery, validation, metrics, double spend)
//! - **watchtower**: Watchtower pattern tests (EVM time skip, delay mechanism, delay enforcement)

mod canceler;
mod configuration;
mod connectivity;
mod database;
mod edge_cases;
pub mod evm_to_evm;
mod fraud;
pub mod helpers;
mod integration;
mod operator;
mod transfer;
mod watchtower;

// Re-export all public tests
pub use canceler::*;
pub use configuration::*;
pub use connectivity::*;
pub use database::*;
pub use edge_cases::*;
pub use fraud::*;
pub use integration::*;
pub use operator::*;
pub use transfer::*;
pub use watchtower::*;

use crate::{E2eConfig, TestResult};

/// Run quick connectivity tests only
///
/// Executes a minimal set of tests to verify basic connectivity.
/// Returns a vector of `TestResult` objects.
pub async fn run_quick_tests(config: &E2eConfig) -> Vec<TestResult> {
    vec![
        test_evm_connectivity(config).await,
        test_terra_connectivity(config).await,
        test_database_connectivity(config).await,
    ]
}

/// Run all E2E tests
///
/// Executes the full suite of E2E tests.
/// If `skip_terra` is true, Terra-specific tests are skipped.
/// Returns a vector of `TestResult` objects.
pub async fn run_all_tests(config: &E2eConfig, skip_terra: bool) -> Vec<TestResult> {
    let mut results = Vec::new();

    // Connectivity tests
    results.push(test_evm_connectivity(config).await);
    if !skip_terra {
        results.push(test_terra_connectivity(config).await);
    }
    results.push(test_database_connectivity(config).await);

    // Configuration tests
    results.push(test_accounts_configured(config).await);
    results.push(test_terra_bridge_configured(config).await);
    results.push(test_evm_contracts_deployed(config).await);

    // Infrastructure verification tests
    results.push(test_evm_to_terra_transfer(config).await);
    results.push(test_terra_to_evm_transfer(config).await);
    results.push(test_fraud_detection(config).await);
    results.push(test_deposit_nonce(config).await);
    results.push(test_token_registry(config).await);
    results.push(test_chain_registry(config).await);
    results.push(test_access_manager(config).await);

    // ========================================
    // Watchtower Pattern Tests
    // ========================================

    // Watchtower Pattern (4)
    results.push(watchtower::test_evm_time_skip(config).await);
    results.push(watchtower::test_watchtower_delay_mechanism(config).await);
    results.push(watchtower::test_withdraw_delay_enforcement(config).await);
    results.push(watchtower::test_approval_cancellation_blocks_withdraw(config).await);

    // Database & Hash Parity (5) - IMPLEMENTED in database.rs
    results.push(database::test_nonce_replay_prevention(config).await);
    results.push(database::test_database_tables(config).await);
    results.push(database::test_database_migrations(config).await);
    results.push(database::test_database_connection_pool(config).await);
    results.push(database::test_hash_parity(config).await);

    // Hash Parity & Operator Integration (5) - IMPLEMENTED in operator.rs
    results.push(operator::test_withdraw_hash_computation(config).await);
    results.push(operator::test_operator_startup(config).await);
    results.push(operator::test_operator_deposit_detection(config).await);
    results.push(operator::test_operator_approval_creation(config).await);
    results.push(operator::test_operator_withdrawal_execution(config).await);

    // Canceler Security (5) - IMPLEMENTED in canceler.rs
    results.push(canceler::test_operator_fee_collection(config).await);
    results.push(canceler::test_canceler_autonomous_detection(config).await);
    results.push(canceler::test_canceler_health_endpoint(config).await);
    results.push(canceler::test_concurrent_approvals(config).await);
    results.push(canceler::test_rpc_failure_resilience(config).await);

    // Edge Cases & Observability (6) - IMPLEMENTED in edge_cases.rs
    results.push(edge_cases::test_canceler_restart_recovery(config).await);
    results.push(edge_cases::test_double_spend_prevention(config).await);
    results.push(edge_cases::test_invalid_chain_key_rejected(config).await);
    results.push(edge_cases::test_invalid_recipient_rejected(config).await);
    results.push(edge_cases::test_metrics_endpoint(config).await);
    results.push(edge_cases::test_structured_logging(config).await);

    // EVM-to-EVM Transfer Tests (3) - IMPLEMENTED in evm_to_evm.rs
    results.push(evm_to_evm::test_evm_chain_key_computation(config).await);
    results.push(evm_to_evm::test_mock_chain_registration(config).await);
    // Note: test_evm_to_evm_deposit and test_evm_to_evm_full_cycle require token address

    // CW20 Cross-Chain Transfer Tests (4) - IMPLEMENTED in integration.rs
    // These tests use the CW20 address from config (set during setup)
    let cw20_address = config.terra.cw20_address.as_deref();
    results.push(integration::test_cw20_deployment(config, cw20_address).await);
    results.push(integration::test_cw20_balance_query(config, cw20_address).await);
    results.push(integration::test_cw20_mint_burn_pattern(config, cw20_address).await);
    results.push(integration::test_cw20_lock_unlock_pattern(config, cw20_address).await);

    results
}
