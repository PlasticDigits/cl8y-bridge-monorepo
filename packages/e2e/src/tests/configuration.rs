//! Configuration tests for E2E test suite
//!
//! Tests that verify contracts and accounts are properly configured.

use crate::{E2eConfig, TestResult};
use alloy::primitives::Address;
use std::time::Instant;

use super::helpers::{
    query_contract_code, query_deposit_nonce, query_evm_chain_key, query_has_role,
};

/// Test that EVM contracts are deployed
///
/// Verifies that the contract addresses from the configuration are not zero addresses.
/// Returns a `TestResult::Pass` if all contracts are deployed, `TestResult::Fail` otherwise.
pub async fn test_evm_contracts_deployed(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_contracts_deployed";

    let contracts = &config.evm.contracts;

    if contracts.bridge == Address::ZERO {
        return TestResult::fail(name, "Bridge address is zero", start.elapsed());
    }
    if contracts.router == Address::ZERO {
        return TestResult::fail(name, "Router address is zero", start.elapsed());
    }
    if contracts.access_manager == Address::ZERO {
        return TestResult::fail(name, "AccessManager address is zero", start.elapsed());
    }
    if contracts.chain_registry == Address::ZERO {
        return TestResult::fail(name, "ChainRegistry address is zero", start.elapsed());
    }
    if contracts.token_registry == Address::ZERO {
        return TestResult::fail(name, "TokenRegistry address is zero", start.elapsed());
    }
    if contracts.mint_burn == Address::ZERO {
        return TestResult::fail(name, "MintBurn address is zero", start.elapsed());
    }
    if contracts.lock_unlock == Address::ZERO {
        return TestResult::fail(name, "LockUnlock address is zero", start.elapsed());
    }

    tracing::info!(
        "All EVM contracts deployed: bridge={}, router={}",
        contracts.bridge,
        contracts.router
    );

    TestResult::pass(name, start.elapsed())
}

/// Test Terra bridge address configuration
///
/// Verifies that the Terra bridge address is configured and valid.
/// Returns a `TestResult::Pass` if configured, `TestResult::Skip` if not needed.
pub async fn test_terra_bridge_configured(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "terra_bridge_configured";

    match &config.terra.bridge_address {
        Some(address) if !address.is_empty() => {
            tracing::info!("Terra bridge address configured: {}", address);
            TestResult::pass(name, start.elapsed())
        }
        Some(_) => TestResult::skip(name, "Bridge address is empty string".to_string()),
        None => TestResult::skip(name, "Terra bridge address not configured".to_string()),
    }
}

/// Test account configuration
///
/// Verifies that test accounts are properly configured.
/// Returns a `TestResult::Pass` if configured, `TestResult::Fail` otherwise.
pub async fn test_accounts_configured(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "accounts_configured";

    if config.test_accounts.evm_address == Address::ZERO {
        return TestResult::fail(name, "EVM test address is zero", start.elapsed());
    }

    if config.test_accounts.evm_private_key == alloy::primitives::B256::ZERO {
        return TestResult::fail(name, "EVM test private key is zero", start.elapsed());
    }

    if config.test_accounts.terra_address.is_empty() {
        return TestResult::fail(name, "Terra test address is empty", start.elapsed());
    }

    if config.test_accounts.terra_key_name.is_empty() {
        return TestResult::fail(name, "Terra key name is empty", start.elapsed());
    }

    tracing::info!(
        "Test accounts configured: evm={}, terra={}",
        config.test_accounts.evm_address,
        config.test_accounts.terra_address
    );

    TestResult::pass(name, start.elapsed())
}

/// Test deposit nonce management
///
/// Verifies that deposit nonces can be read from the bridge contract.
/// Returns a `TestResult` indicating success or failure.
pub async fn test_deposit_nonce(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "deposit_nonce";

    // Query the deposit nonce from the bridge contract
    match query_deposit_nonce(config).await {
        Ok(nonce) => {
            tracing::info!("Current deposit nonce: {}", nonce);
            TestResult::pass(name, start.elapsed())
        }
        Err(e) => TestResult::fail(
            name,
            format!("Failed to query deposit nonce: {}", e),
            start.elapsed(),
        ),
    }
}

/// Test token registry functionality
///
/// Verifies that the token registry contract is accessible and operational.
/// Returns a `TestResult` indicating success or failure.
pub async fn test_token_registry(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "token_registry";

    // Verify the TokenRegistry contract is deployed (not zero address)
    if config.evm.contracts.token_registry == Address::ZERO {
        return TestResult::fail(name, "TokenRegistry address is zero", start.elapsed());
    }

    // Query the contract to verify it's accessible
    match query_contract_code(config, config.evm.contracts.token_registry).await {
        Ok(has_code) => {
            if has_code {
                tracing::info!(
                    "TokenRegistry contract at {} has code",
                    config.evm.contracts.token_registry
                );
                TestResult::pass(name, start.elapsed())
            } else {
                TestResult::fail(name, "TokenRegistry has no code deployed", start.elapsed())
            }
        }
        Err(e) => TestResult::fail(
            name,
            format!("Failed to query TokenRegistry: {}", e),
            start.elapsed(),
        ),
    }
}

/// Test chain registry functionality
///
/// Verifies that the chain registry contract is accessible and can query chain keys.
/// Returns a `TestResult` indicating success or failure.
///
/// SECURITY HARDENED: Chain key query failures now cause test failure.
pub async fn test_chain_registry(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "chain_registry";

    // Verify the ChainRegistry contract is deployed (not zero address)
    if config.evm.contracts.chain_registry == Address::ZERO {
        return TestResult::fail(name, "ChainRegistry address is zero", start.elapsed());
    }

    // Query the contract to verify it's accessible
    match query_contract_code(config, config.evm.contracts.chain_registry).await {
        Ok(has_code) => {
            if has_code {
                tracing::info!(
                    "ChainRegistry contract at {} has code",
                    config.evm.contracts.chain_registry
                );

                // Try to query the EVM chain key for local chain (chain id 31337 for Anvil)
                match query_evm_chain_key(config, 31337).await {
                    Ok(chain_key) => {
                        tracing::info!("Local EVM chain key: 0x{}", hex::encode(chain_key));
                        TestResult::pass(name, start.elapsed())
                    }
                    Err(e) => {
                        // SECURITY HARDENED: Convert WARN to FAIL
                        return TestResult::fail(
                            name,
                            format!("Chain key query failed (security-critical): {}", e),
                            start.elapsed(),
                        );
                    }
                }
            } else {
                TestResult::fail(name, "ChainRegistry has no code deployed", start.elapsed())
            }
        }
        Err(e) => TestResult::fail(
            name,
            format!("Failed to query ChainRegistry: {}", e),
            start.elapsed(),
        ),
    }
}

/// Test access manager permissions
///
/// Verifies that the access manager contract is accessible and can query roles.
/// Returns a `TestResult` indicating success or failure.
///
/// SECURITY HARDENED: Role query failures now cause test failure.
pub async fn test_access_manager(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "access_manager";

    // Verify the AccessManager contract is deployed (not zero address)
    if config.evm.contracts.access_manager == Address::ZERO {
        return TestResult::fail(name, "AccessManager address is zero", start.elapsed());
    }

    // Query the contract to verify it's accessible
    match query_contract_code(config, config.evm.contracts.access_manager).await {
        Ok(has_code) => {
            if has_code {
                tracing::info!(
                    "AccessManager contract at {} has code",
                    config.evm.contracts.access_manager
                );

                // Try to query if test account has OPERATOR_ROLE (role id 1)
                let test_address = config.test_accounts.evm_address;
                match query_has_role(config, 1, test_address).await {
                    Ok(has_role) => {
                        if has_role {
                            tracing::info!("Test account {} has OPERATOR_ROLE", test_address);
                        } else {
                            tracing::info!(
                                "Test account {} does not have OPERATOR_ROLE",
                                test_address
                            );
                        }
                        TestResult::pass(name, start.elapsed())
                    }
                    Err(e) => {
                        // SECURITY HARDENED: Convert WARN to FAIL
                        return TestResult::fail(
                            name,
                            format!("Role query failed (security-critical): {}", e),
                            start.elapsed(),
                        );
                    }
                }
            } else {
                TestResult::fail(name, "AccessManager has no code deployed", start.elapsed())
            }
        }
        Err(e) => TestResult::fail(
            name,
            format!("Failed to query AccessManager: {}", e),
            start.elapsed(),
        ),
    }
}
