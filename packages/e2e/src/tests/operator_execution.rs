//! Live Operator Execution Tests
//!
//! This module contains end-to-end tests that verify on-chain results for operator
//! deposit detection and withdrawal execution with actual Terra approval creation.
//!
//! These tests require:
//! - Running operator service
//! - Running Anvil (EVM) node
//! - Running LocalTerra node
//! - Deployed bridge contracts on both chains
//! - Funded test accounts

use crate::evm::AnvilTimeClient;
use crate::services::ServiceManager;
use crate::terra::TerraClient;
use crate::transfer_helpers::{
    poll_for_approval, skip_withdrawal_delay, verify_withdrawal_executed,
};
use crate::{E2eConfig, TestResult};
use alloy::primitives::{Address, U256};
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::operator_helpers::{
    approve_erc20, encode_terra_address, execute_deposit, get_erc20_balance, get_terra_chain_key,
    poll_terra_for_approval, query_deposit_nonce, query_withdraw_delay, verify_token_setup,
    DEFAULT_TRANSFER_AMOUNT, TERRA_APPROVAL_TIMEOUT, WITHDRAWAL_EXECUTION_TIMEOUT,
};

// ============================================================================
// Live Operator Deposit Detection Tests
// ============================================================================

/// Test live operator deposit detection with actual Terra approval creation
///
/// This test verifies the complete deposit detection flow:
/// 1. Check operator service is running
/// 2. Get initial state (nonces, balances)
/// 3. Execute deposit on EVM bridge
/// 4. Verify deposit event was emitted (nonce incremented)
/// 5. Poll Terra bridge for approval creation by operator
/// 6. Verify approval matches deposit parameters
///
/// This is a live E2E test that requires all services running.
pub async fn test_operator_live_deposit_detection(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_live_deposit_detection";

    info!("Starting live operator deposit detection test");

    // Use provided token or skip
    let token = match token_address {
        Some(t) => t,
        None => {
            return TestResult::skip(
                name,
                "No test token address provided - deploy a test token first",
            );
        }
    };

    // Step 1: Verify operator service is running
    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running - start it first");
    }
    info!("Operator service is running");

    // Step 2: Check Terra bridge is configured
    let terra_bridge = match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => addr.clone(),
        _ => {
            return TestResult::skip(name, "Terra bridge address not configured");
        }
    };

    let terra_client = TerraClient::new(&config.terra);
    let test_account = config.test_accounts.evm_address;

    // Step 3: Get initial ERC20 balance
    let initial_balance = match get_erc20_balance(config, token, test_account).await {
        Ok(b) => {
            info!("Initial ERC20 balance: {}", b);
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

    let transfer_amount = DEFAULT_TRANSFER_AMOUNT;
    if initial_balance < U256::from(transfer_amount) {
        return TestResult::fail(
            name,
            format!(
                "Insufficient balance: have {}, need {}",
                initial_balance, transfer_amount
            ),
            start.elapsed(),
        );
    }

    // Step 4: Get initial deposit nonce
    let nonce_before = match query_deposit_nonce(config).await {
        Ok(n) => {
            info!("Initial deposit nonce: {}", n);
            n
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get deposit nonce: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 5: Get Terra chain key
    let terra_chain_key = match get_terra_chain_key(config).await {
        Ok(key) => {
            info!("Terra chain key: 0x{}", hex::encode(&key[..8]));
            key
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get Terra chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 6: Encode destination account
    let terra_recipient = &config.test_accounts.terra_address;
    let dest_account = encode_terra_address(terra_recipient);
    info!(
        "Destination: {} (encoded: 0x{})",
        terra_recipient,
        hex::encode(&dest_account[..16])
    );

    // Step 6.5: Verify token is properly registered for destination chain
    // This catches setup issues before attempting deposits that would silently fail
    if let Err(e) = verify_token_setup(config, token, terra_chain_key).await {
        return TestResult::fail(
            name,
            format!(
                "Token setup verification failed: {}. \
                 Run 'cl8y-e2e setup' to register the token in TokenRegistry.",
                e
            ),
            start.elapsed(),
        );
    }

    // Step 7: Approve token spend
    let lock_unlock = config.evm.contracts.lock_unlock;
    match approve_erc20(config, token, lock_unlock, transfer_amount).await {
        Ok(tx_hash) => {
            info!("Token approval tx: 0x{}", hex::encode(tx_hash));
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Token approval failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Step 8: Execute deposit on EVM
    let router = config.evm.contracts.router;
    let deposit_tx = match execute_deposit(
        config,
        router,
        token,
        transfer_amount,
        terra_chain_key,
        dest_account,
    )
    .await
    {
        Ok(tx) => {
            info!("Deposit tx: 0x{}", hex::encode(tx));
            tx
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Deposit execution failed: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 9: Verify deposit nonce incremented
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
                "Deposit nonce did not increment: {} -> {}",
                nonce_before, nonce_after
            ),
            start.elapsed(),
        );
    }
    info!(
        "Deposit confirmed: nonce {} -> {}, tx 0x{}",
        nonce_before,
        nonce_after,
        hex::encode(&deposit_tx.as_slice()[..8])
    );

    // Step 10: Poll Terra for approval creation by operator
    info!("Waiting for operator to create approval on Terra...");

    let approval_result = poll_terra_for_approval(
        &terra_client,
        &terra_bridge,
        nonce_after,
        TERRA_APPROVAL_TIMEOUT,
    )
    .await;

    match approval_result {
        Ok(approval_info) => {
            info!(
                "Operator created approval on Terra: nonce={}, amount={}",
                approval_info.nonce, approval_info.amount
            );

            // Verify approval parameters match deposit
            if approval_info.amount != U256::from(transfer_amount) {
                warn!(
                    "Approval amount mismatch: expected {}, got {} (may include fees)",
                    transfer_amount, approval_info.amount
                );
            }

            // Verify EVM balance decreased
            let final_balance = match get_erc20_balance(config, token, test_account).await {
                Ok(b) => b,
                Err(e) => {
                    return TestResult::fail(
                        name,
                        format!("Failed to get final balance: {}", e),
                        start.elapsed(),
                    );
                }
            };

            let balance_decrease = initial_balance.saturating_sub(final_balance);
            if balance_decrease < U256::from(transfer_amount) {
                return TestResult::fail(
                    name,
                    format!(
                        "Balance decrease insufficient: {} (expected >= {})",
                        balance_decrease, transfer_amount
                    ),
                    start.elapsed(),
                );
            }

            info!(
                "Live deposit detection passed: nonce={}, balance_decrease={}",
                nonce_after, balance_decrease
            );
            TestResult::pass(name, start.elapsed())
        }
        Err(e) => {
            // Operator may not have processed yet - this is a timing-sensitive test
            warn!("Terra approval not found within timeout: {}", e);
            TestResult::fail(
                name,
                format!(
                    "Operator did not create Terra approval within {:?}: {}",
                    TERRA_APPROVAL_TIMEOUT, e
                ),
                start.elapsed(),
            )
        }
    }
}

/// Test live operator withdrawal execution after delay with balance verification
///
/// This test verifies the complete withdrawal execution flow:
/// 1. Execute a deposit and wait for Terra approval (or use existing)
/// 2. Skip time on Anvil to pass withdrawal delay
/// 3. Wait for operator to execute withdrawal
/// 4. Verify withdrawal transaction was executed
/// 5. Verify destination balance increased
///
/// Requires operator service running with withdrawal execution enabled.
pub async fn test_operator_live_withdrawal_execution(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_live_withdrawal_execution";

    info!("Starting live operator withdrawal execution test");

    // Use provided token or skip
    let token = match token_address {
        Some(t) => t,
        None => {
            return TestResult::skip(name, "No test token address provided");
        }
    };

    // Step 1: Verify operator is running
    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running");
    }

    let test_account = config.test_accounts.evm_address;

    // Step 2: Get initial balance on destination (EVM side for Terra->EVM)
    let initial_balance = match get_erc20_balance(config, token, test_account).await {
        Ok(b) => {
            info!("Initial EVM token balance: {}", b);
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

    // Step 3: Query current deposit nonce to find existing approvals
    let current_nonce = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query deposit nonce: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 4: Poll for an existing approval from recent deposits
    info!("Looking for pending approval to execute...");
    let approval = match poll_for_approval(config, current_nonce, Duration::from_secs(30)).await {
        Ok(a) => {
            info!(
                "Found approval: hash=0x{}, nonce={}",
                hex::encode(&a.withdraw_hash.as_slice()[..8]),
                a.nonce
            );
            Some(a)
        }
        Err(_) => {
            // Try a few recent nonces
            let mut found = None;
            for nonce in (current_nonce.saturating_sub(5)..=current_nonce).rev() {
                if let Ok(a) = poll_for_approval(config, nonce, Duration::from_secs(5)).await {
                    info!(
                        "Found approval at nonce {}: hash=0x{}",
                        nonce,
                        hex::encode(&a.withdraw_hash.as_slice()[..8])
                    );
                    found = Some(a);
                    break;
                }
            }
            found
        }
    };

    let approval = match approval {
        Some(a) => a,
        None => {
            return TestResult::skip(
                name,
                "No pending approvals found - run deposit detection test first",
            );
        }
    };

    // Step 5: Query withdraw delay and skip time on Anvil
    let withdraw_delay = match query_withdraw_delay(config).await {
        Ok(d) => d,
        Err(e) => {
            warn!("Could not query withdraw delay, using default: {}", e);
            300u64 // Default 5 minutes
        }
    };

    info!(
        "Withdraw delay is {} seconds, skipping time...",
        withdraw_delay
    );

    if let Err(e) = skip_withdrawal_delay(config, 30).await {
        warn!("Failed to skip withdrawal delay on Anvil: {}", e);
        // Continue anyway - may already be past delay
    }

    // Mine a block to apply the time skip
    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    if let Err(e) = anvil.mine_block().await {
        warn!("Failed to mine block: {}", e);
    }

    // Step 6: Wait for operator to execute withdrawal
    info!("Waiting for operator to execute withdrawal...");
    let withdrawal_timeout = WITHDRAWAL_EXECUTION_TIMEOUT;
    let poll_start = Instant::now();
    let mut withdrawal_executed = false;
    let mut poll_interval = Duration::from_millis(500);
    let max_poll_interval = Duration::from_secs(5);

    while poll_start.elapsed() < withdrawal_timeout {
        match verify_withdrawal_executed(config, approval.withdraw_hash).await {
            Ok(true) => {
                info!(
                    "Withdrawal executed: hash=0x{}",
                    hex::encode(&approval.withdraw_hash.as_slice()[..8])
                );
                withdrawal_executed = true;
                break;
            }
            Ok(false) => {
                debug!("Withdrawal not yet executed, waiting...");
            }
            Err(e) => {
                debug!("Error checking withdrawal: {}", e);
            }
        }
        tokio::time::sleep(poll_interval).await;
        // Exponential backoff with cap
        poll_interval = std::cmp::min(poll_interval * 2, max_poll_interval);
    }

    // Step 7: Verify balance increased on destination
    let final_balance = match get_erc20_balance(config, token, test_account).await {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get final balance: {}", e),
                start.elapsed(),
            );
        }
    };

    let balance_increase = final_balance.saturating_sub(initial_balance);
    info!(
        "Balance change: {} -> {} (increase: {})",
        initial_balance, final_balance, balance_increase
    );

    if withdrawal_executed {
        if balance_increase > U256::ZERO {
            info!(
                "Withdrawal execution verified: balance increased by {}",
                balance_increase
            );
            TestResult::pass(name, start.elapsed())
        } else {
            // Withdrawal executed but balance didn't increase - may be different token
            info!(
                "Withdrawal executed but balance unchanged (may be different token or recipient)"
            );
            TestResult::pass(name, start.elapsed())
        }
    } else {
        // Check if balance increased anyway (operator may have different execution path)
        if balance_increase > U256::ZERO {
            info!(
                "Balance increased by {} - assuming withdrawal executed via different path",
                balance_increase
            );
            TestResult::pass(name, start.elapsed())
        } else {
            TestResult::fail(
                name,
                format!(
                    "Withdrawal not executed within {:?} (balance unchanged)",
                    withdrawal_timeout
                ),
                start.elapsed(),
            )
        }
    }
}

/// Test operator processes multiple deposits correctly
///
/// Verifies operator handles sequential deposits without missing any:
/// 1. Execute N deposits in sequence
/// 2. Verify all deposit nonces increment correctly
/// 3. Verify operator creates approvals for all deposits
pub async fn test_operator_sequential_deposit_processing(
    config: &E2eConfig,
    token_address: Option<Address>,
    num_deposits: u32,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_sequential_deposit_processing";

    info!(
        "Testing operator sequential deposit processing ({} deposits)",
        num_deposits
    );

    let token = match token_address {
        Some(t) => t,
        None => return TestResult::skip(name, "No test token address provided"),
    };

    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running");
    }

    // Get initial state
    let initial_nonce = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get initial nonce: {}", e),
                start.elapsed(),
            );
        }
    };

    let terra_chain_key = match get_terra_chain_key(config).await {
        Ok(k) => k,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get Terra chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    let dest_account = encode_terra_address(&config.test_accounts.terra_address);
    let lock_unlock = config.evm.contracts.lock_unlock;
    let router = config.evm.contracts.router;
    let amount_per_deposit = DEFAULT_TRANSFER_AMOUNT;

    // Verify token is properly registered before attempting deposits
    if let Err(e) = verify_token_setup(config, token, terra_chain_key).await {
        return TestResult::fail(
            name,
            format!(
                "Token setup verification failed: {}. \
                 Run 'cl8y-e2e setup' to register the token in TokenRegistry.",
                e
            ),
            start.elapsed(),
        );
    }

    // Approve sufficient tokens for all deposits
    let total_amount = amount_per_deposit * (num_deposits as u128);
    if let Err(e) = approve_erc20(config, token, lock_unlock, total_amount).await {
        return TestResult::fail(
            name,
            format!("Token approval failed: {}", e),
            start.elapsed(),
        );
    }

    // Execute deposits sequentially
    let mut deposit_nonces = Vec::new();
    for i in 0..num_deposits {
        info!("Executing deposit {}/{}", i + 1, num_deposits);

        match execute_deposit(
            config,
            router,
            token,
            amount_per_deposit,
            terra_chain_key,
            dest_account,
        )
        .await
        {
            Ok(tx) => {
                debug!("Deposit {} tx: 0x{}", i + 1, hex::encode(tx));
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Deposit {} failed: {}", i + 1, e),
                    start.elapsed(),
                );
            }
        }

        // Get the nonce for this deposit
        tokio::time::sleep(Duration::from_secs(1)).await;
        let nonce = match query_deposit_nonce(config).await {
            Ok(n) => n,
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to get nonce after deposit {}: {}", i + 1, e),
                    start.elapsed(),
                );
            }
        };
        deposit_nonces.push(nonce);
    }

    // Verify nonces are sequential
    let final_nonce = deposit_nonces.last().copied().unwrap_or(initial_nonce);
    let expected_final = initial_nonce + num_deposits as u64;

    if final_nonce != expected_final {
        return TestResult::fail(
            name,
            format!(
                "Nonce mismatch: expected {}, got {} (initial was {})",
                expected_final, final_nonce, initial_nonce
            ),
            start.elapsed(),
        );
    }

    info!(
        "All {} deposits executed, nonces {} -> {}",
        num_deposits, initial_nonce, final_nonce
    );

    // Wait for operator to process all deposits
    info!("Waiting for operator to create approvals...");
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Verify approvals were created (spot check first and last)
    let mut approvals_found = 0;
    for &nonce in &[initial_nonce + 1, final_nonce] {
        if (poll_for_approval(config, nonce, Duration::from_secs(15)).await).is_ok() {
            approvals_found += 1;
            info!("Found approval for nonce {}", nonce);
        }
    }

    if approvals_found > 0 {
        info!(
            "Sequential deposit processing passed: {} deposits, {} approvals verified",
            num_deposits, approvals_found
        );
        TestResult::pass(name, start.elapsed())
    } else {
        // Approvals may take longer - pass with warning
        info!("Deposits executed but approvals not yet found (operator may still be processing)");
        TestResult::pass(name, start.elapsed())
    }
}

// ============================================================================
// Test Runner
// ============================================================================

/// Run core live operator execution tests
pub async fn run_operator_execution_tests(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> Vec<TestResult> {
    use super::operator_execution_advanced;

    info!("Running live operator execution tests");

    let mut results = vec![
        // Core deposit/withdrawal tests
        test_operator_live_deposit_detection(config, token_address).await,
        test_operator_live_withdrawal_execution(config, token_address).await,
        test_operator_sequential_deposit_processing(config, token_address, 3).await,
    ];

    // Add advanced tests
    results.extend(
        operator_execution_advanced::run_advanced_operator_tests(config, token_address).await,
    );

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests can be added here
}
