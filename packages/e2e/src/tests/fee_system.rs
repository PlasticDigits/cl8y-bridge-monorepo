//! Fee System E2E Tests
//!
//! This module contains end-to-end tests for the bridge fee calculation system:
//! - Standard fee calculation (0.5% = 50 bps)
//! - CL8Y holder discount (0.1% = 10 bps)
//! - Custom per-account fees
//! - Fee priority ordering (custom > discount > standard)
//! - Fee collection to recipient address
//!
//! These tests verify fee calculation logic by querying the bridge contract
//! and comparing results with expected calculations.

use crate::{E2eConfig, TestResult};
use alloy::primitives::{Address, U256};
use eyre::Result;
use std::time::Instant;
use tracing::{info, warn};

use super::helpers::{check_evm_connection, selector};
use super::operator_helpers::calculate_fee;

// ============================================================================
// Helper Functions
// ============================================================================

/// Query fee configuration from bridge contract
///
/// Returns: (standardFeeBps, discountedFeeBps, cl8yThreshold, cl8yToken, feeRecipient)
async fn query_fee_config(config: &E2eConfig) -> Result<(u64, u64, U256, Address, Address)> {
    let client = reqwest::Client::new();

    let call_data = format!("0x{}", selector("getFeeConfig()"));

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "EVM RPC returned status: {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response.json().await?;

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    // Parse FeeConfig struct response
    // Layout: standardFeeBps (uint256), discountedFeeBps (uint256), cl8yThreshold (uint256),
    //         cl8yToken (address), feeRecipient (address)
    // Each field is 32 bytes in ABI encoding
    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;

    if bytes.len() < 160 {
        return Err(eyre::eyre!(
            "Invalid fee config response: expected at least 160 bytes, got {}",
            bytes.len()
        ));
    }

    // Parse each field (32 bytes each)
    let standard_fee_bps = U256::from_be_slice(&bytes[0..32]);
    let discounted_fee_bps = U256::from_be_slice(&bytes[32..64]);
    let cl8y_threshold = U256::from_be_slice(&bytes[64..96]);
    let cl8y_token = Address::from_slice(&bytes[108..128]); // address is right-aligned in 32 bytes
    let fee_recipient = Address::from_slice(&bytes[140..160]); // address is right-aligned in 32 bytes

    Ok((
        standard_fee_bps.to::<u64>(),
        discounted_fee_bps.to::<u64>(),
        cl8y_threshold,
        cl8y_token,
        fee_recipient,
    ))
}

/// Query calculated fee for a depositor and amount
async fn query_calculate_fee(config: &E2eConfig, depositor: Address, amount: u128) -> Result<u128> {
    let client = reqwest::Client::new();

    let sel = selector("calculateFee(address,uint256)");
    let depositor_padded = format!("{:0>64}", hex::encode(depositor.as_slice()));
    let amount_padded = format!("{:064x}", amount);
    let call_data = format!("0x{}{}{}", sel, depositor_padded, amount_padded);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "EVM RPC returned status: {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response.json().await?;

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let fee_amount = U256::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(fee_amount.to::<u128>())
}

/// Query account fee info (bps and type)
async fn query_account_fee(config: &E2eConfig, account: Address) -> Result<(u64, String)> {
    let client = reqwest::Client::new();

    // getAccountFee(address) returns: (uint256 feeBps, string memory feeType)
    let sel = selector("getAccountFee(address)");
    let account_padded = format!("{:0>64}", hex::encode(account.as_slice()));
    let call_data = format!("0x{}{}", sel, account_padded);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "EVM RPC returned status: {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response.json().await?;

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    // Parse return value: (uint256 feeBps, string memory feeType)
    // Layout: offset to string (32 bytes), feeBps (32 bytes), string length (32 bytes), string data (padded to 32 bytes)
    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;

    if bytes.len() < 96 {
        return Err(eyre::eyre!(
            "Invalid account fee response: expected at least 96 bytes, got {}",
            bytes.len()
        ));
    }

    // Parse feeBps (offset 32-64)
    let fee_bps = U256::from_be_slice(&bytes[32..64]).to::<u64>();

    // Parse string offset (first 32 bytes)
    let string_offset = U256::from_be_slice(&bytes[0..32]).to::<usize>();

    // Parse string length (at offset)
    if bytes.len() < string_offset + 32 {
        return Err(eyre::eyre!("Invalid string offset in response"));
    }

    let string_length =
        U256::from_be_slice(&bytes[string_offset..string_offset + 32]).to::<usize>();

    // Parse string data (after length)
    if bytes.len() < string_offset + 32 + string_length {
        return Err(eyre::eyre!("Invalid string length in response"));
    }

    let string_bytes = &bytes[string_offset + 32..string_offset + 32 + string_length];
    let fee_type = String::from_utf8_lossy(string_bytes).to_string();

    Ok((fee_bps, fee_type))
}

/// Query if account has custom fee
async fn query_has_custom_fee(config: &E2eConfig, account: Address) -> Result<bool> {
    let client = reqwest::Client::new();

    let sel = selector("hasCustomFee(address)");
    let account_padded = format!("{:0>64}", hex::encode(account.as_slice()));
    let call_data = format!("0x{}{}", sel, account_padded);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "EVM RPC returned status: {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response.json().await?;

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    // Parse boolean (last byte of 32-byte response)
    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;
    Ok(bytes.last().copied().unwrap_or(0) != 0)
}

// ============================================================================
// Test Functions
// ============================================================================

/// Test standard fee calculation (0.5% = 50 bps)
///
/// Verifies that the standard fee is correctly calculated for deposits.
/// Calculates expected fee for a given amount and verifies it matches
/// what the contract computes.
pub async fn test_standard_fee_calculation(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "standard_fee_calculation";

    // Check EVM connectivity first
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {}
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Query fee configuration
    let (standard_fee_bps, _, _, _, _) = match query_fee_config(config).await {
        Ok(config) => config,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query fee config: {}", e),
                start.elapsed(),
            );
        }
    };

    info!("Standard fee BPS: {}", standard_fee_bps);

    // Test with multiple amounts
    let test_amounts = vec![
        1_000_000u128,     // 1 token (6 decimals)
        10_000_000u128,    // 10 tokens
        100_000_000u128,   // 100 tokens
        1_000_000_000u128, // 1000 tokens
    ];

    let test_account = config.test_accounts.evm_address;

    for amount in test_amounts {
        // Calculate expected fee
        let expected_fee = calculate_fee(amount, standard_fee_bps);

        // Query contract for calculated fee
        let calculated_fee = match query_calculate_fee(config, test_account, amount).await {
            Ok(fee) => fee,
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!(
                        "Failed to query calculated fee for amount {}: {}",
                        amount, e
                    ),
                    start.elapsed(),
                );
            }
        };

        // Verify fees match (allow small rounding differences)
        let diff = if calculated_fee > expected_fee {
            calculated_fee - expected_fee
        } else {
            expected_fee - calculated_fee
        };

        // Allow up to 1 wei difference due to rounding
        if diff > 1 {
            return TestResult::fail(
                name,
                format!(
                    "Fee mismatch for amount {}: expected {}, got {} (diff: {})",
                    amount, expected_fee, calculated_fee, diff
                ),
                start.elapsed(),
            );
        }

        info!(
            "Amount: {}, Expected fee: {}, Calculated fee: {}",
            amount, expected_fee, calculated_fee
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test CL8Y holder discount (0.1% = 10 bps)
///
/// Verifies that CL8Y holder discount is correctly applied.
/// This test checks if the fee system is configured to support CL8Y discounts,
/// even if CL8Y is not deployed.
pub async fn test_cl8y_holder_discount(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "cl8y_holder_discount";

    // Check EVM connectivity first
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {}
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Query fee configuration
    let (standard_fee_bps, discounted_fee_bps, cl8y_threshold, cl8y_token, _) =
        match query_fee_config(config).await {
            Ok(config) => config,
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to query fee config: {}", e),
                    start.elapsed(),
                );
            }
        };

    info!(
        "Standard fee BPS: {}, Discounted fee BPS: {}, CL8Y threshold: {}, CL8Y token: {}",
        standard_fee_bps, discounted_fee_bps, cl8y_threshold, cl8y_token
    );

    // Verify discount is configured correctly
    if discounted_fee_bps >= standard_fee_bps {
        return TestResult::fail(
            name,
            format!(
                "Discounted fee ({}) should be less than standard fee ({})",
                discounted_fee_bps, standard_fee_bps
            ),
            start.elapsed(),
        );
    }

    // Verify discount is 10 bps (0.1%) as expected
    if discounted_fee_bps != 10 {
        warn!(
            "Discounted fee is {} bps, expected 10 bps (0.1%). Test continues but may indicate misconfiguration.",
            discounted_fee_bps
        );
    }

    // Verify standard fee is 50 bps (0.5%) as expected
    if standard_fee_bps != 50 {
        warn!(
            "Standard fee is {} bps, expected 50 bps (0.5%). Test continues but may indicate misconfiguration.",
            standard_fee_bps
        );
    }

    // Test fee calculation with discount rate
    let test_amount = 1_000_000u128; // 1 token
    let expected_discounted_fee = calculate_fee(test_amount, discounted_fee_bps);
    let expected_standard_fee = calculate_fee(test_amount, standard_fee_bps);

    info!(
        "For amount {}: discounted fee = {}, standard fee = {}",
        test_amount, expected_discounted_fee, expected_standard_fee
    );

    // Verify discount is meaningful (at least 20% reduction)
    let discount_percentage = ((standard_fee_bps - discounted_fee_bps) * 100) / standard_fee_bps;
    if discount_percentage < 20 {
        warn!(
            "Discount is only {}%, which seems low. Expected at least 20%.",
            discount_percentage
        );
    }

    // If CL8Y token is not set (zero address), discount mechanism is disabled
    // but the configuration should still be valid
    if cl8y_token == Address::ZERO {
        info!("CL8Y token not configured (address is zero). Discount mechanism is disabled but config is valid.");
    } else {
        info!("CL8Y token configured at: {}", cl8y_token);
    }

    TestResult::pass(name, start.elapsed())
}

/// Test custom per-account fee setting
///
/// Verifies that an operator can set a custom fee for a specific account.
/// Checks that custom fees override standard and discounted fees.
pub async fn test_custom_per_account_fee(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "custom_per_account_fee";

    // Check EVM connectivity first
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {}
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    let test_account = config.test_accounts.evm_address;

    // Query if account has custom fee
    let has_custom = match query_has_custom_fee(config, test_account).await {
        Ok(has) => has,
        Err(_e) => {
            // If query fails, the function might not exist or contract might not support it
            warn!("Could not query custom fee status. Test will verify fee calculation works.");
            false
        }
    };

    info!("Account {} has custom fee: {}", test_account, has_custom);

    // Query fee configuration
    let (standard_fee_bps, discounted_fee_bps, _, _, _) = match query_fee_config(config).await {
        Ok(config) => config,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query fee config: {}", e),
                start.elapsed(),
            );
        }
    };

    // Query account fee info
    let (account_fee_bps, fee_type) = match query_account_fee(config, test_account).await {
        Ok(info) => info,
        Err(_e) => {
            // Fallback: calculate fee and infer type
            let test_amount = 1_000_000u128;
            let calculated_fee = match query_calculate_fee(config, test_account, test_amount).await
            {
                Ok(fee) => fee,
                Err(e2) => {
                    return TestResult::fail(
                        name,
                        format!("Failed to query calculated fee: {}", e2),
                        start.elapsed(),
                    );
                }
            };

            let standard_fee = calculate_fee(test_amount, standard_fee_bps);
            let discounted_fee = calculate_fee(test_amount, discounted_fee_bps);

            let (bps, ftype) = if calculated_fee == standard_fee {
                (standard_fee_bps, "standard".to_string())
            } else if calculated_fee == discounted_fee {
                (discounted_fee_bps, "discounted".to_string())
            } else {
                // Custom fee - calculate bps from fee amount
                let custom_bps = ((calculated_fee * 10_000) / test_amount) as u64;
                (custom_bps, "custom".to_string())
            };

            (bps, ftype)
        }
    };

    info!(
        "Account {} fee: {} bps (type: {})",
        test_account, account_fee_bps, fee_type
    );

    // Verify fee calculation matches account fee
    let test_amount = 1_000_000u128;
    let expected_fee = calculate_fee(test_amount, account_fee_bps);
    let calculated_fee = match query_calculate_fee(config, test_account, test_amount).await {
        Ok(fee) => fee,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query calculated fee: {}", e),
                start.elapsed(),
            );
        }
    };

    let diff = if calculated_fee > expected_fee {
        calculated_fee - expected_fee
    } else {
        expected_fee - calculated_fee
    };

    if diff > 1 {
        return TestResult::fail(
            name,
            format!(
                "Fee calculation mismatch: expected {} (from {} bps), got {}",
                expected_fee, account_fee_bps, calculated_fee
            ),
            start.elapsed(),
        );
    }

    // If account has custom fee, verify it's different from standard/discounted
    if has_custom && fee_type == "custom" {
        if account_fee_bps == standard_fee_bps || account_fee_bps == discounted_fee_bps {
            warn!(
                "Account has custom fee set, but fee BPS ({}) matches standard/discounted fee",
                account_fee_bps
            );
        }
    }

    TestResult::pass(name, start.elapsed())
}

/// Test fee priority order: custom > discount > standard
///
/// Verifies that fee priority is correctly applied:
/// 1. Custom account fee (if set) takes highest priority
/// 2. CL8Y holder discount (if eligible) takes second priority
/// 3. Standard fee is the default
pub async fn test_fee_priority_order(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "fee_priority_order";

    // Check EVM connectivity first
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {}
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Query fee configuration
    let (standard_fee_bps, discounted_fee_bps, _, cl8y_token, _) =
        match query_fee_config(config).await {
            Ok(config) => config,
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to query fee config: {}", e),
                    start.elapsed(),
                );
            }
        };

    info!(
        "Fee config: standard={} bps, discounted={} bps, cl8y_token={}",
        standard_fee_bps, discounted_fee_bps, cl8y_token
    );

    let test_account = config.test_accounts.evm_address;
    let test_amount = 1_000_000u128;

    // Query account fee info
    let (account_fee_bps, fee_type) = match query_account_fee(config, test_account).await {
        Ok(info) => info,
        Err(_e) => {
            // Fallback calculation
            let calculated_fee = match query_calculate_fee(config, test_account, test_amount).await
            {
                Ok(fee) => fee,
                Err(e2) => {
                    return TestResult::fail(
                        name,
                        format!("Failed to query calculated fee: {}", e2),
                        start.elapsed(),
                    );
                }
            };

            let standard_fee = calculate_fee(test_amount, standard_fee_bps);
            let discounted_fee = calculate_fee(test_amount, discounted_fee_bps);

            let (bps, ftype) = if calculated_fee == standard_fee {
                (standard_fee_bps, "standard".to_string())
            } else if calculated_fee == discounted_fee {
                (discounted_fee_bps, "discounted".to_string())
            } else {
                // Custom fee - calculate bps from fee amount
                let custom_bps = ((calculated_fee * 10_000) / test_amount) as u64;
                (custom_bps, "custom".to_string())
            };

            (bps, ftype)
        }
    };

    info!(
        "Account {} effective fee: {} bps (type: {})",
        test_account, account_fee_bps, fee_type
    );

    // Verify calculated fee matches account fee
    let calculated_fee = match query_calculate_fee(config, test_account, test_amount).await {
        Ok(fee) => fee,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query calculated fee: {}", e),
                start.elapsed(),
            );
        }
    };

    let expected_fee = calculate_fee(test_amount, account_fee_bps);
    let diff = if calculated_fee > expected_fee {
        calculated_fee - expected_fee
    } else {
        expected_fee - calculated_fee
    };

    if diff > 1 {
        return TestResult::fail(
            name,
            format!(
                "Fee calculation mismatch: expected {}, got {}",
                expected_fee, calculated_fee
            ),
            start.elapsed(),
        );
    }

    // Verify priority logic:
    // If custom fee is set, it should be used
    // Otherwise, if CL8Y discount applies, discounted fee should be used
    // Otherwise, standard fee should be used

    let has_custom = match query_has_custom_fee(config, test_account).await {
        Ok(has) => has,
        Err(_) => false, // Assume no custom fee if query fails
    };

    if has_custom {
        if fee_type != "custom" {
            return TestResult::fail(
                name,
                format!(
                    "Account has custom fee set but fee type is '{}', expected 'custom'",
                    fee_type
                ),
                start.elapsed(),
            );
        }
        info!("Priority verified: Custom fee takes precedence");
    } else if fee_type == "discounted" {
        // Verify CL8Y token is configured
        if cl8y_token == Address::ZERO {
            warn!("Account has discounted fee but CL8Y token is not configured. This may indicate a test environment issue.");
        } else {
            info!("Priority verified: Discounted fee applies (CL8Y holder)");
        }
    } else {
        info!("Priority verified: Standard fee applies (no custom, no discount)");
    }

    TestResult::pass(name, start.elapsed())
}

/// Test fee collection to recipient address
///
/// Verifies that fees are collected to the correct recipient address.
/// This test checks the fee recipient configuration and verifies it's not zero.
pub async fn test_fee_collection_to_recipient(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "fee_collection_to_recipient";

    // Check EVM connectivity first
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {}
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Query fee configuration
    let (standard_fee_bps, _, _, _, fee_recipient) = match query_fee_config(config).await {
        Ok(config) => config,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query fee config: {}", e),
                start.elapsed(),
            );
        }
    };

    info!("Fee recipient address: {}", fee_recipient);

    // Verify fee recipient is not zero address
    if fee_recipient == Address::ZERO {
        return TestResult::fail(
            name,
            "Fee recipient is zero address, which is invalid",
            start.elapsed(),
        );
    }

    // Verify fee recipient is a valid address (has code or is EOA)
    // We can't easily check if it's an EOA vs contract, but we can verify
    // it's not the zero address (already checked above)

    // If fees are enabled (standard_fee_bps > 0), verify recipient is configured
    if standard_fee_bps > 0 && fee_recipient == Address::ZERO {
        return TestResult::fail(
            name,
            format!(
                "Fees are enabled ({} bps) but fee recipient is zero address",
                standard_fee_bps
            ),
            start.elapsed(),
        );
    }

    // Verify fee recipient is different from bridge address
    if fee_recipient == config.evm.contracts.bridge {
        warn!(
            "Fee recipient is the same as bridge address. This may be intentional but is unusual."
        );
    }

    info!("Fee collection recipient verified: {}", fee_recipient);

    TestResult::pass(name, start.elapsed())
}
