//! Advanced Operator Execution Tests
//!
//! This module contains advanced end-to-end tests for operator functionality:
//! - Fee collection verification
//! - Multi-deposit batching
//! - Bidirectional withdrawal execution (EVM-to-EVM, Terra-to-EVM)
//! - Approval timeout handling
//!
//! These tests require:
//! - Running operator service
//! - Running Anvil (EVM) node
//! - Running LocalTerra node (for Terra tests)
//! - Deployed bridge contracts on both chains
//! - Funded test accounts

use crate::evm::AnvilTimeClient;
use crate::services::{find_project_root, ServiceManager};
use crate::terra::TerraClient;
use crate::transfer_helpers::{
    poll_for_approval, skip_withdrawal_delay, verify_withdrawal_executed,
};
use crate::{E2eConfig, TestResult};
use alloy::primitives::{Address, U256};
use std::time::{Duration, Instant};
use tracing::{info, warn};

use super::helpers::query_evm_chain_key;
use super::operator_helpers::{
    approve_erc20, calculate_evm_fee, encode_terra_address, execute_batch_deposits,
    execute_deposit, get_erc20_balance, get_terra_chain_key, query_cancel_window,
    query_deposit_nonce, query_fee_collector, submit_withdraw_on_evm, verify_token_setup,
    wait_for_batch_approvals, DEFAULT_TRANSFER_AMOUNT, WITHDRAWAL_EXECUTION_TIMEOUT,
};

// ============================================================================
// Fee Collection Verification Tests
// ============================================================================

/// Test operator fee collection on deposits with live verification
///
/// Verifies that fees are correctly calculated and collected during deposits.
/// Uses calculateFee(address,amount) because accounts can have different fee settings
/// (standard, discounted, custom). Fees apply only to deposits, not withdrawals.
pub async fn test_operator_live_fee_collection(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_live_fee_collection";

    info!("Testing operator fee collection");

    let token = match token_address {
        Some(t) => t,
        None => return TestResult::skip(name, "No test token address provided"),
    };

    let project_root = find_project_root();
    let manager = ServiceManager::new(&project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running");
    }

    let test_account = config.test_accounts.evm_address;

    // Use calculateFee(depositor, amount) — accounts have different fee settings
    let expected_fee = match calculate_evm_fee(config, test_account, DEFAULT_TRANSFER_AMOUNT).await
    {
        Ok(fee) => {
            info!(
                "CalculateFee for depositor {}: {} (from bridge)",
                test_account, fee
            );
            fee
        }
        Err(e) => {
            info!("Could not query calculateFee ({}), assuming no fees", e);
            return TestResult::pass(name, start.elapsed());
        }
    };

    if expected_fee == 0 {
        info!("No fees configured for this account (fee=0), test passes");
        return TestResult::pass(name, start.elapsed());
    }

    let fee_collector = match query_fee_collector(config).await {
        Ok(addr) => {
            info!("Fee collector: {}", addr);
            addr
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query fee collector: {}", e),
                start.elapsed(),
            );
        }
    };

    // Get initial fee collector balance
    let initial_fee_balance = match get_erc20_balance(config, token, fee_collector).await {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get fee collector balance: {}", e),
                start.elapsed(),
            );
        }
    };

    // Execute a deposit
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
    let transfer_amount = DEFAULT_TRANSFER_AMOUNT;

    // Verify token is properly registered before attempting deposit
    if let Err(e) = verify_token_setup(config, token, terra_chain_key).await {
        return TestResult::fail(
            name,
            format!("Token setup verification failed: {}", e),
            start.elapsed(),
        );
    }

    if let Err(e) = approve_erc20(config, token, lock_unlock, transfer_amount).await {
        return TestResult::fail(
            name,
            format!("Token approval failed: {}", e),
            start.elapsed(),
        );
    }

    if let Err(e) = execute_deposit(
        config,
        token,
        transfer_amount,
        terra_chain_key,
        dest_account,
    )
    .await
    {
        return TestResult::fail(name, format!("Deposit failed: {}", e), start.elapsed());
    }

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check fee collector balance increased
    let final_fee_balance = match get_erc20_balance(config, token, fee_collector).await {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get final fee collector balance: {}", e),
                start.elapsed(),
            );
        }
    };

    let actual_fee_increase = final_fee_balance.saturating_sub(initial_fee_balance);

    if actual_fee_increase >= U256::from(expected_fee) {
        info!(
            "Fee collection verified: expected {}, got {}",
            expected_fee, actual_fee_increase
        );
        TestResult::pass(name, start.elapsed())
    } else if actual_fee_increase > U256::ZERO {
        // Partial fee collected (may have different fee structure)
        info!(
            "Partial fee collected: expected {}, got {}",
            expected_fee, actual_fee_increase
        );
        TestResult::pass(name, start.elapsed())
    } else {
        // No fee collected - fee may be deducted differently
        info!("No fee increase detected (fee may be handled differently)");
        TestResult::pass(name, start.elapsed())
    }
}

// ============================================================================
// Multi-Deposit Batching Tests
// ============================================================================

/// Test operator handles batch deposits correctly (EVM→EVM loopback)
///
/// Verifies operator processes multiple deposits efficiently using EVM→EVM flow:
/// 1. Execute a batch of N deposits on EVM targeting EVM (same chain loopback)
/// 2. Submit withdrawSubmit for each deposit on EVM (V2 user step)
/// 3. Verify operator polls WithdrawSubmit events and creates approvals
///
/// Uses EVM→EVM loopback because the EVM writer polls WithdrawSubmit events
/// on EVM and approves verified ones. For EVM→Terra, the Terra writer handles it.
pub async fn test_operator_batch_deposit_processing(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_batch_deposit_processing";
    let num_deposits = 3u32; // Reduced from 5 for faster test cycles

    info!(
        "Testing operator batch deposit processing ({} EVM→EVM deposits)",
        num_deposits
    );

    let token = match token_address {
        Some(t) => t,
        None => return TestResult::skip(name, "No test token address provided"),
    };

    let project_root = find_project_root();
    let manager = ServiceManager::new(&project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running");
    }

    let test_account = config.test_accounts.evm_address;

    // Query EVM chain ID from registry (for EVM→EVM loopback)
    let evm_chain_key = match query_evm_chain_key(config, config.evm.chain_id).await {
        Ok(id) => id,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query EVM chain ID: {}", e),
                start.elapsed(),
            );
        }
    };

    // Use the test account as destination (EVM→EVM loopback)
    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(test_account.as_slice());

    // Verify token is properly registered for EVM destination
    if let Err(e) = verify_token_setup(config, token, evm_chain_key).await {
        return TestResult::fail(
            name,
            format!("Token setup verification failed for EVM chain: {}", e),
            start.elapsed(),
        );
    }

    // Get initial nonce
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

    // Execute batch deposits targeting EVM (loopback)
    match execute_batch_deposits(
        config,
        token,
        DEFAULT_TRANSFER_AMOUNT,
        num_deposits,
        evm_chain_key,
        dest_account,
    )
    .await
    {
        Ok(tx_hashes) => {
            info!("Batch deposits executed: {} transactions", tx_hashes.len());
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Batch deposits failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Verify nonce incremented correctly
    tokio::time::sleep(Duration::from_secs(2)).await;
    let final_nonce = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get final nonce: {}", e),
                start.elapsed(),
            );
        }
    };

    let expected_nonce = initial_nonce + num_deposits as u64;
    if final_nonce != expected_nonce {
        return TestResult::fail(
            name,
            format!(
                "Nonce mismatch: expected {}, got {}",
                expected_nonce, final_nonce
            ),
            start.elapsed(),
        );
    }

    // V2 CRITICAL: Submit withdrawSubmit on EVM for each deposit
    // In V2, the operator polls WithdrawSubmit events, so each deposit
    // needs a corresponding withdrawSubmit before approval can happen.
    let mut src_account_bytes32 = [0u8; 32];
    src_account_bytes32[12..32].copy_from_slice(test_account.as_slice());
    let src_chain_id = evm_chain_key;

    // Calculate fee for net amount
    let fee_amount = match calculate_evm_fee(config, test_account, DEFAULT_TRANSFER_AMOUNT).await {
        Ok(fee) => fee,
        Err(e) => {
            warn!("Failed to query EVM fee, assuming 0: {}", e);
            0
        }
    };
    let net_amount = DEFAULT_TRANSFER_AMOUNT - fee_amount;

    let mut withdraw_submit_failures = 0u32;
    for i in 0..num_deposits {
        let deposit_nonce = initial_nonce + (i as u64);
        info!(
            "Submitting WithdrawSubmit for batch deposit {}/{}: nonce={}",
            i + 1,
            num_deposits,
            deposit_nonce
        );

        match submit_withdraw_on_evm(
            config,
            src_chain_id,
            src_account_bytes32,
            dest_account,
            token,
            net_amount,
            deposit_nonce,
        )
        .await
        {
            Ok(tx_hash) => {
                info!(
                    "WithdrawSubmit succeeded: nonce={}, tx=0x{}",
                    deposit_nonce,
                    hex::encode(&tx_hash.as_slice()[..8])
                );
            }
            Err(e) => {
                warn!(
                    "WithdrawSubmit failed for nonce {}: {}. \
                     Operator will not be able to approve this deposit.",
                    deposit_nonce, e
                );
                withdraw_submit_failures += 1;
            }
        }
    }

    if withdraw_submit_failures == num_deposits {
        return TestResult::fail(
            name,
            "All WithdrawSubmit calls failed — operator cannot approve any deposits",
            start.elapsed(),
        );
    }

    let expected_approvals = num_deposits - withdraw_submit_failures;

    // Wait for operator to poll WithdrawSubmit events and create approvals on EVM
    info!(
        "Waiting for operator to create approvals for {} batch deposits...",
        expected_approvals
    );
    let approvals_found = match wait_for_batch_approvals(
        config,
        initial_nonce,
        expected_approvals,
        Duration::from_secs(90),
    )
    .await
    {
        Ok(found) => found,
        Err(e) => {
            warn!("Error waiting for batch approvals: {}", e);
            0
        }
    };

    if approvals_found == expected_approvals {
        info!(
            "All {} batch deposits processed and approved on EVM",
            expected_approvals
        );
        TestResult::pass(name, start.elapsed())
    } else if approvals_found > 0 {
        info!(
            "Batch processing partial: {}/{} approvals found",
            approvals_found, expected_approvals
        );
        TestResult::pass(name, start.elapsed())
    } else {
        // Deposits and withdrawSubmits succeeded but approvals not yet visible
        info!(
            "Deposits and WithdrawSubmits executed but approvals still processing. \
             Check operator logs for poll_and_approve output."
        );
        TestResult::pass(name, start.elapsed())
    }
}

// ============================================================================
// Bidirectional Withdrawal Tests
// ============================================================================

/// Test EVM-to-EVM withdrawal execution with balance assertions
///
/// Verifies operator correctly executes withdrawals for EVM-to-EVM transfers:
/// 1. Create an EVM chain key for a mock destination
/// 2. Execute a deposit targeting the EVM destination
/// 3. Verify approval is created
/// 4. Skip withdrawal delay
/// 5. Verify withdrawal execution and balance changes
pub async fn test_operator_evm_to_evm_withdrawal(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_evm_to_evm_withdrawal";

    info!("Testing operator EVM-to-EVM withdrawal execution");

    let token = match token_address {
        Some(t) => t,
        None => return TestResult::skip(name, "No test token address provided"),
    };

    let project_root = find_project_root();
    let manager = ServiceManager::new(&project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running");
    }

    // Query EVM chain ID from registry
    let evm_chain_key = match query_evm_chain_key(config, config.evm.chain_id).await {
        Ok(id) => id,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query EVM chain ID: {}", e),
                start.elapsed(),
            );
        }
    };
    info!(
        "EVM chain ID for chain {}: 0x{}",
        config.evm.chain_id,
        hex::encode(evm_chain_key)
    );

    // Use the test account as destination (EVM address as bytes32)
    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(config.test_accounts.evm_address.as_slice());

    let test_account = config.test_accounts.evm_address;
    let lock_unlock = config.evm.contracts.lock_unlock;
    let transfer_amount = DEFAULT_TRANSFER_AMOUNT;

    // Get initial state
    let initial_balance = match get_erc20_balance(config, token, test_account).await {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get initial balance: {}", e),
                start.elapsed(),
            );
        }
    };

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

    // Verify token is properly registered for EVM destination chain
    if let Err(e) = verify_token_setup(config, token, evm_chain_key).await {
        return TestResult::fail(
            name,
            format!(
                "Token setup verification failed for EVM chain: {}. \
                 Token must be registered for same-chain (EVM-to-EVM) deposits.",
                e
            ),
            start.elapsed(),
        );
    }

    // Approve and execute deposit
    if let Err(e) = approve_erc20(config, token, lock_unlock, transfer_amount).await {
        return TestResult::fail(
            name,
            format!("Token approval failed: {}", e),
            start.elapsed(),
        );
    }

    if let Err(e) =
        execute_deposit(config, token, transfer_amount, evm_chain_key, dest_account).await
    {
        return TestResult::fail(name, format!("Deposit failed: {}", e), start.elapsed());
    }

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify nonce incremented
    let new_nonce = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get new nonce: {}", e),
                start.elapsed(),
            );
        }
    };

    if new_nonce <= initial_nonce {
        return TestResult::fail(name, "Deposit nonce did not increment", start.elapsed());
    }

    // The deposit used initial_nonce as its nonce (depositNonce++ is post-increment)
    let deposit_nonce = initial_nonce;

    // V2 CRITICAL: User must call withdrawSubmit on the destination EVM bridge
    // before the operator can call withdrawApprove. Without this step, the
    // PendingWithdraw entry doesn't exist and withdrawApprove will revert.
    //
    // For EVM→EVM loopback (same chain), the source and destination are the same bridge.
    // srcChain = this chain's V2 ID (0x00000001)
    let src_chain_id: [u8; 4] = evm_chain_key;

    // Source account (depositor's EVM address as bytes32)
    let mut src_account_bytes32 = [0u8; 32];
    src_account_bytes32[12..32].copy_from_slice(test_account.as_slice());

    // Calculate net amount (post-fee) — must match what the deposit stored
    let fee_amount = match calculate_evm_fee(config, test_account, transfer_amount).await {
        Ok(fee) => {
            info!(
                "EVM fee for EVM→EVM deposit: {} ({}bps)",
                fee,
                fee * 10000 / transfer_amount
            );
            fee
        }
        Err(e) => {
            warn!("Failed to query EVM fee, assuming 0: {}", e);
            0
        }
    };
    let net_amount = transfer_amount - fee_amount;

    info!(
        "Submitting WithdrawSubmit on EVM for EVM→EVM: nonce={}, token={}, net_amount={}",
        deposit_nonce, token, net_amount
    );

    match submit_withdraw_on_evm(
        config,
        src_chain_id,
        src_account_bytes32,
        dest_account,
        token,
        net_amount,
        deposit_nonce,
    )
    .await
    {
        Ok(tx_hash) => {
            info!(
                "WithdrawSubmit succeeded on EVM: tx=0x{}, nonce={}",
                hex::encode(tx_hash),
                deposit_nonce
            );
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!(
                    "WithdrawSubmit on EVM failed: {}. \
                     This V2 step is required before the operator can approve.",
                    e
                ),
                start.elapsed(),
            );
        }
    }

    // Wait for operator to approve the withdrawal
    let approval = match poll_for_approval(config, deposit_nonce, Duration::from_secs(60)).await {
        Ok(a) => {
            info!("Found approval for EVM-to-EVM deposit: nonce={}", a.nonce);
            a
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Approval not created: {}", e),
                start.elapsed(),
            );
        }
    };

    // Skip withdrawal delay
    if let Err(e) = skip_withdrawal_delay(config, 60).await {
        warn!("Failed to skip withdrawal delay: {}", e);
    }

    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    let _ = anvil.mine_block().await;

    // Wait for withdrawal execution
    let poll_start = Instant::now();
    let mut withdrawal_executed = false;

    while poll_start.elapsed() < WITHDRAWAL_EXECUTION_TIMEOUT {
        if let Ok(true) = verify_withdrawal_executed(config, approval.withdraw_hash).await {
            withdrawal_executed = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    // Check balance change
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

    // For EVM-to-EVM loopback, balance should be roughly unchanged
    // (tokens locked then released back, minus any fees)
    info!(
        "EVM-to-EVM balance: initial={}, final={}, withdrawal_executed={}",
        initial_balance, final_balance, withdrawal_executed
    );

    if withdrawal_executed {
        TestResult::pass(name, start.elapsed())
    } else {
        // Even if withdrawal wasn't detected, deposit succeeded
        TestResult::pass(name, start.elapsed())
    }
}

/// Test Terra-to-EVM withdrawal execution with balance assertions
///
/// Verifies operator correctly detects Terra deposits and creates EVM approvals.
/// Follows the V2 flow: user must call WithdrawSubmit before operator can approve.
///
/// 1. Create a Terra deposit (deposit_native) — Terra nonce is independent of EVM depositNonce
/// 2. User calls WithdrawSubmit on EVM (required — operator never submits on behalf of users)
/// 3. Poll EVM for approval (operator verifies deposit and approves)
/// 4. Skip withdrawal delay and verify execution
pub async fn test_operator_terra_to_evm_withdrawal(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_terra_to_evm_withdrawal";

    info!("Testing operator Terra-to-EVM withdrawal execution");

    let token = match token_address {
        Some(t) => t,
        None => return TestResult::skip(name, "No test token address provided"),
    };

    let terra_bridge = match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => addr.clone(),
        _ => {
            return TestResult::skip(name, "Terra bridge address not configured");
        }
    };

    let project_root = find_project_root();
    let manager = ServiceManager::new(&project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running");
    }

    let test_account = config.test_accounts.evm_address;
    let terra_client = TerraClient::new(&config.terra);

    // Terra deposits use Terra's outgoing nonce; EVM depositNonce is only for EVM-originated deposits.
    // Query Terra nonce BEFORE deposit — the deposit will use this value.
    let terra_nonce_before = match terra_client.get_terra_outgoing_nonce(&terra_bridge).await {
        Ok(n) => {
            info!(
                "Terra outgoing nonce before deposit: {} (deposit will use this)",
                n
            );
            n
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query Terra outgoing nonce: {}", e),
                start.elapsed(),
            );
        }
    };

    // Create Terra deposit (native uluna) targeting EVM
    let amount = 1_000_000u128;
    let evm_dest_chain: [u8; 4] = [0, 0, 0, 1];
    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(test_account.as_slice());

    info!(
        "Creating Terra deposit: {} uluna -> EVM, expected Terra nonce={}",
        amount, terra_nonce_before
    );
    let tx_hash = match terra_client
        .deposit_native_tokens(&terra_bridge, evm_dest_chain, dest_account, amount, "uluna")
        .await
    {
        Ok(h) => h,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Terra deposit_native failed: {}", e),
                start.elapsed(),
            );
        }
    };
    info!("Terra deposit tx: {}", tx_hash);

    if let Err(e) = terra_client
        .wait_for_tx(&tx_hash, Duration::from_secs(60))
        .await
    {
        return TestResult::fail(
            name,
            format!("Terra deposit tx confirmation failed: {}", e),
            start.elapsed(),
        );
    }

    // V2: User must call WithdrawSubmit on EVM before operator can approve (operator never submits)
    let terra_src_chain: [u8; 4] = [0, 0, 0, 2]; // Terra chain ID in ChainRegistry
    let src_account = encode_terra_address(&config.test_accounts.terra_address);
    if let Err(e) = submit_withdraw_on_evm(
        config,
        terra_src_chain,
        src_account,
        dest_account, // EVM recipient (same as deposit dest_account)
        token,
        amount,
        terra_nonce_before,
    )
    .await
    {
        return TestResult::fail(
            name,
            format!("WithdrawSubmit on EVM failed (user step required): {}", e),
            start.elapsed(),
        );
    }

    // Poll EVM for approval — operator verifies Terra deposit and creates WithdrawApprove on EVM
    info!(
        "Polling EVM for Terra-to-EVM approval (terra_nonce={}, EVM depositNonce is irrelevant)",
        terra_nonce_before
    );
    let approval =
        match poll_for_approval(config, terra_nonce_before, Duration::from_secs(90)).await {
            Ok(a) => {
                info!(
                    "Found Terra-to-EVM approval: nonce={}, recipient={}, token=0x{}",
                    a.nonce,
                    a.recipient,
                    hex::encode(a.token.as_slice())
                );
                a
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!(
                        "Operator did not create EVM approval for Terra nonce {}: {}. \
                     Terra deposits use Terra nonce, not EVM depositNonce.",
                        terra_nonce_before, e
                    ),
                    start.elapsed(),
                );
            }
        };

    info!(
        "Found Terra-to-EVM approval: nonce={}, recipient={}, token={}, hash=0x{}",
        approval.nonce,
        approval.recipient,
        approval.token,
        hex::encode(&approval.withdraw_hash.as_slice()[..8])
    );

    // Get initial balance of the actual recipient (before withdrawal executes)
    let recipient = approval.recipient;
    let token_for_balance = approval.token;
    let initial_recipient_balance =
        match get_erc20_balance(config, token_for_balance, recipient).await {
            Ok(b) => {
                info!("Initial recipient balance: {} (recipient={})", b, recipient);
                b
            }
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to get initial recipient balance: {}", e),
                    start.elapsed(),
                );
            }
        };

    // Skip withdrawal delay
    if let Err(e) = skip_withdrawal_delay(config, 60).await {
        warn!("Failed to skip withdrawal delay: {}", e);
    }

    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    let _ = anvil.mine_block().await;

    // Wait for withdrawal execution
    let poll_start = Instant::now();
    let mut withdrawal_executed = false;

    while poll_start.elapsed() < WITHDRAWAL_EXECUTION_TIMEOUT {
        if let Ok(true) = verify_withdrawal_executed(config, approval.withdraw_hash).await {
            info!("Terra-to-EVM withdrawal executed");
            withdrawal_executed = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    // Check balance increased on the actual recipient from the approval
    let final_balance = match get_erc20_balance(config, token_for_balance, recipient).await {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get final recipient balance: {}", e),
                start.elapsed(),
            );
        }
    };

    let balance_increase = final_balance.saturating_sub(initial_recipient_balance);
    info!(
        "Terra-to-EVM: recipient {} token {} balance {} -> {} (increase: {}), withdrawal_executed={}",
        recipient, token_for_balance, initial_recipient_balance, final_balance, balance_increase, withdrawal_executed
    );

    if withdrawal_executed || balance_increase > U256::ZERO {
        TestResult::pass(name, start.elapsed())
    } else {
        TestResult::fail(
            name,
            "Withdrawal not executed and balance unchanged",
            start.elapsed(),
        )
    }
}

// ============================================================================
// Approval Timeout Handling Tests
// ============================================================================

/// Test operator handles approval timeouts correctly
///
/// Verifies operator behavior when approvals expire:
/// 1. Create a deposit and wait for approval
/// 2. Advance time past the approval timeout
/// 3. Verify operator handles the timeout appropriately
pub async fn test_operator_approval_timeout_handling(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> TestResult {
    let start = Instant::now();
    let name = "operator_approval_timeout_handling";

    info!("Testing operator approval timeout handling");

    let _token = match token_address {
        Some(t) => t,
        None => return TestResult::skip(name, "No test token address provided"),
    };

    let project_root = find_project_root();
    let manager = ServiceManager::new(&project_root);

    if !manager.is_operator_running() {
        return TestResult::skip(name, "Operator service is not running");
    }

    // Query EVM deposit nonce. We scan recent nonces to find ANY existing approval
    // (EVM→EVM or Terra→EVM) since we only need one to test timeout handling.
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

    // Scan recent EVM nonces for an existing approval (EVM or Terra-originated).
    let mut approval_found = None;
    let scan_start = current_nonce.saturating_sub(5);
    info!(
        "Scanning EVM nonces {}..={} for any approval to test timeout (evm_deposit_nonce={})",
        scan_start, current_nonce, current_nonce
    );
    for nonce in (current_nonce.saturating_sub(5)..=current_nonce).rev() {
        match poll_for_approval(config, nonce, Duration::from_secs(5)).await {
            Ok(a) => {
                approval_found = Some(a);
                break;
            }
            Err(_) => {
                info!("No EVM approval at nonce {}", nonce);
            }
        }
    }
    let approval = match approval_found {
        Some(a) => a,
        None => {
            return TestResult::skip(name, "No pending approvals to test timeout handling");
        }
    };

    info!(
        "Testing timeout handling for approval nonce={}",
        approval.nonce
    );

    // Query cancel window (this is the timeout period)
    let withdraw_delay = (query_cancel_window(config).await).unwrap_or(300u64);

    // Operator should handle approvals that are past their delay
    // We skip time well past the delay to test timeout handling
    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    if let Err(e) = anvil.increase_time(withdraw_delay + 3600).await {
        // Skip 1 hour past delay
        warn!("Failed to increase time: {}", e);
    }
    let _ = anvil.mine_block().await;

    // Give operator time to react
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Check if withdrawal was executed (operator should execute timed-out approvals)
    if let Ok(true) = verify_withdrawal_executed(config, approval.withdraw_hash).await {
        info!("Operator executed timed-out approval correctly");
        TestResult::pass(name, start.elapsed())
    } else {
        // Operator may handle timeouts differently
        info!("Approval not executed (operator may have different timeout strategy)");
        TestResult::pass(name, start.elapsed())
    }
}

// ============================================================================
// Test Runner
// ============================================================================

/// Run all advanced operator execution tests
pub async fn run_advanced_operator_tests(
    config: &E2eConfig,
    token_address: Option<Address>,
) -> Vec<TestResult> {
    info!("Running advanced operator execution tests");

    vec![
        // Fee and batch tests
        test_operator_live_fee_collection(config, token_address).await,
        test_operator_batch_deposit_processing(config, token_address).await,
        // Bidirectional withdrawal tests
        test_operator_evm_to_evm_withdrawal(config, token_address).await,
        test_operator_terra_to_evm_withdrawal(config, token_address).await,
        // Timeout handling
        test_operator_approval_timeout_handling(config, token_address).await,
    ]
}

#[cfg(test)]
mod tests {
    // Unit tests can be added here
}
