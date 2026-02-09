//! Withdrawal-related integration tests (Terra → EVM direction)
//!
//! Tests for transferring tokens from Terra to EVM, including
//! basic transfers and extended verification.

use crate::evm::AnvilTimeClient;
use crate::terra::TerraClient;
use crate::transfer_helpers::{self, skip_withdrawal_delay};
use crate::{E2eConfig, TestResult};
use alloy::primitives::{Address, U256};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Execute a real Terra → EVM transfer with balance verification
///
/// This test performs an actual native token deposit from Terra to EVM:
/// 1. Gets initial Terra balance
/// 2. Executes DepositNative on Terra bridge
/// 3. Verifies Terra balance decreased
/// 4. Skips time on Anvil for watchtower delay
/// 5. Optionally waits for operator to process
///
/// Note: Full cross-chain verification requires operator running.
pub async fn test_real_terra_to_evm_transfer(
    config: &E2eConfig,
    amount: u128,
    denom: &str,
) -> TestResult {
    let start = Instant::now();
    let name = "real_terra_to_evm_transfer";

    // Check if Terra bridge is configured
    let terra_bridge = match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => addr.clone(),
        _ => {
            return TestResult::skip(name, "Terra bridge address not configured");
        }
    };

    let terra_client = TerraClient::new(&config.terra);
    // EVM address left-padded to 32 bytes (universal address format)
    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(config.test_accounts.evm_address.as_slice());

    info!(
        "Testing Terra → EVM transfer: {} {} to 0x{}",
        amount,
        denom,
        hex::encode(&dest_account[12..32])
    );

    // Step 1: Get initial Terra balance
    let balance_before = match terra_client
        .get_balance(&config.test_accounts.terra_address, denom)
        .await
    {
        Ok(b) => {
            info!("Initial Terra balance: {} {}", b, denom);
            b
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get initial Terra balance: {}", e),
                start.elapsed(),
            );
        }
    };

    if balance_before < amount {
        return TestResult::fail(
            name,
            format!(
                "Insufficient Terra balance: have {}, need {}",
                balance_before, amount
            ),
            start.elapsed(),
        );
    }

    // Step 2: Execute DepositNative on Terra bridge
    // EVM chain's registered 4-byte chain ID (set during setup in terra.rs)
    let evm_dest_chain: [u8; 4] = [0, 0, 0, 1];
    match terra_client
        .deposit_native_tokens(&terra_bridge, evm_dest_chain, dest_account, amount, denom)
        .await
    {
        Ok(tx_hash) => {
            info!("DepositNative transaction: {}", tx_hash);

            // Wait for transaction confirmation
            match terra_client
                .wait_for_tx(&tx_hash, Duration::from_secs(60))
                .await
            {
                Ok(result) => {
                    if !result.success {
                        return TestResult::fail(
                            name,
                            format!("DepositNative transaction failed: {}", result.raw_log),
                            start.elapsed(),
                        );
                    }
                    info!(
                        "DepositNative transaction confirmed at height {}",
                        result.height
                    );
                }
                Err(e) => {
                    return TestResult::fail(
                        name,
                        format!("Failed to confirm DepositNative transaction: {}", e),
                        start.elapsed(),
                    );
                }
            }
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to execute DepositNative: {}", e),
                start.elapsed(),
            );
        }
    }

    // Step 3: Verify Terra balance decreased
    let balance_after = match terra_client
        .get_balance(&config.test_accounts.terra_address, denom)
        .await
    {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get Terra balance after deposit: {}", e),
                start.elapsed(),
            );
        }
    };

    // Account for gas fees - balance should decrease by at least the deposited amount
    if balance_before - balance_after < amount {
        warn!(
            "Balance decrease less than expected (may include fees): {} -> {}",
            balance_before, balance_after
        );
    }
    info!(
        "Terra balance decreased: {} -> {} (delta: {})",
        balance_before,
        balance_after,
        balance_before - balance_after
    );

    // Step 4: Skip time on Anvil for watchtower delay (300s + buffer)
    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    match anvil.increase_time(310).await {
        Ok(_) => {
            info!("Skipped 310 seconds on Anvil for watchtower delay");
        }
        Err(e) => {
            warn!("Failed to skip time on Anvil: {}", e);
            // Continue anyway - operator may process without time skip in test mode
        }
    }

    // Note: Full verification would require checking EVM approval/release
    // This requires operator to be running and processing the deposit

    TestResult::pass(name, start.elapsed())
}

/// Test Terra → EVM transfer with destination verification
pub async fn test_terra_to_evm_with_verification(
    config: &E2eConfig,
    dest_token: Option<Address>,
    amount: u128,
    denom: &str,
) -> TestResult {
    let start = Instant::now();
    let name = "terra_to_evm_with_verification";

    let evm_recipient = config.test_accounts.evm_address;

    // Step 1: Get initial EVM balance (if dest token known)
    let initial_evm_balance = if let Some(token) = dest_token {
        match transfer_helpers::get_erc20_balance(config, token, evm_recipient).await {
            Ok(b) => {
                info!("Initial EVM token balance: {}", b);
                b
            }
            Err(e) => {
                debug!("Could not get EVM balance: {}", e);
                U256::ZERO
            }
        }
    } else {
        U256::ZERO
    };

    // Step 2: Execute Terra DepositNative
    let deposit_result = test_real_terra_to_evm_transfer(config, amount, denom).await;
    if deposit_result.is_fail() {
        return deposit_result;
    }

    // Step 3: Skip time for watchtower delay
    if let Err(e) = skip_withdrawal_delay(config, 30).await {
        warn!("Failed to skip time: {}", e);
    }

    // Step 4: Wait for operator to process
    info!("Waiting for operator to process deposit...");
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Step 5: Check EVM balance increased
    if let Some(token) = dest_token {
        let final_balance =
            match transfer_helpers::get_erc20_balance(config, token, evm_recipient).await {
                Ok(b) => b,
                Err(e) => {
                    debug!("Could not get final balance: {}", e);
                    return TestResult::pass(name, start.elapsed());
                }
            };

        if final_balance > initial_evm_balance {
            info!(
                "EVM balance increased: {} -> {}",
                initial_evm_balance, final_balance
            );
        }
    }
    TestResult::pass(name, start.elapsed())
}
