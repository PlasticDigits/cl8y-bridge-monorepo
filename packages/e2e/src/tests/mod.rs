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
//! - **canceler_execution**: Live canceler fraud detection and cancel transaction tests
//! - **operator**: Operator infrastructure tests (hash computation, startup, detection)
//! - **operator_execution**: Live operator deposit/withdrawal execution tests
//! - **edge_cases**: Edge cases and observability tests (restart recovery, validation, metrics, double spend)
//! - **watchtower**: Watchtower pattern tests (EVM time skip, delay mechanism, delay enforcement)
//! - **cw20**: CW20 cross-chain transfer tests (deployment, balance, mint/burn, lock/unlock)

mod canceler;
mod canceler_execution;
mod canceler_helpers;
mod configuration;
mod connectivity;
mod cw20;
mod database;
mod edge_cases;
pub mod evm_to_evm;
mod fraud;
pub mod helpers;
mod integration;
mod operator;
mod operator_execution;
pub mod operator_helpers;
mod transfer;
mod watchtower;

// Re-export all public tests
pub use canceler::*;
pub use canceler_execution::*;
pub use configuration::*;
pub use connectivity::*;
pub use cw20::*;
pub use database::*;
pub use edge_cases::*;
pub use fraud::*;
pub use integration::*;
pub use operator::*;
pub use operator_execution::*;
pub use transfer::*;
pub use watchtower::*;

use crate::{E2eConfig, TestResult};
use alloy::primitives::Address;

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

    // CW20 Cross-Chain Transfer Tests (4) - IMPLEMENTED in cw20.rs
    // These tests use the CW20 address from config (set during setup)
    let cw20_address = config.terra.cw20_address.as_deref();
    results.push(cw20::test_cw20_deployment(config, cw20_address).await);
    results.push(cw20::test_cw20_balance_query(config, cw20_address).await);
    results.push(cw20::test_cw20_mint_burn_pattern(config, cw20_address).await);
    results.push(cw20::test_cw20_lock_unlock_pattern(config, cw20_address).await);

    // ========================================
    // Live Operator/Canceler Execution Tests
    // ========================================
    // These tests verify on-chain results with actual transaction execution.
    // They will skip gracefully if the required services are not running.

    // Live Canceler Execution Tests (6) - IMPLEMENTED in canceler_execution.rs
    // Canceler is started by E2E setup, so these should run
    results.push(canceler_execution::test_canceler_live_fraud_detection(config).await);
    results.push(canceler_execution::test_cancelled_approval_blocks_withdrawal(config).await);
    results.push(canceler_execution::test_canceler_concurrent_fraud_handling(config).await);
    results.push(canceler_execution::test_canceler_restart_fraud_detection(config).await);
    // EVM→EVM and Terra→EVM fraud detection tests
    results.push(canceler_execution::test_canceler_evm_source_fraud_detection(config).await);
    results.push(canceler_execution::test_canceler_terra_source_fraud_detection(config).await);

    // Live Operator Execution Tests (3) - IMPLEMENTED in operator_execution.rs
    // Operator is NOT started by default E2E setup, so these will skip unless
    // the operator is manually started. They verify deposit detection and
    // withdrawal execution with balance verification.
    // Note: These require a test token address which may not be available
    let test_token = config.evm.contracts.test_token;
    let token_address = if test_token != Address::ZERO {
        Some(test_token)
    } else {
        None
    };
    results.push(
        operator_execution::test_operator_live_deposit_detection(config, token_address).await,
    );
    results.push(
        operator_execution::test_operator_live_withdrawal_execution(config, token_address).await,
    );
    results.push(
        operator_execution::test_operator_sequential_deposit_processing(config, token_address, 3)
            .await,
    );

    results
}

/// Run live operator/canceler execution tests
///
/// These tests verify on-chain results with actual transaction execution:
/// - Operator deposit detection with Terra approval creation
/// - Operator withdrawal execution after delay with balance verification
/// - Canceler fraud detection and cancel transaction submission
/// - Cancelled approvals blocking withdrawal execution
///
/// Requires running operator and canceler services, and a test token address.
pub async fn run_live_execution_tests(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    tracing::info!("Running live operator/canceler execution tests");

    // Live Operator Execution Tests (3)
    results.push(
        operator_execution::test_operator_live_deposit_detection(config, token_address).await,
    );
    results.push(
        operator_execution::test_operator_live_withdrawal_execution(config, token_address).await,
    );
    results.push(
        operator_execution::test_operator_sequential_deposit_processing(config, token_address, 3)
            .await,
    );

    // Live Canceler Execution Tests (6)
    results.push(canceler_execution::test_canceler_live_fraud_detection(config).await);
    results.push(canceler_execution::test_cancelled_approval_blocks_withdrawal(config).await);
    results.push(canceler_execution::test_canceler_concurrent_fraud_handling(config).await);
    results.push(canceler_execution::test_canceler_restart_fraud_detection(config).await);
    // EVM→EVM and Terra→EVM fraud detection tests
    results.push(canceler_execution::test_canceler_evm_source_fraud_detection(config).await);
    results.push(canceler_execution::test_canceler_terra_source_fraud_detection(config).await);

    results
}

/// Run all tests including live execution tests
///
/// Executes the complete test suite including live on-chain execution tests.
/// Requires all services running and a funded test token.
pub async fn run_all_tests_with_live_execution(
    config: &E2eConfig,
    skip_terra: bool,
    token_address: Option<Address>,
) -> Vec<TestResult> {
    let mut results = run_all_tests(config, skip_terra).await;
    results.extend(run_live_execution_tests(config, token_address).await);
    results
}
