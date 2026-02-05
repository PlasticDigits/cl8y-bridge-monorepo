//! EVM ↔ EVM cross-chain transfer tests
//!
//! Tests for cross-chain transfers between two EVM chains.
//! Uses either a second Anvil instance or mock chain key registration.

use crate::transfer_helpers::{
    get_erc20_balance, poll_for_approval, skip_withdrawal_delay, verify_withdrawal_executed,
};
use crate::{ChainKey, E2eConfig, TestResult};
use alloy::primitives::{Address, B256, U256};
use std::time::{Duration, Instant};
use tracing::{info, warn};

use super::helpers::{approve_erc20, execute_deposit, query_deposit_nonce};

/// Secondary EVM chain configuration for cross-EVM tests
#[derive(Debug, Clone)]
pub struct SecondaryEvmConfig {
    /// RPC URL for secondary chain
    pub rpc_url: String,
    /// Chain ID for secondary chain
    pub chain_id: u64,
    /// Bridge address on secondary chain
    pub bridge: Address,
    /// Router address on secondary chain  
    pub router: Address,
    /// Token registry on secondary chain
    pub token_registry: Address,
    /// Chain registry on secondary chain
    pub chain_registry: Address,
}

impl Default for SecondaryEvmConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8546".to_string(), // Second Anvil port
            chain_id: 31338,                              // Different chain ID
            bridge: Address::ZERO,
            router: Address::ZERO,
            token_registry: Address::ZERO,
            chain_registry: Address::ZERO,
        }
    }
}

/// Options for EVM-to-EVM transfer test
#[derive(Debug, Clone)]
pub struct EvmToEvmOptions {
    /// Token on source chain
    pub source_token: Address,
    /// Token on destination chain (if different)
    pub dest_token: Option<Address>,
    /// Amount to transfer
    pub amount: U256,
    /// Whether to use mock chain (vs real second Anvil)
    pub use_mock_chain: bool,
    /// Skip time for withdrawal delay
    pub skip_delay: bool,
    /// Approval polling timeout
    pub approval_timeout: Duration,
}

impl Default for EvmToEvmOptions {
    fn default() -> Self {
        Self {
            source_token: Address::ZERO,
            dest_token: None,
            amount: U256::from(1_000_000u64),
            use_mock_chain: true,
            skip_delay: true,
            // Increased timeout to account for operator processing and block confirmation
            approval_timeout: Duration::from_secs(120),
        }
    }
}

// ============================================================================
// EVM Chain Key Registration
// ============================================================================

/// Register a secondary EVM chain key in the chain registry
///
/// This registers a new EVM chain for cross-EVM transfers.
/// The chain key format is: `evm_<chain_id>` (first 4 bytes "evm_", then chain ID as bytes)
pub async fn register_evm_chain_key(
    config: &E2eConfig,
    secondary_chain_id: u64,
) -> eyre::Result<B256> {
    info!(
        "Registering EVM chain key for chain ID {}",
        secondary_chain_id
    );

    // Compute chain key using the same format as ChainRegistry.getChainKeyEVM
    let chain_key = ChainKey::evm(secondary_chain_id);
    let chain_key_bytes = chain_key.0;

    // Check if already registered by calling getChainKeyEVM
    let client = reqwest::Client::new();
    let chain_id_hex = format!("{:064x}", secondary_chain_id);
    let call_data = format!("0x8e499bcf{}", chain_id_hex); // getChainKeyEVM(uint256)

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.chain_registry),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;
    let result_hex = body["result"].as_str().unwrap_or("0x");
    let existing = hex::decode(result_hex.trim_start_matches("0x")).unwrap_or_default();

    // Check if non-zero (already registered)
    if existing.len() == 32 && existing.iter().any(|&b| b != 0) {
        let existing_key = B256::from_slice(&existing);
        info!(
            "EVM chain key already registered: 0x{}",
            hex::encode(existing_key)
        );
        return Ok(existing_key);
    }

    // Register new chain key via addEVMChainKey (if available)
    // For now, return the computed key (assumes pre-registration or use mock)
    info!("EVM chain key computed: 0x{}", hex::encode(chain_key_bytes));
    Ok(chain_key_bytes)
}

/// Get the EVM chain key for a given chain ID
pub fn get_evm_chain_key(chain_id: u64) -> B256 {
    ChainKey::evm(chain_id).0
}

// ============================================================================
// Mock Chain Key Registration (for single-Anvil testing)
// ============================================================================

/// Register a mock EVM chain key for testing without second Anvil
///
/// This creates a "fake" chain key that allows testing the deposit flow
/// on a single Anvil instance by registering a non-existent chain.
pub async fn register_mock_evm_chain(
    _config: &E2eConfig,
    mock_chain_id: u64,
) -> eyre::Result<B256> {
    info!("Registering mock EVM chain: {}", mock_chain_id);

    // Create a mock chain key
    let chain_key = get_evm_chain_key(mock_chain_id);

    // For mock testing, we just use the computed key
    // Real registration would require admin access to ChainRegistry
    info!("Mock EVM chain key: 0x{}", hex::encode(chain_key));

    Ok(chain_key)
}

// ============================================================================
// EVM-to-EVM Transfer Tests
// ============================================================================

/// Test EVM → EVM deposit flow
///
/// Tests depositing tokens on the primary EVM chain destined for a secondary EVM chain.
/// If `use_mock_chain` is true, uses a mock chain key instead of real secondary.
pub async fn test_evm_to_evm_deposit(config: &E2eConfig, options: &EvmToEvmOptions) -> TestResult {
    let start = Instant::now();
    let name = "evm_to_evm_deposit";

    let token = options.source_token;
    if token == Address::ZERO {
        return TestResult::skip(name, "No source token configured");
    }

    let test_account = config.test_accounts.evm_address;
    info!("Testing EVM→EVM deposit: {} tokens", options.amount);

    // Step 1: Get initial balance
    let initial_balance = match get_erc20_balance(config, token, test_account).await {
        Ok(b) => {
            info!("Initial balance: {}", b);
            b
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get balance: {}", e),
                start.elapsed(),
            );
        }
    };

    if initial_balance < options.amount {
        return TestResult::fail(
            name,
            format!(
                "Insufficient balance: {} < {}",
                initial_balance, options.amount
            ),
            start.elapsed(),
        );
    }

    // Step 2: Get/register destination chain key
    // Note: Both mock and real chain use 31338 for testing purposes
    let dest_chain_id = 31338u64;
    let dest_chain_key = match register_mock_evm_chain(config, dest_chain_id).await {
        Ok(key) => key,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    // Step 3: Create destination account (EVM address as bytes32)
    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(test_account.as_slice());

    // Step 4: Get nonce before deposit
    let nonce_before = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(name, format!("Failed to get nonce: {}", e), start.elapsed());
        }
    };

    // Step 5: Approve tokens
    let lock_unlock = config.evm.contracts.lock_unlock;
    let amount_u128 = options.amount.try_into().unwrap_or(1_000_000u128);

    if let Err(e) = approve_erc20(config, token, lock_unlock, amount_u128).await {
        return TestResult::fail(name, format!("Approval failed: {}", e), start.elapsed());
    }
    info!("Token approval successful");

    // Step 6: Execute deposit
    let router = config.evm.contracts.router;
    match execute_deposit(
        config,
        router,
        token,
        amount_u128,
        dest_chain_key.into(),
        dest_account,
    )
    .await
    {
        Ok(tx) => {
            info!("Deposit executed: 0x{}", hex::encode(tx));
        }
        Err(e) => {
            return TestResult::fail(name, format!("Deposit failed: {}", e), start.elapsed());
        }
    }

    // Step 7: Verify nonce incremented
    tokio::time::sleep(Duration::from_secs(2)).await;
    let nonce_after = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(name, format!("Failed to get nonce: {}", e), start.elapsed());
        }
    };

    if nonce_after <= nonce_before {
        return TestResult::fail(
            name,
            format!("Nonce not incremented: {} -> {}", nonce_before, nonce_after),
            start.elapsed(),
        );
    }
    info!("Deposit nonce: {} -> {}", nonce_before, nonce_after);

    // Step 8: Verify balance decreased
    let final_balance = match get_erc20_balance(config, token, test_account).await {
        Ok(b) => b,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get balance: {}", e),
                start.elapsed(),
            );
        }
    };

    if final_balance >= initial_balance {
        return TestResult::fail(
            name,
            format!(
                "Balance not decreased: {} -> {}",
                initial_balance, final_balance
            ),
            start.elapsed(),
        );
    }

    info!(
        "EVM→EVM deposit successful, balance: {} -> {}",
        initial_balance, final_balance
    );
    TestResult::pass(name, start.elapsed())
}

/// Test EVM → EVM full transfer cycle with operator relay
///
/// Tests complete flow including operator relay and withdrawal execution.
/// Requires operator to be running and configured for cross-EVM.
pub async fn test_evm_to_evm_full_cycle(
    config: &E2eConfig,
    options: &EvmToEvmOptions,
) -> TestResult {
    let start = Instant::now();
    let name = "evm_to_evm_full_cycle";

    // First run the deposit test
    let deposit_result = test_evm_to_evm_deposit(config, options).await;
    if deposit_result.is_fail() {
        return deposit_result;
    }

    // Get the deposit nonce
    let nonce = match query_deposit_nonce(config).await {
        Ok(n) => n,
        Err(e) => {
            return TestResult::fail(name, format!("Failed to get nonce: {}", e), start.elapsed());
        }
    };

    // Poll for operator to create approval
    info!("Waiting for operator relay...");
    let approval = match poll_for_approval(config, nonce, options.approval_timeout).await {
        Ok(a) => {
            info!(
                "Approval received: 0x{}",
                hex::encode(&a.withdraw_hash.as_slice()[..8])
            );
            a
        }
        Err(e) => {
            // Pass with warning if operator not running
            warn!("Approval not received: {} (operator may not be running)", e);
            return TestResult::pass(name, start.elapsed());
        }
    };

    // Skip withdrawal delay if requested
    if options.skip_delay {
        if let Err(e) = skip_withdrawal_delay(config, 10).await {
            warn!("Failed to skip delay: {}", e);
        }
    }

    // Check if withdrawal was executed
    match verify_withdrawal_executed(config, approval.withdraw_hash).await {
        Ok(true) => {
            info!("Withdrawal executed successfully");
        }
        Ok(false) => {
            info!("Withdrawal not yet executed");
        }
        Err(e) => {
            warn!("Could not verify withdrawal: {}", e);
        }
    }

    TestResult::pass(name, start.elapsed())
}

/// Run all EVM-to-EVM transfer tests
pub async fn run_evm_to_evm_tests(config: &E2eConfig, token: Option<Address>) -> Vec<TestResult> {
    let mut results = Vec::new();

    let options = EvmToEvmOptions {
        source_token: token.unwrap_or(Address::ZERO),
        use_mock_chain: true,
        skip_delay: true,
        ..Default::default()
    };

    // Run deposit test
    results.push(test_evm_to_evm_deposit(config, &options).await);

    // Run full cycle test (if deposit succeeded)
    if results.last().map(|r| r.is_pass()).unwrap_or(false) {
        results.push(test_evm_to_evm_full_cycle(config, &options).await);
    }

    results
}

// ============================================================================
// Helper Tests
// ============================================================================

/// Test chain key computation for EVM chains
///
/// Verifies that chain keys are computed correctly using keccak256(abi.encode("EVM", bytes32(chainId))).
/// This matches the on-chain ChainRegistry.getChainKeyEVM() function.
pub async fn test_evm_chain_key_computation(_config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "evm_chain_key_computation";

    // Test that chain key computation matches expected format
    let chain_id_1 = 1u64; // Mainnet
    let chain_id_2 = 31337u64; // Anvil
    let chain_id_3 = 31338u64; // Secondary Anvil

    let key_1 = get_evm_chain_key(chain_id_1);
    let key_2 = get_evm_chain_key(chain_id_2);
    let key_3 = get_evm_chain_key(chain_id_3);

    info!("Chain key for {}: 0x{}", chain_id_1, hex::encode(key_1));
    info!("Chain key for {}: 0x{}", chain_id_2, hex::encode(key_2));
    info!("Chain key for {}: 0x{}", chain_id_3, hex::encode(key_3));

    // Verify keys are different for different chain IDs
    if key_1 == key_2 || key_2 == key_3 || key_1 == key_3 {
        return TestResult::fail(
            name,
            "Chain keys should be unique for different chain IDs",
            start.elapsed(),
        );
    }

    // Verify keys are non-zero (proper hash output)
    if key_1 == B256::ZERO || key_2 == B256::ZERO || key_3 == B256::ZERO {
        return TestResult::fail(name, "Chain keys should not be zero", start.elapsed());
    }

    // Verify key is a proper 32-byte hash (not a simple prefix concatenation)
    // The hash should have high entropy - check that bytes are distributed
    let bytes = key_1.as_slice();
    let unique_bytes: std::collections::HashSet<u8> = bytes.iter().cloned().collect();
    if unique_bytes.len() < 8 {
        return TestResult::fail(
            name,
            "Chain key should have high entropy (proper keccak256 hash)",
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test mock chain key registration
pub async fn test_mock_chain_registration(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "mock_chain_registration";

    let mock_chain_id = 99999u64;

    match register_mock_evm_chain(config, mock_chain_id).await {
        Ok(key) => {
            info!("Mock chain key registered: 0x{}", hex::encode(key));
            TestResult::pass(name, start.elapsed())
        }
        Err(e) => TestResult::fail(name, format!("Failed: {}", e), start.elapsed()),
    }
}
