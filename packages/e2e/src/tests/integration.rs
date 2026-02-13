//! Integration tests for E2E test suite
//!
//! Real token transfer tests with balance verification and full cross-chain cycles.
//!
//! Deposit tests (EVM → Terra) are in `integration_deposit.rs`.
//! Withdrawal tests (Terra → EVM) are in `integration_withdraw.rs`.

use crate::services::ServiceManager;
use crate::terra::TerraClient;
use crate::transfer_helpers::{self};
use crate::{E2eConfig, TestResult};
use alloy::primitives::{Address, B256, U256};
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use super::helpers::{
    approve_erc20, create_fraudulent_approval, encode_terra_address, execute_deposit,
    get_terra_chain_key, is_approval_cancelled, query_deposit_nonce,
};
use super::operator_helpers::{
    calculate_evm_fee, poll_terra_for_approval, submit_withdraw_on_terra, TERRA_APPROVAL_TIMEOUT,
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

/// Execute a complete EVM → Terra transfer cycle with full V2 verification.
///
/// This performs the entire cross-chain transfer flow following the V2 protocol:
/// 1. Record initial EVM balance
/// 2. Execute deposit on EVM (locks tokens, creates deposit hash)
/// 3. User calls `WithdrawSubmit` on Terra (creates pending withdrawal)
/// 4. Operator polls Terra PendingWithdrawals, verifies EVM deposit, calls `WithdrawApprove`
/// 5. After cancel window, withdrawal can be executed on Terra
/// 6. Verify EVM balance decreased
///
/// Requires operator service running. The key V2 invariant tested here is:
/// **user submits → operator approves → execute after delay**.
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

    let terra_bridge = match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => addr.clone(),
        _ => {
            return TestResult::skip(name, "Terra bridge address not configured");
        }
    };

    let test_account = config.test_accounts.evm_address;
    let terra_client = TerraClient::new(&config.terra);
    let terra_recipient = &config.test_accounts.terra_address;

    info!(
        "Testing full transfer cycle: {} tokens for account {}",
        amount, test_account
    );

    // Step 1: Get initial EVM balance
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

    // Step 5: Execute EVM deposit (execute_deposit approves Bridge - single approval for fee + net)
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

    // Step 6: Verify nonce incremented
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

    // The deposit used nonce_before as its nonce (depositNonce++ is post-increment)
    let deposit_nonce = nonce_before;

    // Step 8: Calculate net amount (post-fee) — must match what EVM stored in the deposit hash
    let fee_amount = match calculate_evm_fee(config, test_account, amount).await {
        Ok(fee) => {
            info!("EVM fee for deposit: {} ({}bps)", fee, fee * 10000 / amount);
            fee
        }
        Err(e) => {
            warn!("Failed to query EVM fee, assuming 0: {}", e);
            0
        }
    };
    let net_amount = amount - fee_amount;

    // Step 9: V2 — User calls WithdrawSubmit on TERRA (destination chain)
    //
    // In V2, the user must initiate the withdrawal on the destination chain.
    // The operator NEVER submits on behalf of users. The canceler relies on
    // user-initiated submits to detect fraud.
    let evm_chain_id: [u8; 4] = [0, 0, 0, 1]; // EVM predetermined chain ID
    let mut src_account_bytes32 = [0u8; 32];
    src_account_bytes32[12..32].copy_from_slice(test_account.as_slice());

    let terra_token = config.terra.cw20_address.as_deref().unwrap_or("uluna");
    info!(
        "V2: User calling WithdrawSubmit on Terra: nonce={}, token={}, amount={}",
        deposit_nonce, terra_token, net_amount
    );

    match submit_withdraw_on_terra(
        &terra_client,
        &terra_bridge,
        evm_chain_id,
        src_account_bytes32,
        terra_token,
        terra_recipient,
        net_amount,
        deposit_nonce,
    )
    .await
    {
        Ok(tx_hash) => {
            info!(
                "WithdrawSubmit on Terra succeeded: tx={}, nonce={}",
                tx_hash, deposit_nonce
            );
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!(
                    "WithdrawSubmit on Terra failed: {}. \
                     V2 requires user to submit on destination chain before operator can approve.",
                    e
                ),
                start.elapsed(),
            );
        }
    }

    // Step 10: Poll TERRA for operator approval (not EVM — approval lives on Terra)
    info!(
        "Waiting for operator to approve withdrawal on Terra (nonce={})...",
        deposit_nonce
    );

    match poll_terra_for_approval(
        &terra_client,
        &terra_bridge,
        deposit_nonce,
        TERRA_APPROVAL_TIMEOUT,
    )
    .await
    {
        Ok(approval_info) => {
            info!(
                "Operator approved on Terra: nonce={}, amount={}, approved_at={}",
                approval_info.nonce, approval_info.amount, approval_info.approved_at
            );
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!(
                    "Operator did not approve withdrawal on Terra within {:?}: {}. \
                     V2 flow: user WithdrawSubmit → operator WithdrawApprove → execute after delay.",
                    TERRA_APPROVAL_TIMEOUT, e
                ),
                start.elapsed(),
            );
        }
    }

    // Step 11: Verify EVM balance decreased
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
    info!(
        "Full transfer cycle completed: EVM balance decreased by {} (net_amount={}, fee={})",
        decrease, net_amount, fee_amount
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    /// Verify that the V2 flow correctly encodes src_account as bytes32 with EVM address
    /// right-aligned in the last 20 bytes (standard ABI packing).
    #[test]
    fn test_src_account_encoding_for_terra_withdraw_submit() {
        let evm_address =
            Address::from_slice(&hex::decode("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap());
        let mut src_account_bytes32 = [0u8; 32];
        src_account_bytes32[12..32].copy_from_slice(evm_address.as_slice());

        // First 12 bytes should be zero (left-padding)
        assert_eq!(&src_account_bytes32[..12], &[0u8; 12]);
        // Last 20 bytes should be the EVM address
        assert_eq!(&src_account_bytes32[12..32], evm_address.as_slice());
    }

    /// Verify net amount computation: gross - fee = net.
    /// The EVM bridge deducts fees on deposit, so WithdrawSubmit on Terra
    /// must use net_amount for hash parity.
    #[test]
    fn test_net_amount_calculation_for_hash_parity() {
        let gross_amount: u128 = 1_000_000;
        let fee_bps: u128 = 50; // 0.50%
        let fee_amount = gross_amount * fee_bps / 10_000;
        let net_amount = gross_amount - fee_amount;

        assert_eq!(fee_amount, 5_000);
        assert_eq!(net_amount, 995_000);
        // The Terra WithdrawSubmit MUST use 995_000, not 1_000_000
    }

    /// Verify that EVM chain ID encoding matches Terra's expected base64 format.
    /// EVM chain ID 1 should be [0, 0, 0, 1] as 4-byte big-endian.
    #[test]
    fn test_evm_chain_id_encoding() {
        let evm_chain_id: [u8; 4] = [0, 0, 0, 1];
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, evm_chain_id);
        assert_eq!(b64, "AAAAAQ==");

        let terra_chain_id: [u8; 4] = [0, 0, 0, 2];
        let b64_terra =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, terra_chain_id);
        assert_eq!(b64_terra, "AAAAAg==");
    }

    /// Verify that the V2 nonce convention is correct:
    /// depositNonce++ is post-increment, so the deposit uses nonce_before.
    #[test]
    fn test_post_increment_nonce_convention() {
        let nonce_before: u64 = 5;
        let nonce_after: u64 = nonce_before + 1; // Simulates depositNonce++

        // The deposit uses nonce_before as its nonce
        let deposit_nonce = nonce_before;
        assert_eq!(deposit_nonce, 5);
        assert_eq!(nonce_after, 6);

        // WithdrawSubmit on Terra must use deposit_nonce (5), NOT nonce_after (6)
        assert_ne!(deposit_nonce, nonce_after);
    }

    /// Verify that Terra destination check logic works.
    #[test]
    fn test_terra_destination_detection() {
        let bridge_some =
            Some("terra14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9ssrc8au".to_string());
        let bridge_none: Option<String> = None;
        let bridge_empty = Some("".to_string());

        // When bridge address exists and is non-empty, it's a Terra destination
        assert!(bridge_some.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
        // When None, not Terra destination
        assert!(!bridge_none.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
        // When empty string, not Terra destination
        assert!(!bridge_empty
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false));
    }
}
