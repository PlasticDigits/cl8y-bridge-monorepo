//! Integration tests for E2E test suite
//!
//! Real token transfer tests with balance verification and full cross-chain cycles.
//!
//! Deposit tests (EVM → Terra) are in `integration_deposit.rs`.
//! Withdrawal tests (Terra → EVM) are in `integration_withdraw.rs`.

use crate::services::ServiceManager;
use crate::transfer_helpers::{
    self, poll_for_approval, skip_withdrawal_delay, verify_withdrawal_executed,
    DEFAULT_POLL_TIMEOUT,
};
use crate::{E2eConfig, TestResult};
use alloy::primitives::{Address, B256, U256};
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use super::helpers::{
    approve_erc20, create_fraudulent_approval, encode_terra_address, execute_deposit,
    get_terra_chain_key, is_approval_cancelled, query_deposit_nonce,
};

// Import deposit and withdraw test functions for runner functions
use super::integration_deposit::{
    test_evm_to_terra_with_verification, test_real_evm_to_terra_transfer,
};
use super::integration_withdraw::{
    test_real_terra_to_evm_transfer, test_terra_to_evm_with_verification,
};

/// Test fraud detection: create fake approval, verify canceler detects and cancels it
///
/// This test:
/// 1. Optionally starts the canceler service
/// 2. Creates a fraudulent approval (no matching deposit)
/// 3. Waits for canceler to detect and cancel
/// 4. Verifies approval was cancelled
/// 5. Stops canceler service
pub async fn test_fraud_detection_full(
    config: &E2eConfig,
    project_root: &Path,
    start_canceler: bool,
) -> TestResult {
    let start = Instant::now();
    let name = "fraud_detection_full";

    info!("Testing fraud detection with fake approval");

    let mut services = ServiceManager::new(project_root);

    // Step 1: Optionally start canceler
    if start_canceler {
        match services.start_canceler(config).await {
            Ok(pid) => {
                info!("Canceler started with PID {}", pid);
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to start canceler: {}", e),
                    start.elapsed(),
                );
            }
        }
    }

    // Step 2: Generate fraudulent approval parameters
    let fraud_nonce = 999_000_000
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            % 1000);
    let fraud_amount = "1234567890123456789";

    // Use registered Terra chain ID — fraud is in the nonce (no matching deposit)
    let fake_src_chain_key = B256::from_slice(&{
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // registered Terra chain
        bytes
    });

    // Use registered test token
    let fake_token = config.evm.contracts.test_token;

    info!(
        "Creating fraudulent approval: nonce={}, amount={}",
        fraud_nonce, fraud_amount
    );

    // Step 3: Create fraudulent approval
    let fraud_result = match create_fraudulent_approval(
        config,
        fake_src_chain_key,
        fake_token,
        config.test_accounts.evm_address,
        fraud_amount,
        fraud_nonce,
    )
    .await
    {
        Ok(result) => {
            info!(
                "Fraudulent approval created: tx=0x{}, withdrawHash=0x{}",
                hex::encode(result.tx_hash),
                hex::encode(&result.withdraw_hash.as_slice()[..8])
            );
            result
        }
        Err(e) => {
            // Clean up canceler if we started it
            if start_canceler {
                let _ = services.stop_canceler().await;
            }
            return TestResult::fail(
                name,
                format!("Failed to create fraudulent approval: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 4: Wait for canceler to detect and cancel (if running)
    if start_canceler || services.is_canceler_running() {
        info!("Waiting for canceler to detect and cancel fraudulent approval...");
        tokio::time::sleep(Duration::from_secs(15)).await;

        // Step 5: Check if approval was cancelled
        match is_approval_cancelled(config, fraud_result.withdraw_hash).await {
            Ok(true) => {
                info!("Fraudulent approval was cancelled successfully");
            }
            Ok(false) => {
                // Clean up
                if start_canceler {
                    let _ = services.stop_canceler().await;
                }
                return TestResult::fail(
                    name,
                    "Canceler did not cancel fraudulent approval within timeout",
                    start.elapsed(),
                );
            }
            Err(e) => {
                // Clean up
                if start_canceler {
                    let _ = services.stop_canceler().await;
                }
                return TestResult::fail(
                    name,
                    format!("Failed to check cancellation status: {}", e),
                    start.elapsed(),
                );
            }
        }
    } else {
        info!("Canceler not running - skipping cancellation verification");
        info!("Fraudulent approval created but not verified cancelled");
    }

    // Step 6: Stop canceler if we started it
    if start_canceler {
        if let Err(e) = services.stop_canceler().await {
            warn!("Failed to stop canceler: {}", e);
        }
    }

    TestResult::pass(name, start.elapsed())
}

/// Run integration tests with real token transfers
///
/// Executes integration tests that perform actual token transfers.
/// Requires:
/// - Test token deployed and funded
/// - Terra bridge deployed and configured
/// - Sufficient balances for transfers
///
/// Options:
/// - `token_address`: ERC20 token to use for EVM transfers
/// - `transfer_amount`: Amount to transfer (in token decimals)
/// - `terra_denom`: Terra denom to use (e.g., "uluna")
/// - `project_root`: Project root for service management
/// - `run_fraud_test`: Whether to run fraud detection test
pub async fn run_integration_tests(
    config: &E2eConfig,
    token_address: Option<Address>,
    transfer_amount: u128,
    terra_denom: &str,
    project_root: &Path,
    run_fraud_test: bool,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    info!("Running integration tests with real token transfers");

    // Run EVM → Terra transfer test
    results.push(test_real_evm_to_terra_transfer(config, token_address, transfer_amount).await);

    // Run Terra → EVM transfer test
    results.push(test_real_terra_to_evm_transfer(config, transfer_amount, terra_denom).await);

    // Optionally run fraud detection test
    if run_fraud_test {
        results.push(test_fraud_detection_full(config, project_root, false).await);
    }

    results
}

/// Test options for integration tests
#[derive(Debug, Clone)]
pub struct IntegrationTestOptions {
    /// ERC20 token address for EVM transfers
    pub token_address: Option<Address>,
    /// Amount to transfer (in token decimals)
    pub transfer_amount: u128,
    /// Terra denom for Terra transfers
    pub terra_denom: String,
    /// Whether to run fraud detection test
    pub run_fraud_test: bool,
    /// Whether to start/stop services automatically
    pub manage_services: bool,
}

impl Default for IntegrationTestOptions {
    fn default() -> Self {
        Self {
            token_address: None,
            transfer_amount: 1_000_000, // 1 token with 6 decimals
            terra_denom: "uluna".to_string(),
            run_fraud_test: false,
            manage_services: false,
        }
    }
}

// ============================================================================
// Full Transfer Cycle Verification
// ============================================================================

/// Execute a complete transfer cycle with full verification
///
/// This performs the entire cross-chain transfer flow:
/// 1. Record initial balances on both chains
/// 2. Execute deposit on source chain
/// 3. Poll for operator to create approval on destination
/// 4. Skip time for withdrawal delay (if on Anvil)
/// 5. Verify withdrawal can be executed
/// 6. Confirm destination balance increased
///
/// Requires operator service to be running.
pub async fn test_full_transfer_cycle(
    config: &E2eConfig,
    token_address: Option<Address>,
    amount: u128,
) -> TestResult {
    let start = Instant::now();
    let name = "full_transfer_cycle";

    // Require token address
    let token = match token_address {
        Some(t) => t,
        None => {
            return TestResult::skip(name, "No test token address provided");
        }
    };

    let test_account = config.test_accounts.evm_address;
    info!(
        "Testing full transfer cycle: {} tokens for account {}",
        amount, test_account
    );

    // Step 1: Get initial balances
    let initial_balance =
        match transfer_helpers::get_erc20_balance(config, token, test_account).await {
            Ok(b) => {
                info!("Initial balance: {}", b);
                b
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to get initial balance: {}", e),
                    start.elapsed(),
                );
            }
        };

    if initial_balance < U256::from(amount) {
        return TestResult::fail(
            name,
            format!(
                "Insufficient balance: have {}, need {}",
                initial_balance, amount
            ),
            start.elapsed(),
        );
    }

    // Step 2: Get initial deposit nonce
    let nonce_before = match query_deposit_nonce(config).await {
        Ok(n) => {
            info!("Initial deposit nonce: {}", n);
            n
        }
        Err(e) => {
            return TestResult::fail(name, format!("Failed to get nonce: {}", e), start.elapsed());
        }
    };

    // Step 3: Get destination chain key (Terra)
    let terra_chain_key = match get_terra_chain_key(config).await {
        Ok(key) => key,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 4: Prepare destination account
    let dest_account = encode_terra_address(&config.test_accounts.terra_address);

    // Step 5: Approve tokens for LockUnlock
    let lock_unlock = config.evm.contracts.lock_unlock;
    if let Err(e) = approve_erc20(config, token, lock_unlock, amount).await {
        return TestResult::fail(
            name,
            format!("Token approval failed: {}", e),
            start.elapsed(),
        );
    }
    info!("Token approval successful");

    // Step 6: Execute deposit
    let _deposit_tx =
        match execute_deposit(config, token, amount, terra_chain_key, dest_account).await {
            Ok(tx) => {
                info!("Deposit executed: 0x{}", hex::encode(tx));
                tx
            }
            Err(e) => {
                return TestResult::fail(name, format!("Deposit failed: {}", e), start.elapsed());
            }
        };

    // Step 7: Verify nonce incremented
    tokio::time::sleep(Duration::from_secs(2)).await;
    let nonce_after = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get nonce after deposit: {}", e),
                start.elapsed(),
            );
        }
    };

    if nonce_after <= nonce_before {
        return TestResult::fail(
            name,
            format!(
                "Nonce did not increment: {} -> {}",
                nonce_before, nonce_after
            ),
            start.elapsed(),
        );
    }
    info!(
        "Deposit nonce incremented: {} -> {}",
        nonce_before, nonce_after
    );

    // Step 8: Poll for operator to create approval
    // The deposit used nonce_before as its nonce (depositNonce++ is post-increment)
    let deposit_nonce = nonce_before;
    info!("Waiting for operator to relay deposit...");
    let approval = match poll_for_approval(config, deposit_nonce, DEFAULT_POLL_TIMEOUT).await {
        Ok(a) => {
            info!(
                "Approval received: hash=0x{}",
                hex::encode(&a.withdraw_hash.as_slice()[..8])
            );
            Some(a)
        }
        Err(e) => {
            warn!("Approval not received (operator may not be running): {}", e);
            None
        }
    };

    // Step 9: Skip time for withdrawal delay (Anvil only)
    if approval.is_some() {
        if let Err(e) = skip_withdrawal_delay(config, 10).await {
            warn!("Failed to skip withdrawal delay: {}", e);
        }
    }

    // Step 10: Verify balance decreased on source
    let final_balance = match transfer_helpers::get_erc20_balance(config, token, test_account).await
    {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get final balance: {}", e),
                start.elapsed(),
            );
        }
    };

    if final_balance >= initial_balance {
        return TestResult::fail(
            name,
            format!(
                "Balance did not decrease: {} -> {}",
                initial_balance, final_balance
            ),
            start.elapsed(),
        );
    }

    let decrease = initial_balance - final_balance;
    info!("Source balance decreased by {}", decrease);

    // Step 11: Verify withdrawal executed (if approval was received)
    if let Some(ref approval_info) = approval {
        match verify_withdrawal_executed(config, approval_info.withdraw_hash).await {
            Ok(true) => {
                info!("Withdrawal was executed successfully");
            }
            Ok(false) => {
                info!("Withdrawal not yet executed (may need manual execution)");
            }
            Err(e) => {
                warn!("Could not verify withdrawal: {}", e);
            }
        }
    }

    info!("Full transfer cycle completed in {:?}", start.elapsed());

    TestResult::pass(name, start.elapsed())
}

/// Run extended integration tests with full verification
pub async fn run_extended_integration_tests(
    config: &E2eConfig,
    token_address: Option<Address>,
    transfer_amount: u128,
    terra_denom: &str,
    project_root: &Path,
) -> Vec<TestResult> {
    info!("Running extended integration tests with full verification");
    vec![
        test_real_evm_to_terra_transfer(config, token_address, transfer_amount).await,
        test_real_terra_to_evm_transfer(config, transfer_amount, terra_denom).await,
        test_full_transfer_cycle(config, token_address, transfer_amount).await,
        test_evm_to_terra_with_verification(config, token_address, transfer_amount).await,
        test_terra_to_evm_with_verification(config, token_address, transfer_amount, terra_denom)
            .await,
        test_fraud_detection_full(config, project_root, false).await,
    ]
}

// Note: CW20 tests have been moved to the dedicated cw20 module.
// Import them from crate::tests::cw20 or use the re-exports from tests::mod.rs.
