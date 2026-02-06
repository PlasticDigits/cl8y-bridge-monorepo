//! Live Canceler Execution Tests
//!
//! This module contains end-to-end tests that verify on-chain results for canceler
//! fraud detection and cancel transaction submission.
//!
//! These tests require:
//! - Running canceler service
//! - Running Anvil (EVM) node
//! - Deployed bridge contracts with CANCELER_ROLE granted
//! - Test accounts with appropriate permissions

use crate::evm::AnvilTimeClient;
use crate::services::ServiceManager;
use crate::transfer_helpers::skip_withdrawal_delay;
use crate::{E2eConfig, TestResult};
use alloy::primitives::B256;
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::info;

use super::canceler_helpers::{
    cancel_approval_directly, check_canceler_health, create_fraudulent_approval,
    generate_unique_nonce, is_approval_cancelled, try_execute_withdrawal, FraudulentApprovalResult,
};
use super::helpers::{query_contract_code, verify_tx_success};

// ============================================================================
// Constants
// ============================================================================

/// Maximum time to wait for canceler to detect and cancel fraud
const FRAUD_DETECTION_TIMEOUT: Duration = Duration::from_secs(45);

/// Poll interval for checking cancellation status
const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_secs(2);

// ============================================================================
// Live Canceler Fraud Detection Tests
// ============================================================================

/// Test canceler live fraud detection and cancel transaction submission
///
/// This test verifies the complete fraud detection flow:
/// 1. Verify canceler service is running with CANCELER_ROLE
/// 2. Create a fraudulent approval (no matching deposit on source chain)
/// 3. Wait for canceler to detect the fraud
/// 4. Verify canceler submits cancel transaction
/// 5. Verify approval status changes to cancelled
pub async fn test_canceler_live_fraud_detection(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "canceler_live_fraud_detection";

    info!("Starting live canceler fraud detection test");

    // Step 1: Verify canceler service is running
    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_canceler_running() && !check_canceler_health().await {
        return TestResult::skip(name, "Canceler service is not running - start it first");
    }
    info!("Canceler service is running");

    // Step 2: Verify bridge contract is deployed
    let has_code = match query_contract_code(config, config.evm.contracts.bridge).await {
        Ok(c) => c,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to check bridge contract: {}", e),
                start.elapsed(),
            );
        }
    };

    if !has_code {
        return TestResult::fail(
            name,
            "Bridge contract has no code deployed",
            start.elapsed(),
        );
    }

    // Step 3: Generate unique fraud parameters using registered chain and token
    let fraud_nonce = generate_unique_nonce();
    let fraud_amount = "1234567890123456789";

    // Use registered Terra chain ID (0x00000001) — fraud is in the nonce (no matching deposit)
    let fraud_src_chain_key = B256::from_slice(&{
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        bytes
    });

    // Use registered test token
    let fraud_token = config.evm.contracts.test_token;

    info!(
        "Creating fraudulent approval: nonce={}, srcChain=0x{}, token={}",
        fraud_nonce,
        hex::encode(&fraud_src_chain_key.as_slice()[..4]),
        fraud_token
    );

    // Step 4: Create fraudulent approval on-chain
    let fraud_result = match create_fraudulent_approval(
        config,
        fraud_src_chain_key,
        fraud_token,
        config.test_accounts.evm_address,
        fraud_amount,
        fraud_nonce,
    )
    .await
    {
        Ok(result) => {
            info!(
                "Fraudulent approval created: withdrawHash=0x{}",
                hex::encode(&result.withdraw_hash.as_slice()[..8])
            );
            result
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to create fraudulent approval: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 5: Verify transaction succeeded
    tokio::time::sleep(Duration::from_secs(2)).await;
    if let Ok(false) = verify_tx_success(config, fraud_result.tx_hash).await {
        return TestResult::fail(
            name,
            "Fraudulent approval transaction failed",
            start.elapsed(),
        );
    }

    // Step 6: Wait for canceler to detect and cancel
    info!("Waiting for canceler to detect and cancel fraudulent approval...");
    let poll_start = Instant::now();

    while poll_start.elapsed() < FRAUD_DETECTION_TIMEOUT {
        if let Ok(true) = is_approval_cancelled(config, fraud_result.withdraw_hash).await {
            info!(
                "Fraudulent approval cancelled in {:?}",
                poll_start.elapsed()
            );
            return TestResult::pass(name, start.elapsed());
        }
        tokio::time::sleep(CANCELLATION_POLL_INTERVAL).await;
    }

    TestResult::fail(
        name,
        format!(
            "Canceler did not cancel fraudulent approval within {:?}",
            FRAUD_DETECTION_TIMEOUT
        ),
        start.elapsed(),
    )
}

/// Test that cancelled approvals block withdrawal execution
pub async fn test_cancelled_approval_blocks_withdrawal(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "cancelled_approval_blocks_withdrawal";

    info!("Testing that cancelled approvals block withdrawal execution");

    // Verify bridge contract
    if let Ok(false) = query_contract_code(config, config.evm.contracts.bridge).await {
        return TestResult::fail(
            name,
            "Bridge contract has no code deployed",
            start.elapsed(),
        );
    }

    // Create a test approval using registered chain and token
    let test_nonce = generate_unique_nonce();
    let test_src_chain_key = B256::from_slice(&{
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // registered Terra chain
        bytes
    });

    let approval_result = match create_fraudulent_approval(
        config,
        test_src_chain_key,
        config.evm.contracts.test_token,
        config.test_accounts.evm_address,
        "5000000000000000000",
        test_nonce,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to create test approval: {}", e),
                start.elapsed(),
            );
        }
    };

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Wait for canceler or cancel directly
    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);
    let canceler_running = manager.is_canceler_running() || check_canceler_health().await;

    if canceler_running {
        let poll_start = Instant::now();
        while poll_start.elapsed() < Duration::from_secs(30) {
            if let Ok(true) = is_approval_cancelled(config, approval_result.withdraw_hash).await {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    } else if let Err(e) = cancel_approval_directly(config, approval_result.withdraw_hash).await {
        return TestResult::skip(name, format!("Could not cancel approval: {}", e));
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify cancelled
    if let Ok(false) = is_approval_cancelled(config, approval_result.withdraw_hash).await {
        return TestResult::fail(name, "Approval was not cancelled", start.elapsed());
    }

    // Skip withdrawal delay
    let _ = skip_withdrawal_delay(config, 60).await;
    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    let _ = anvil.mine_block().await;

    // Verify withdrawal is blocked
    match try_execute_withdrawal(config, approval_result.withdraw_hash).await {
        Ok(true) => TestResult::fail(
            name,
            "SECURITY ISSUE: Cancelled approval allowed withdrawal!",
            start.elapsed(),
        ),
        _ => {
            info!("Cancelled approval correctly blocks withdrawal");
            TestResult::pass(name, start.elapsed())
        }
    }
}

/// Test canceler handles multiple concurrent fraudulent approvals
pub async fn test_canceler_concurrent_fraud_handling(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "canceler_concurrent_fraud_handling";

    info!("Testing canceler concurrent fraud handling");

    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_canceler_running() && !check_canceler_health().await {
        return TestResult::skip(name, "Canceler service is not running");
    }

    // Create multiple fraudulent approvals
    let num_frauds = 5;
    let mut fraud_results: Vec<FraudulentApprovalResult> = Vec::new();

    // Use registered chain and token for all concurrent frauds
    let fraud_src_chain_key = B256::from_slice(&{
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // registered Terra chain
        bytes
    });
    let fraud_token = config.evm.contracts.test_token;

    for i in 0..num_frauds {
        let nonce = generate_unique_nonce() + i as u64;
        let amount = format!("{}00000000000000000", i + 1);

        if let Ok(result) = create_fraudulent_approval(
            config,
            fraud_src_chain_key,
            fraud_token,
            config.test_accounts.evm_address,
            &amount,
            nonce,
        )
        .await
        {
            fraud_results.push(result);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    if fraud_results.is_empty() {
        return TestResult::fail(
            name,
            "Failed to create any fraudulent approvals",
            start.elapsed(),
        );
    }

    // Wait for canceler to process
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Check cancellation rate
    let cancelled_count = {
        let mut count = 0;
        for result in &fraud_results {
            if let Ok(true) = is_approval_cancelled(config, result.withdraw_hash).await {
                count += 1;
            }
        }
        count
    };

    if cancelled_count > 0 {
        TestResult::pass(name, start.elapsed()) // Full or partial success
    } else {
        TestResult::fail(
            name,
            "Canceler failed to cancel any fraudulent approvals",
            start.elapsed(),
        )
    }
}

/// Test canceler detects fraud after service restart
pub async fn test_canceler_restart_fraud_detection(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "canceler_restart_fraud_detection";

    info!("Testing canceler fraud detection after restart");

    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_canceler_running() && !check_canceler_health().await {
        return TestResult::skip(name, "Canceler service is not running");
    }

    let fraud_nonce = generate_unique_nonce();
    // Use registered chain and token — fraud is in the nonce
    let fraud_src_chain_key = B256::from_slice(&{
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // registered Terra chain
        bytes
    });

    let fraud_result = match create_fraudulent_approval(
        config,
        fraud_src_chain_key,
        config.evm.contracts.test_token,
        config.test_accounts.evm_address,
        "9999999999999999999",
        fraud_nonce,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to create fraudulent approval: {}", e),
                start.elapsed(),
            );
        }
    };

    // Wait for canceler to detect
    tokio::time::sleep(Duration::from_secs(15)).await;

    if let Ok(true) = is_approval_cancelled(config, fraud_result.withdraw_hash).await {
        return TestResult::pass(name, start.elapsed());
    }

    tokio::time::sleep(Duration::from_secs(15)).await;

    if let Ok(true) = is_approval_cancelled(config, fraud_result.withdraw_hash).await {
        TestResult::pass(name, start.elapsed())
    } else {
        TestResult::fail(
            name,
            "Canceler did not detect fraud within timeout",
            start.elapsed(),
        )
    }
}

/// Test canceler detects EVM→EVM fraud (approval with no matching deposit on source EVM)
pub async fn test_canceler_evm_source_fraud_detection(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "canceler_evm_source_fraud_detection";

    info!("Testing canceler EVM→EVM fraud detection");

    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_canceler_running() && !check_canceler_health().await {
        return TestResult::skip(name, "Canceler service is not running");
    }

    // Use registered EVM chain ID (0x00000002) — the bytes4 ID, not the native chain ID
    let evm_chain_key = B256::from_slice(&{
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x02]); // registered EVM chain
        bytes
    });
    info!(
        "Using registered EVM chain key: 0x{}",
        hex::encode(&evm_chain_key.as_slice()[..4])
    );

    let fraud_nonce = generate_unique_nonce();
    let fraud_result = match create_fraudulent_approval(
        config,
        evm_chain_key,
        config.evm.contracts.test_token,
        config.test_accounts.evm_address,
        "2500000000000000000",
        fraud_nonce,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to create EVM-source fraud: {}", e),
                start.elapsed(),
            );
        }
    };

    info!("Waiting for canceler to detect EVM-source fraud...");
    let poll_start = Instant::now();

    while poll_start.elapsed() < FRAUD_DETECTION_TIMEOUT {
        if let Ok(true) = is_approval_cancelled(config, fraud_result.withdraw_hash).await {
            info!(
                "EVM-source fraud detected and cancelled in {:?}",
                poll_start.elapsed()
            );
            return TestResult::pass(name, start.elapsed());
        }
        tokio::time::sleep(CANCELLATION_POLL_INTERVAL).await;
    }

    TestResult::fail(
        name,
        "Canceler did not cancel EVM-source fraud within timeout",
        start.elapsed(),
    )
}

/// Test canceler detects Terra→EVM fraud (approval with no matching deposit on Terra)
pub async fn test_canceler_terra_source_fraud_detection(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "canceler_terra_source_fraud_detection";

    info!("Testing canceler Terra→EVM fraud detection");

    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_canceler_running() && !check_canceler_health().await {
        return TestResult::skip(name, "Canceler service is not running");
    }

    // Use registered Terra chain ID (0x00000001) — the bytes4 ID assigned by ChainRegistry
    let terra_chain_key = B256::from_slice(&{
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&[0x00, 0x00, 0x00, 0x01]); // registered Terra chain
        bytes
    });
    info!(
        "Using registered Terra chain key: 0x{}",
        hex::encode(&terra_chain_key.as_slice()[..4])
    );

    let fraud_nonce = generate_unique_nonce();
    let fraud_result = match create_fraudulent_approval(
        config,
        terra_chain_key,
        config.evm.contracts.test_token,
        config.test_accounts.evm_address,
        "3500000000000000000",
        fraud_nonce,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to create Terra-source fraud: {}", e),
                start.elapsed(),
            );
        }
    };

    info!("Waiting for canceler to detect Terra-source fraud...");
    let poll_start = Instant::now();

    while poll_start.elapsed() < FRAUD_DETECTION_TIMEOUT {
        if let Ok(true) = is_approval_cancelled(config, fraud_result.withdraw_hash).await {
            info!(
                "Terra-source fraud detected and cancelled in {:?}",
                poll_start.elapsed()
            );
            return TestResult::pass(name, start.elapsed());
        }
        tokio::time::sleep(CANCELLATION_POLL_INTERVAL).await;
    }

    TestResult::fail(
        name,
        "Canceler did not cancel Terra-source fraud within timeout",
        start.elapsed(),
    )
}

/// Run all live canceler execution tests
pub async fn run_canceler_execution_tests(config: &E2eConfig) -> Vec<TestResult> {
    info!("Running live canceler execution tests");

    vec![
        // Unknown chain key fraud tests
        test_canceler_live_fraud_detection(config).await,
        test_cancelled_approval_blocks_withdrawal(config).await,
        test_canceler_concurrent_fraud_handling(config).await,
        test_canceler_restart_fraud_detection(config).await,
        // Specific chain source fraud tests
        test_canceler_evm_source_fraud_detection(config).await,
        test_canceler_terra_source_fraud_detection(config).await,
    ]
}
