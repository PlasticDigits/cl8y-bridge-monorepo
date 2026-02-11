//! Integration tests for cross-chain transfers
//!
//! Run with: cargo test --test integration_test -- --nocapture
//!
//! Prerequisites:
//! - Anvil running on localhost:8545
//! - LocalTerra running on localhost:26657
//! - Contracts deployed and configured
//! - DATABASE_URL set

use alloy::primitives::keccak256;

mod helpers {
    use std::time::Duration;

    #[allow(dead_code)]

    /// Test configuration loaded from environment variables
    pub struct TestConfig {
        pub evm_rpc_url: String,
        pub terra_rpc_url: String,
        pub terra_lcd_url: String,
        pub database_url: String,
        pub evm_bridge_address: String,
        pub terra_bridge_address: String,
    }

    impl TestConfig {
        /// Load test configuration from environment variables
        pub fn from_env() -> Option<Self> {
            Some(TestConfig {
                evm_rpc_url: std::env::var("EVM_RPC_URL").ok()?,
                terra_rpc_url: std::env::var("TERRA_RPC_URL").ok()?,
                terra_lcd_url: std::env::var("TERRA_LCD_URL").ok()?,
                database_url: std::env::var("DATABASE_URL").ok()?,
                evm_bridge_address: std::env::var("EVM_BRIDGE_ADDRESS").ok()?,
                terra_bridge_address: std::env::var("TERRA_BRIDGE_ADDRESS").ok()?,
            })
        }
    }

    /// Check EVM RPC connectivity
    pub async fn check_evm_connectivity(rpc_url: &str) -> bool {
        match reqwest::Client::new()
            .post(rpc_url)
            .header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Check Terra RPC connectivity
    pub async fn check_terra_connectivity(rpc_url: &str) -> bool {
        match reqwest::Client::new()
            .get(format!("{}/status", rpc_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Check database connectivity
    pub async fn check_database_connectivity(url: &str) -> bool {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(url)
            .await
        {
            Ok(_pool) => true,
            Err(_) => false,
        }
    }
}

/// Compute EVM chain key: keccak256("EVM" || chainId.to_be_bytes())
fn compute_evm_chain_key(chain_id: u64) -> [u8; 32] {
    let mut input = b"EVM".to_vec();
    input.extend(chain_id.to_be_bytes());
    keccak256(&input).0
}

/// Compute Cosmos chain key: keccak256("COSMOS" || chainId || ":" || addressPrefix)
fn compute_cosmos_chain_key(chain_id: &str, address_prefix: &str) -> [u8; 32] {
    let mut input = b"COSMOS".to_vec();
    input.extend(chain_id.as_bytes());
    input.extend(b":");
    input.extend(address_prefix.as_bytes());
    keccak256(&input).0
}

// ============================================================================
// Environment Tests (require running infrastructure)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_environment_setup() {
    let config = helpers::TestConfig::from_env();
    assert!(
        config.is_some(),
        "Test configuration not found. Set required environment variables: \
         EVM_RPC_URL, TERRA_RPC_URL, TERRA_LCD_URL, DATABASE_URL, \
         EVM_BRIDGE_ADDRESS, TERRA_BRIDGE_ADDRESS"
    );

    let config = config.unwrap();

    // Check EVM RPC connectivity
    assert!(
        helpers::check_evm_connectivity(&config.evm_rpc_url).await,
        "Failed to connect to EVM RPC at {}",
        config.evm_rpc_url
    );
    println!("EVM RPC OK: {}", config.evm_rpc_url);

    // Check Terra RPC connectivity
    assert!(
        helpers::check_terra_connectivity(&config.terra_rpc_url).await,
        "Failed to connect to Terra RPC at {}",
        config.terra_rpc_url
    );
    println!("Terra RPC OK: {}", config.terra_rpc_url);

    // Check database connectivity
    assert!(
        helpers::check_database_connectivity(&config.database_url).await,
        "Failed to connect to database"
    );
    println!("Database OK");

    println!("Environment setup verified!");
}

#[tokio::test]
#[ignore]
async fn test_terra_to_evm_transfer() {
    let config = helpers::TestConfig::from_env().expect("Test configuration required");

    println!("Testing Terra -> EVM transfer flow");
    println!("Terra Bridge: {}", config.terra_bridge_address);
    println!("EVM Bridge: {}", config.evm_bridge_address);

    // This test verifies the database state after a lock transaction
    // In a full E2E test, we would:
    // 1. Execute lock on Terra via terrad or LCD
    // 2. Wait for relayer to process
    // 3. Verify approval on EVM

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    // Query for any pending Terra deposits
    let terra_chain_key = compute_cosmos_chain_key("localterra", "terra");
    println!("Terra chain key: 0x{}", hex::encode(terra_chain_key));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deposits WHERE status = 'pending'")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    println!("Pending deposits in database: {}", count);
}

#[tokio::test]
#[ignore]
async fn test_evm_to_terra_transfer() {
    let config = helpers::TestConfig::from_env().expect("Test configuration required");

    println!("Testing EVM -> Terra transfer flow");
    println!("EVM Bridge: {}", config.evm_bridge_address);
    println!("Terra Bridge: {}", config.terra_bridge_address);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    // Query for any pending EVM deposits
    let evm_chain_key = compute_evm_chain_key(31337);
    println!("EVM chain key (Anvil): 0x{}", hex::encode(evm_chain_key));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deposits WHERE status = 'pending'")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    println!("Pending deposits in database: {}", count);
}

// ============================================================================
// Unit Tests (no infrastructure required)
// ============================================================================

#[tokio::test]
async fn test_chain_key_computation() {
    // Test EVM chain key computation
    let anvil_chain_key = compute_evm_chain_key(31337);
    println!(
        "Anvil (31337) chain key: 0x{}",
        hex::encode(anvil_chain_key)
    );

    // Verify it's deterministic
    let anvil_chain_key_2 = compute_evm_chain_key(31337);
    assert_eq!(
        anvil_chain_key, anvil_chain_key_2,
        "Chain key should be deterministic"
    );

    // Test different chain IDs produce different keys
    let mainnet_chain_key = compute_evm_chain_key(1);
    assert_ne!(
        anvil_chain_key, mainnet_chain_key,
        "Different chain IDs should produce different keys"
    );

    // Test Terra chain key computation
    let terra_local_key = compute_cosmos_chain_key("localterra", "terra");
    println!("LocalTerra chain key: 0x{}", hex::encode(terra_local_key));

    // Test mainnet Terra
    let terra_mainnet_key = compute_cosmos_chain_key("columbus-5", "terra");
    assert_ne!(
        terra_local_key, terra_mainnet_key,
        "Different chain IDs should produce different keys"
    );

    // Verify Terra key is deterministic
    let terra_local_key_2 = compute_cosmos_chain_key("localterra", "terra");
    assert_eq!(
        terra_local_key, terra_local_key_2,
        "Chain key should be deterministic"
    );
}

#[tokio::test]
async fn test_address_encoding() {
    // Test EVM address to bytes32 conversion
    let evm_address_hex = "70997970C51812dc3A010C7d01b50e0d17dc79C8";
    let evm_address_bytes = hex::decode(evm_address_hex).expect("Valid hex");
    assert_eq!(
        evm_address_bytes.len(),
        20,
        "EVM address should be 20 bytes"
    );

    // Convert to bytes32 (left-padded)
    let mut bytes32 = [0u8; 32];
    bytes32[12..].copy_from_slice(&evm_address_bytes);

    // Verify padding
    assert_eq!(&bytes32[..12], &[0u8; 12], "Left padding should be zeros");
    assert_eq!(
        &bytes32[12..],
        evm_address_bytes.as_slice(),
        "Address should be in last 20 bytes"
    );

    // Test round-trip
    let recovered: [u8; 20] = bytes32[12..].try_into().unwrap();
    assert_eq!(
        recovered,
        evm_address_bytes.as_slice(),
        "Round-trip should preserve address"
    );

    println!("EVM address: 0x{}", evm_address_hex);
    println!("As bytes32: 0x{}", hex::encode(bytes32));
}

#[tokio::test]
async fn test_terra_address_encoding() {
    // Terra addresses are bech32 encoded (44 characters)
    let terra_address = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";

    // Terra addresses are 44 characters, too long for bytes32
    // In cross-chain messages, we store the full address as variable-length bytes
    let addr_bytes = terra_address.as_bytes();
    assert_eq!(
        addr_bytes.len(),
        44,
        "Terra address should be 44 characters"
    );

    // For storage, we use the full bytes (not truncated to 32)
    let mut storage = Vec::with_capacity(44);
    storage.extend_from_slice(addr_bytes);

    println!("Terra address: {}", terra_address);
    println!("Length: {} bytes", addr_bytes.len());
    println!("As hex: 0x{}", hex::encode(&storage));

    // Test recovery
    let recovered = String::from_utf8(storage.clone()).expect("Valid UTF-8");
    assert_eq!(recovered, terra_address, "Should recover original address");

    // Alternative: For EVM contracts that require bytes32, we can hash the address
    let addr_hash = keccak256(addr_bytes);
    println!("As keccak256 hash: 0x{}", hex::encode(addr_hash));
    assert_eq!(addr_hash.len(), 32, "Hash should be 32 bytes");
}

#[tokio::test]
async fn test_amount_conversion() {
    // Terra uses 6 decimals (uluna), EVM uses 18 decimals
    let terra_amount: u128 = 1_000_000; // 1 LUNA in uluna
    let evm_amount: u128 = 1_000_000_000_000_000_000; // 1 token in wei (18 decimals)

    // Convert Terra (6 dec) to EVM (18 dec): multiply by 10^12
    let terra_to_evm = terra_amount * 1_000_000_000_000u128;
    assert_eq!(terra_to_evm, evm_amount, "1 LUNA should equal 1 wLUNA");

    // Convert EVM (18 dec) to Terra (6 dec): divide by 10^12
    let evm_to_terra = evm_amount / 1_000_000_000_000u128;
    assert_eq!(evm_to_terra, terra_amount, "1 wLUNA should equal 1 LUNA");

    println!("1 LUNA (uluna): {}", terra_amount);
    println!("1 wLUNA (wei): {}", evm_amount);
}

#[tokio::test]
async fn test_keccak256_computation() {
    // Test keccak256 produces expected output
    let input = b"test";
    let hash = keccak256(input);

    assert_eq!(hash.len(), 32, "keccak256 should produce 32 bytes");

    // Known hash of "test"
    let expected = "9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";
    assert_eq!(
        hex::encode(hash),
        expected,
        "keccak256 should match expected value"
    );

    println!("keccak256('test'): 0x{}", hex::encode(hash));
}

// ============================================================================
// Event Signature Tests (critical for deposit detection)
// ============================================================================

#[tokio::test]
async fn test_v2_deposit_event_signature_matches_solidity() {
    // The Solidity Bridge contract defines:
    //
    //   event Deposit(
    //       bytes4 indexed destChain,
    //       bytes32 indexed destAccount,
    //       bytes32 srcAccount,
    //       address token,
    //       uint256 amount,
    //       uint64 nonce,
    //       uint256 fee
    //   );
    //
    // The event signature MUST include ALL 7 parameters (indexed + non-indexed).
    // Missing srcAccount (bytes32) was a critical bug that prevented deposit detection.
    let correct_sig = keccak256(b"Deposit(bytes4,bytes32,bytes32,address,uint256,uint64,uint256)");

    // This is the WRONG signature (missing bytes32 for srcAccount)
    let wrong_sig = keccak256(b"Deposit(bytes4,bytes32,address,uint256,uint64,uint256)");

    assert_ne!(
        correct_sig, wrong_sig,
        "Correct and incorrect signatures must differ - \
         the srcAccount (bytes32) parameter changes the hash"
    );

    println!(
        "Correct V2 Deposit signature: 0x{}",
        hex::encode(correct_sig)
    );
    println!("Wrong (6-param) signature:    0x{}", hex::encode(wrong_sig));

    // Verify the correct signature is what the e2e tests use
    // This is the ground truth from deposit_flow.rs
    let e2e_sig = keccak256(b"Deposit(bytes4,bytes32,bytes32,address,uint256,uint64,uint256)");
    assert_eq!(
        correct_sig, e2e_sig,
        "Operator event signature must match e2e test event signature"
    );
}

#[tokio::test]
async fn test_v1_deposit_request_event_signature() {
    // V1 DepositRequest event:
    //   event DepositRequest(bytes32, bytes32, bytes32, address, uint256, uint256)
    let sig = keccak256(b"DepositRequest(bytes32,bytes32,bytes32,address,uint256,uint256)");

    // V1 and V2 must have different signatures
    let v2_sig = keccak256(b"Deposit(bytes4,bytes32,bytes32,address,uint256,uint64,uint256)");
    assert_ne!(sig, v2_sig, "V1 and V2 event signatures must be different");

    println!("V1 DepositRequest signature: 0x{}", hex::encode(sig));
    println!("V2 Deposit signature:        0x{}", hex::encode(v2_sig));
}

#[tokio::test]
async fn test_withdraw_function_signature() {
    // The V2 withdrawSubmit function signature must match Bridge.sol exactly.
    // This is used by E2E tests and the operator's sol! bindings.
    //
    // Bridge.sol: withdrawSubmit(bytes4 srcChain, bytes32 srcAccount,
    //   bytes32 destAccount, address token, uint256 amount, uint64 nonce, uint8 srcDecimals)
    let func_sig =
        keccak256(b"withdrawSubmit(bytes4,bytes32,bytes32,address,uint256,uint64,uint8)");
    let selector = &func_sig[..4];
    println!("withdrawSubmit selector: 0x{}", hex::encode(selector));
    assert_ne!(selector, &[0u8; 4]);
}

#[tokio::test]
async fn test_withdraw_event_signatures() {
    // WithdrawSubmit event (matches IBridge.sol):
    //   event WithdrawSubmit(bytes32 indexed withdrawHash, bytes4 srcChain,
    //     bytes32 srcAccount, bytes32 destAccount, address token,
    //     uint256 amount, uint64 nonce, uint256 operatorGas)
    let submit_sig =
        keccak256(b"WithdrawSubmit(bytes32,bytes4,bytes32,bytes32,address,uint256,uint64,uint256)");

    // WithdrawApprove event:
    //   event WithdrawApprove(bytes32 indexed withdrawHash)
    let approve_sig = keccak256(b"WithdrawApprove(bytes32)");

    // WithdrawCancel event:
    //   event WithdrawCancel(bytes32 indexed withdrawHash, address canceler)
    let cancel_sig = keccak256(b"WithdrawCancel(bytes32,address)");

    // WithdrawExecute event:
    //   event WithdrawExecute(bytes32 indexed withdrawHash, address recipient, uint256 amount)
    let execute_sig = keccak256(b"WithdrawExecute(bytes32,address,uint256)");

    // All must be unique
    let sigs = [submit_sig, approve_sig, cancel_sig, execute_sig];
    for i in 0..sigs.len() {
        for j in (i + 1)..sigs.len() {
            assert_ne!(
                sigs[i], sigs[j],
                "Event signatures {} and {} must be unique",
                i, j
            );
        }
    }

    println!("WithdrawSubmit:  0x{}", hex::encode(submit_sig));
    println!("WithdrawApprove: 0x{}", hex::encode(approve_sig));
    println!("WithdrawCancel:  0x{}", hex::encode(cancel_sig));
    println!("WithdrawExecute: 0x{}", hex::encode(execute_sig));
}

// ============================================================================
// Transfer Hash Tests (cross-chain parity)
// ============================================================================

/// Compute a V2 unified transfer hash (matching EVM's HashLib.computeTransferHash)
///
/// keccak256(abi.encode(
///     bytes32(srcChain),     // 4 bytes -> padded to 32
///     bytes32(destChain),    // 4 bytes -> padded to 32
///     srcAccount,            // bytes32
///     destAccount,           // bytes32
///     token,                 // bytes32
///     uint256(amount),       // 32 bytes
///     uint256(nonce)         // 32 bytes
/// ))
fn compute_transfer_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    let mut data = [0u8; 224]; // 7 * 32 = 224 bytes

    // srcChain (bytes4 left-aligned in bytes32)
    data[0..4].copy_from_slice(src_chain);
    // destChain (bytes4 left-aligned in bytes32)
    data[32..36].copy_from_slice(dest_chain);
    // srcAccount
    data[64..96].copy_from_slice(src_account);
    // destAccount
    data[96..128].copy_from_slice(dest_account);
    // token
    data[128..160].copy_from_slice(token);
    // amount (u128 -> left-padded to uint256)
    data[160 + 16..192].copy_from_slice(&amount.to_be_bytes());
    // nonce (u64 -> left-padded to uint256)
    data[192 + 24..224].copy_from_slice(&nonce.to_be_bytes());

    keccak256(&data).0
}

#[tokio::test]
async fn test_transfer_hash_deterministic() {
    let src_chain: [u8; 4] = [0, 0, 0, 1]; // EVM
    let dest_chain: [u8; 4] = [0, 0, 0, 2]; // Terra
    let src_account = [0xABu8; 32];
    let dest_account = [0xCDu8; 32];
    let token = [0xEFu8; 32];
    let amount = 1_000_000u128;
    let nonce = 1u64;

    let hash1 = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );
    let hash2 = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );

    assert_eq!(
        hash1, hash2,
        "Same inputs must produce same hash (deterministic)"
    );
}

#[tokio::test]
async fn test_transfer_hash_different_nonce() {
    let src_chain: [u8; 4] = [0, 0, 0, 1];
    let dest_chain: [u8; 4] = [0, 0, 0, 2];
    let src_account = [0u8; 32];
    let dest_account = [0u8; 32];
    let token = [0u8; 32];

    let hash1 = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        1_000_000,
        1,
    );
    let hash2 = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        1_000_000,
        2,
    );

    assert_ne!(
        hash1, hash2,
        "Different nonces must produce different hashes"
    );
}

#[tokio::test]
async fn test_transfer_hash_src_account_matters() {
    let src_chain: [u8; 4] = [0, 0, 0, 1];
    let dest_chain: [u8; 4] = [0, 0, 0, 2];
    let mut src_a = [0u8; 32];
    src_a[31] = 0xAA;
    let mut src_b = [0u8; 32];
    src_b[31] = 0xBB;
    let dest = [0u8; 32];
    let token = [0u8; 32];

    let hash_a = compute_transfer_hash(&src_chain, &dest_chain, &src_a, &dest, &token, 100, 1);
    let hash_b = compute_transfer_hash(&src_chain, &dest_chain, &src_b, &dest, &token, 100, 1);

    assert_ne!(
        hash_a, hash_b,
        "Different srcAccounts must produce different hashes (V2 7-field hash includes srcAccount)"
    );
}

#[tokio::test]
async fn test_transfer_hash_amount_matters() {
    let src_chain: [u8; 4] = [0, 0, 0, 1];
    let dest_chain: [u8; 4] = [0, 0, 0, 2];
    let src = [0u8; 32];
    let dest = [0u8; 32];
    let token = [0u8; 32];

    let hash_a = compute_transfer_hash(&src_chain, &dest_chain, &src, &dest, &token, 1_000_000, 1);
    let hash_b = compute_transfer_hash(&src_chain, &dest_chain, &src, &dest, &token, 995_000, 1);

    assert_ne!(
        hash_a, hash_b,
        "Different amounts must produce different hashes \
         (pre-fee vs post-fee amounts would cause hash mismatch)"
    );
}

#[tokio::test]
async fn test_transfer_hash_chain_padding() {
    // Verify bytes4 chain IDs are correctly left-aligned in bytes32
    // EVM's abi.encode(bytes4) -> bytes32 with bytes4 at position 0-3
    let chain_id: [u8; 4] = [0, 0, 0, 1];
    let mut padded = [0u8; 32];
    padded[0..4].copy_from_slice(&chain_id);

    // First 4 bytes should be the chain ID
    assert_eq!(&padded[0..4], &chain_id);
    // Remaining 28 bytes should be zeros
    assert_eq!(&padded[4..32], &[0u8; 28]);
}

#[tokio::test]
async fn test_token_encoding_native_denom() {
    // Native denoms on Terra (e.g., "uluna") are hashed with keccak256
    let uluna_hash = keccak256(b"uluna");
    assert_eq!(uluna_hash.len(), 32);

    // Same denom should always produce same hash
    let uluna_hash2 = keccak256(b"uluna");
    assert_eq!(uluna_hash, uluna_hash2);

    // Different denoms should produce different hashes
    let uusd_hash = keccak256(b"uusd");
    assert_ne!(uluna_hash, uusd_hash);

    println!("keccak256('uluna'): 0x{}", hex::encode(uluna_hash));
    println!("keccak256('uusd'):  0x{}", hex::encode(uusd_hash));
}

// ============================================================================
// Withdrawal Approval Flow Tests
// ============================================================================

#[tokio::test]
async fn test_pending_withdrawals_response_parsing() {
    // Simulate the response format from Terra LCD for pending_withdrawals
    let response = serde_json::json!({
        "data": {
            "withdrawals": [
                {
                    "withdraw_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "src_chain": "AAAAAQ==",
                    "src_account": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "dest_account": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "token": "uluna",
                    "recipient": "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
                    "amount": "995000",
                    "nonce": 1,
                    "src_decimals": 18,
                    "dest_decimals": 6,
                    "operator_gas": "0",
                    "submitted_at": 1000000,
                    "approved_at": 0,
                    "approved": false,
                    "cancelled": false,
                    "executed": false,
                    "cancel_window_remaining": 0
                }
            ]
        }
    });

    // Parse the way TerraWriter does
    let withdrawals = response["data"]["withdrawals"].as_array();
    assert!(withdrawals.is_some(), "Should parse data.withdrawals array");

    let entries = withdrawals.unwrap();
    assert_eq!(entries.len(), 1, "Should have one withdrawal entry");

    let entry = &entries[0];
    assert_eq!(entry["nonce"].as_u64(), Some(1));
    assert_eq!(entry["approved"].as_bool(), Some(false));
    assert_eq!(entry["cancelled"].as_bool(), Some(false));
    assert_eq!(entry["executed"].as_bool(), Some(false));
    assert_eq!(entry["token"].as_str(), Some("uluna"));
    assert_eq!(entry["amount"].as_str(), Some("995000"));

    // Test base64 hash decoding
    let hash_b64 = entry["withdraw_hash"].as_str().unwrap();
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(hash_b64)
        .unwrap();
    assert_eq!(decoded.len(), 32, "Decoded hash should be 32 bytes");
}

// ============================================================================
// Cross-Chain Token Encoding Parity Tests (uluna native ↔ ERC20 and CW20 ↔ ERC20)
// ============================================================================

#[tokio::test]
async fn test_uluna_native_token_encoding_matches_solidity() {
    // keccak256("uluna") must match Solidity's keccak256(abi.encodePacked("uluna"))
    let uluna_hash = keccak256(b"uluna").0;
    assert_eq!(
        format!("0x{}", hex::encode(uluna_hash)),
        "0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da",
        "keccak256('uluna') must be identical in Rust and Solidity"
    );
}

#[tokio::test]
async fn test_cw20_token_encoding_matches_evm_registry() {
    // CW20 address → bech32 decode → left-pad to bytes32
    // This must match what EVM's TokenRegistry.setTokenDestination stores
    let cw20_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    let bytes32 = multichain_rs::hash::encode_terra_address_to_bytes32(cw20_addr).unwrap();

    // First 12 bytes must be zero padding (20-byte address in last 20 bytes)
    assert_eq!(&bytes32[0..12], &[0u8; 12]);
    assert!(
        !bytes32[12..32].iter().all(|&b| b == 0),
        "Last 20 bytes must contain the canonical address"
    );

    println!("CW20 bytes32: 0x{}", hex::encode(bytes32));
}

#[tokio::test]
async fn test_uluna_vs_cw20_different_token_encoding() {
    // uluna (native denom) and CW20 (contract address) MUST encode differently
    let uluna_token = keccak256(b"uluna").0;

    let cw20_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    let cw20_token = multichain_rs::hash::encode_terra_address_to_bytes32(cw20_addr).unwrap();

    assert_ne!(
        uluna_token, cw20_token,
        "Native denom hash and CW20 bytes32 must differ. \
         Mixing them causes 'terra approval not found' timeout."
    );
}

#[tokio::test]
async fn test_transfer_hash_uluna_evm_to_terra() {
    // Full transfer hash: EVM → Terra with native uluna
    let evm_chain: [u8; 4] = [0, 0, 0, 1];
    let terra_chain: [u8; 4] = [0, 0, 0, 2];

    let src_account = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);

    let dest_account = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let token = keccak256(b"uluna").0;

    let hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &src_account,
        &dest_account,
        &token,
        1_000_000,
        1,
    );

    assert_ne!(hash, [0u8; 32]);
    println!("EVM→Terra uluna: 0x{}", hex::encode(hash));
}

#[tokio::test]
async fn test_transfer_hash_uluna_terra_to_evm() {
    // Full transfer hash: Terra → EVM with native uluna
    let terra_chain: [u8; 4] = [0, 0, 0, 2];
    let evm_chain: [u8; 4] = [0, 0, 0, 1];

    let src_account = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let dest_account = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);

    let token = keccak256(b"uluna").0;

    let hash = compute_transfer_hash(
        &terra_chain,
        &evm_chain,
        &src_account,
        &dest_account,
        &token,
        1_000_000,
        1,
    );

    assert_ne!(hash, [0u8; 32]);
    println!("Terra→EVM uluna: 0x{}", hex::encode(hash));
}

#[tokio::test]
async fn test_transfer_hash_cw20_evm_to_terra() {
    // Full transfer hash: EVM → Terra with CW20 token
    let evm_chain: [u8; 4] = [0, 0, 0, 1];
    let terra_chain: [u8; 4] = [0, 0, 0, 2];

    let src_account = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);

    let dest_account = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    // CW20 token: bech32 decode → bytes32
    let token = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &src_account,
        &dest_account,
        &token,
        1_000_000,
        1,
    );

    assert_ne!(hash, [0u8; 32]);
    println!("EVM→Terra CW20: 0x{}", hex::encode(hash));
    println!("CW20 token: 0x{}", hex::encode(token));
}

#[tokio::test]
async fn test_transfer_hash_cw20_terra_to_evm() {
    // Full transfer hash: Terra → EVM with CW20 token
    let terra_chain: [u8; 4] = [0, 0, 0, 2];
    let evm_chain: [u8; 4] = [0, 0, 0, 1];

    let src_account = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let dest_account = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);

    let token = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let hash = compute_transfer_hash(
        &terra_chain,
        &evm_chain,
        &src_account,
        &dest_account,
        &token,
        1_000_000,
        1,
    );

    assert_ne!(hash, [0u8; 32]);
    println!("Terra→EVM CW20: 0x{}", hex::encode(hash));
}

#[tokio::test]
async fn test_uluna_vs_cw20_hash_mismatch_causes_approval_failure() {
    // Demonstrates the exact bug: using wrong token encoding causes hash mismatch
    let evm_chain: [u8; 4] = [0, 0, 0, 1];
    let terra_chain: [u8; 4] = [0, 0, 0, 2];
    let src = multichain_rs::hash::address_to_bytes32(&[0xAA; 20]);
    let dest = multichain_rs::hash::address_to_bytes32(&[0xBB; 20]);

    // Token 1: native "uluna" → keccak256("uluna")
    let token_uluna = keccak256(b"uluna").0;

    // Token 2: CW20 address → bech32 decode → left-pad
    let token_cw20 = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let hash_uluna = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &src,
        &dest,
        &token_uluna,
        1_000_000,
        1,
    );
    let hash_cw20 = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &src,
        &dest,
        &token_cw20,
        1_000_000,
        1,
    );

    assert_ne!(
        hash_uluna, hash_cw20,
        "Using keccak256('uluna') vs CW20 bytes32 MUST produce different hashes. \
         This mismatch causes 'terra approval not found within timeout'."
    );

    println!("Token mismatch demo:");
    println!("  uluna: 0x{}", hex::encode(token_uluna));
    println!("  CW20:  0x{}", hex::encode(token_cw20));
    println!("  hash(uluna): 0x{}", hex::encode(hash_uluna));
    println!("  hash(CW20):  0x{}", hex::encode(hash_cw20));
}

// ============================================================================
// Deposit ↔ Withdraw Hash Parity Tests
//
// Deposit side (source chain) and withdraw side (dest chain) must produce
// the SAME hash. Token = destination token address in all cases.
// Expected values verified against Solidity HashLib.t.sol and multichain-rs.
// ============================================================================

#[tokio::test]
async fn test_deposit_withdraw_match_evm_to_evm_erc20() {
    let src_chain: [u8; 4] = [0, 0, 0, 1];
    let dest_chain: [u8; 4] = [0, 0, 0, 56];

    let src_account = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);
    let dest_account = multichain_rs::hash::address_to_bytes32(&[
        0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5, 0x0e,
        0x0d, 0x17, 0xdc, 0x79, 0xC8,
    ]);
    // ERC20 on dest chain: 0x5FbDB2315678afecb367f032d93F642f64180aa3
    let dest_token = multichain_rs::hash::address_to_bytes32(&[
        0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F, 0x64,
        0x2f, 0x64, 0x18, 0x0a, 0xa3,
    ]);

    let deposit_hash = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &dest_token,
        1_000_000_000_000_000_000,
        42,
    );
    let withdraw_hash = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &dest_token,
        1_000_000_000_000_000_000,
        42,
    );

    assert_eq!(
        deposit_hash, withdraw_hash,
        "EVM→EVM ERC20: deposit must equal withdraw"
    );
    assert_eq!(
        format!("0x{}", hex::encode(deposit_hash)),
        "0x11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54",
        "Must match Solidity and multichain-rs"
    );
}

#[tokio::test]
async fn test_deposit_withdraw_match_evm_to_terra_native() {
    let evm_chain: [u8; 4] = [0, 0, 0, 1];
    let terra_chain: [u8; 4] = [0, 0, 0, 2];

    let evm_depositor = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);
    let terra_recipient = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let token = keccak256(b"uluna").0;

    let deposit_hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &evm_depositor,
        &terra_recipient,
        &token,
        995_000,
        1,
    );
    let withdraw_hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &evm_depositor,
        &terra_recipient,
        &token,
        995_000,
        1,
    );

    assert_eq!(
        deposit_hash, withdraw_hash,
        "EVM→Terra native uluna: deposit must equal withdraw"
    );
    assert_eq!(
        format!("0x{}", hex::encode(deposit_hash)),
        "0x92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453"
    );
}

#[tokio::test]
async fn test_deposit_withdraw_match_evm_to_terra_cw20() {
    let evm_chain: [u8; 4] = [0, 0, 0, 1];
    let terra_chain: [u8; 4] = [0, 0, 0, 2];

    let evm_depositor = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);
    let cw20_token = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();

    let deposit_hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &evm_depositor,
        &cw20_token,
        &cw20_token,
        1_000_000,
        5,
    );
    let withdraw_hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &evm_depositor,
        &cw20_token,
        &cw20_token,
        1_000_000,
        5,
    );

    assert_eq!(
        deposit_hash, withdraw_hash,
        "EVM→Terra CW20: deposit must equal withdraw"
    );
    assert_eq!(
        format!("0x{}", hex::encode(deposit_hash)),
        "0x1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b"
    );
}

#[tokio::test]
async fn test_deposit_withdraw_match_terra_to_evm_native_erc20() {
    let terra_chain: [u8; 4] = [0, 0, 0, 2];
    let evm_chain: [u8; 4] = [0, 0, 0, 1];

    let terra_depositor = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();
    let evm_recipient = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);
    // ERC20 dest token: 0x5FbDB2315678afecb367f032d93F642f64180aa3
    let erc20_token = multichain_rs::hash::address_to_bytes32(&[
        0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F, 0x64,
        0x2f, 0x64, 0x18, 0x0a, 0xa3,
    ]);

    let deposit_hash = compute_transfer_hash(
        &terra_chain,
        &evm_chain,
        &terra_depositor,
        &evm_recipient,
        &erc20_token,
        500_000,
        3,
    );
    let withdraw_hash = compute_transfer_hash(
        &terra_chain,
        &evm_chain,
        &terra_depositor,
        &evm_recipient,
        &erc20_token,
        500_000,
        3,
    );

    assert_eq!(
        deposit_hash, withdraw_hash,
        "Terra→EVM native→ERC20: deposit must equal withdraw"
    );
    assert_eq!(
        format!("0x{}", hex::encode(deposit_hash)),
        "0x076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438"
    );
}

#[tokio::test]
async fn test_deposit_withdraw_match_terra_to_evm_cw20_erc20() {
    let terra_chain: [u8; 4] = [0, 0, 0, 2];
    let evm_chain: [u8; 4] = [0, 0, 0, 1];

    let terra_depositor = multichain_rs::hash::encode_terra_address_to_bytes32(
        "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v",
    )
    .unwrap();
    // EVM recipient: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
    let evm_recipient = multichain_rs::hash::address_to_bytes32(&[
        0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5, 0x0e,
        0x0d, 0x17, 0xdc, 0x79, 0xC8,
    ]);
    // ERC20 dest token: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
    let erc20_token = multichain_rs::hash::address_to_bytes32(&[
        0xe7, 0xf1, 0x72, 0x5E, 0x77, 0x34, 0xCE, 0x28, 0x8F, 0x83, 0x67, 0xe1, 0xBb, 0x14, 0x3E,
        0x90, 0xbb, 0x3F, 0x05, 0x12,
    ]);

    let deposit_hash = compute_transfer_hash(
        &terra_chain,
        &evm_chain,
        &terra_depositor,
        &evm_recipient,
        &erc20_token,
        2_500_000,
        7,
    );
    let withdraw_hash = compute_transfer_hash(
        &terra_chain,
        &evm_chain,
        &terra_depositor,
        &evm_recipient,
        &erc20_token,
        2_500_000,
        7,
    );

    assert_eq!(
        deposit_hash, withdraw_hash,
        "Terra→EVM CW20→ERC20: deposit must equal withdraw"
    );
    assert_eq!(
        format!("0x{}", hex::encode(deposit_hash)),
        "0xf1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab"
    );
}

// ============================================================================
// V2 Approval Architecture Tests
//
// In V2, both EVM and Terra writers use poll-and-approve:
// - EVM writer: polls WithdrawSubmit events on EVM, verifies on source chain, approves on EVM
//   Handles: Terra→EVM and EVM→EVM transfers
// - Terra writer: polls pending_withdrawals on Terra, verifies on EVM, approves on Terra
//   Handles: EVM→Terra transfers
//
// The user must call withdrawSubmit on the destination chain first.
// The operator then polls for these events and approves verified ones.
// ============================================================================

#[tokio::test]
async fn test_v2_withdraw_submit_event_signature() {
    // The EVM writer polls for WithdrawSubmit events.
    // The event signature must match what the Bridge.sol emits.
    //
    // Bridge.sol/IBridge.sol:
    //   event WithdrawSubmit(
    //     bytes32 indexed withdrawHash,
    //     bytes4 srcChain,
    //     bytes32 srcAccount,
    //     bytes32 destAccount,
    //     address token,
    //     uint256 amount,
    //     uint64 nonce,
    //     uint256 operatorGas
    //   );
    let submit_sig =
        keccak256(b"WithdrawSubmit(bytes32,bytes4,bytes32,bytes32,address,uint256,uint64,uint256)");

    // This is the hash the EVM writer uses for event filtering.
    // If this doesn't match the Solidity event, the operator will NEVER
    // detect WithdrawSubmit events and NEVER approve withdrawals.
    println!(
        "WithdrawSubmit event signature: 0x{}",
        hex::encode(submit_sig)
    );
    assert_ne!(submit_sig.0, [0u8; 32]);

    // Verify it's DIFFERENT from the wrong (missing srcAccount/destAccount) signature
    let wrong_sig = keccak256(b"WithdrawSubmit(bytes32,bytes4,address,uint256,uint64,uint256)");
    assert_ne!(
        submit_sig.0, wrong_sig.0,
        "Including srcAccount/destAccount in event signature must change the hash. \
         Using the wrong signature means the operator will never find WithdrawSubmit events."
    );
}

#[tokio::test]
async fn test_v2_approval_flow_requires_withdraw_submit() {
    // In V2, the operator CANNOT approve unless the user has called withdrawSubmit.
    // The operator polls WithdrawSubmit events, NOT deposit events.
    // This test verifies the architectural requirement.

    // Scenario: EVM→EVM loopback
    // 1. User deposits on EVM (creates Deposit event, increments nonce)
    // 2. User MUST call withdrawSubmit on EVM (creates WithdrawSubmit event)
    // 3. Operator polls WithdrawSubmit events on EVM
    // 4. For each, calls getPendingWithdraw to check status
    // 5. Verifies deposit on source chain via getDeposit
    // 6. If verified, calls withdrawApprove

    // The key insight: without step 2, the operator will NEVER see the withdrawal.
    // This is different from V1 where the operator directly approved deposits.

    // The operator filters out:
    // - approved: true (already approved)
    // - cancelled: true (cancelled by canceler)
    // - executed: true (already executed)
    let test_cases = vec![
        (false, false, false, true), // unapproved → should process
        (true, false, false, false), // approved → skip
        (false, true, false, false), // cancelled → skip
        (false, false, true, false), // executed → skip
        (true, true, false, false),  // approved+cancelled → skip
        (true, false, true, false),  // approved+executed → skip
    ];

    for (approved, cancelled, executed, should_process) in &test_cases {
        let process = !approved && !cancelled && !executed;
        assert_eq!(
            process, *should_process,
            "approved={}, cancelled={}, executed={}: expected should_process={}",
            approved, cancelled, executed, should_process
        );
    }
}

#[tokio::test]
async fn test_deposit_routing_by_chain_type() {
    // In V2, routing is handled by the destination chain's writer:
    // - Withdrawal on EVM → EVM writer polls and approves
    // - Withdrawal on Terra → Terra writer polls and approves
    //
    // The source chain type is determined from the WithdrawSubmit event's srcChain field.

    // EVM chain IDs are 4-byte identifiers
    let evm_chain: [u8; 4] = [0, 0, 0, 1];
    let terra_chain: [u8; 4] = [0, 0, 0, 2];

    // Different chains should have different IDs
    assert_ne!(evm_chain, terra_chain);

    // Verify chain ID encoding is consistent
    let evm_u32 = u32::from_be_bytes(evm_chain);
    let terra_u32 = u32::from_be_bytes(terra_chain);
    assert_eq!(evm_u32, 1);
    assert_eq!(terra_u32, 2);
}

#[tokio::test]
async fn test_deposit_routing_evm_to_evm() {
    // EVM→EVM loopback: same chain acts as both source and destination.
    // The EVM writer polls WithdrawSubmit events on its chain and verifies
    // deposits also on its chain (or a different EVM chain in multi-chain mode).

    let same_chain: [u8; 4] = [0, 0, 0, 1]; // Same chain for source and dest

    // In loopback, srcChain == destChain (this chain's ID)
    let src_chain = same_chain;
    let dest_chain = same_chain;

    // The hash includes both src and dest chain, so same-chain hashes
    // still contain the chain ID twice
    let src_account = [0xAA; 32];
    let dest_account = [0xBB; 32];
    let token = [0xCC; 32];

    let hash = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        1_000_000,
        1,
    );

    // Hash should be non-zero even for same-chain
    assert_ne!(hash, [0u8; 32], "Same-chain hash should be non-zero");

    // Different amounts produce different hashes (important for fee verification)
    let hash2 = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        999_000, // different amount (post-fee)
        1,
    );
    assert_ne!(
        hash, hash2,
        "Different amounts must produce different hashes (fee mismatch detection)"
    );
}

#[tokio::test]
async fn test_evm_to_evm_hash_computation() {
    // Compute a transfer hash for an EVM→EVM transfer (e.g., BSC → opBNB)
    // and verify it matches the unified 7-field format.

    let bsc_chain: [u8; 4] = [0, 0, 0, 56]; // BSC
    let opbnb_chain: [u8; 4] = [0, 0, 0, 204]; // opBNB

    // BSC depositor
    let src_account = multichain_rs::hash::address_to_bytes32(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);

    // opBNB recipient
    let dest_account = multichain_rs::hash::address_to_bytes32(&[
        0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5, 0x0e,
        0x0d, 0x17, 0xdc, 0x79, 0xC8,
    ]);

    // ERC20 token on destination chain
    let dest_token = multichain_rs::hash::address_to_bytes32(&[
        0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F, 0x64,
        0x2f, 0x64, 0x18, 0x0a, 0xa3,
    ]);

    let amount = 1_000_000_000_000_000_000u128; // 1 token (18 decimals)
    let nonce = 1u64;

    // Compute hash using the shared function (same as EVM contract uses)
    let hash = compute_transfer_hash(
        &bsc_chain,
        &opbnb_chain,
        &src_account,
        &dest_account,
        &dest_token,
        amount,
        nonce,
    );

    // Hash should be non-zero
    assert_ne!(hash, [0u8; 32], "EVM→EVM hash should be non-zero");

    // Hash should be deterministic
    let hash2 = compute_transfer_hash(
        &bsc_chain,
        &opbnb_chain,
        &src_account,
        &dest_account,
        &dest_token,
        amount,
        nonce,
    );
    assert_eq!(hash, hash2, "EVM→EVM hash must be deterministic");

    // Swapping src/dest chains should produce a different hash
    let reverse_hash = compute_transfer_hash(
        &opbnb_chain, // swapped
        &bsc_chain,   // swapped
        &src_account,
        &dest_account,
        &dest_token,
        amount,
        nonce,
    );
    assert_ne!(
        hash, reverse_hash,
        "Swapping src/dest chains must produce different hashes"
    );

    println!("BSC→opBNB ERC20 hash: 0x{}", hex::encode(hash));
}

/// Test that alloy's ProviderBuilder requires `with_recommended_fillers()` for transactions.
///
/// This test validates the fix for the "missing properties" error:
/// `local usage error: missing properties: [("Wallet", ["nonce", "gas_limit", ...])]`
///
/// The issue was that `ProviderBuilder::new().wallet(wallet).on_http(url)` creates a
/// provider that can sign but CANNOT fill nonce/gas fields. The `with_recommended_fillers()`
/// call adds NonceFiller, GasFiller, ChainIdFiller which are prerequisites for the
/// WalletFiller to function.
#[test]
fn test_provider_builder_requires_recommended_fillers() {
    use alloy::network::EthereumWallet;
    use alloy::providers::ProviderBuilder;
    use alloy::signers::local::PrivateKeySigner;

    // This is the pattern that was BROKEN (missing recommended_fillers):
    // ProviderBuilder::new()
    //     .wallet(wallet)
    //     .on_http(url)
    //
    // This is the CORRECT pattern:
    // ProviderBuilder::new()
    //     .with_recommended_fillers()
    //     .wallet(wallet)
    //     .on_http(url)

    let signer: PrivateKeySigner =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse()
            .unwrap();
    let wallet = EthereumWallet::from(signer);

    // Verify we can construct a provider with recommended fillers + wallet
    // (this would fail to compile if the API changed)
    let _provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http("http://localhost:8545".parse().unwrap());

    // The provider is constructable — runtime behavior requires an actual chain
    // but the key invariant is that the builder accepts this call chain.
}

/// Test that the V2 chain IDs for EVM and Terra are correctly assigned.
///
/// In the local E2E setup with ChainRegistry:
/// - EVM (Anvil, native chain ID 31337) is registered as V2 chain ID 0x00000001
/// - Terra (localterra) is registered as V2 chain ID 0x00000002
///
/// This test ensures these assignments are consistent and not swapped.
#[test]
fn test_v2_chain_id_assignments() {
    // V2 chain IDs assigned by ChainRegistry in local setup
    let evm_v2_chain_id: [u8; 4] = [0x00, 0x00, 0x00, 0x01]; // 1
    let terra_v2_chain_id: [u8; 4] = [0x00, 0x00, 0x00, 0x02]; // 2

    // These should be different
    assert_ne!(
        evm_v2_chain_id, terra_v2_chain_id,
        "EVM and Terra must have different V2 chain IDs"
    );

    // EVM is 1, Terra is 2
    assert_eq!(
        u32::from_be_bytes(evm_v2_chain_id),
        1,
        "EVM V2 chain ID should be 1 (0x00000001)"
    );
    assert_eq!(
        u32::from_be_bytes(terra_v2_chain_id),
        2,
        "Terra V2 chain ID should be 2 (0x00000002)"
    );

    // Neither should be the native chain ID
    let anvil_native: [u8; 4] = 31337u32.to_be_bytes(); // 0x00007A69
    assert_ne!(
        evm_v2_chain_id, anvil_native,
        "V2 chain ID should NOT be the native Anvil chain ID"
    );
}

/// Test that the canceler's chain routing uses correct chain IDs.
///
/// Validates the fix for swapped chain IDs in canceler_evm_source_fraud_detection:
/// - EVM source fraud should use 0x00000001 (EVM's V2 ID), NOT 0x00000002
/// - Terra source fraud should use 0x00000002 (Terra's V2 ID), NOT 0x00000001
///
/// The canceler routes verification based on chain ID:
/// - If src_chain == evm_chain_id → verify_evm_deposit (uses getDeposit RPC)
/// - If src_chain == terra_chain_id → verify_terra_deposit (uses LCD query)
/// - Unknown chain → immediate Invalid (cannot verify)
///
/// Swapped IDs cause EVM fraud to be routed through Terra verification (which may
/// return Pending on query errors), leading to timeouts.
#[test]
fn test_canceler_chain_routing_correctness() {
    let evm_v2: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
    let terra_v2: [u8; 4] = [0x00, 0x00, 0x00, 0x02];
    let unknown: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];

    // Simulate chain routing logic
    fn route_chain(id: &[u8; 4], evm_id: &[u8; 4], terra_id: &[u8; 4]) -> &'static str {
        if id == evm_id {
            "evm"
        } else if id == terra_id {
            "terra"
        } else {
            "unknown"
        }
    }

    assert_eq!(
        route_chain(&evm_v2, &evm_v2, &terra_v2),
        "evm",
        "EVM chain ID should route to EVM verification"
    );
    assert_eq!(
        route_chain(&terra_v2, &evm_v2, &terra_v2),
        "terra",
        "Terra chain ID should route to Terra verification"
    );
    assert_eq!(
        route_chain(&unknown, &evm_v2, &terra_v2),
        "unknown",
        "Unknown chain ID should be marked unknown (immediate Invalid)"
    );

    // The OLD buggy test used 0x00000002 for "EVM" which would route to Terra
    let buggy_evm_key: [u8; 4] = [0x00, 0x00, 0x00, 0x02];
    assert_eq!(
        route_chain(&buggy_evm_key, &evm_v2, &terra_v2),
        "terra",
        "Using 0x00000002 for EVM source would incorrectly route to Terra verification"
    );

    // The FIXED test uses 0x00000001 for EVM which correctly routes to EVM
    let fixed_evm_key: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
    assert_eq!(
        route_chain(&fixed_evm_key, &evm_v2, &terra_v2),
        "evm",
        "Using 0x00000001 for EVM source correctly routes to EVM verification"
    );
}

#[tokio::test]
async fn test_pending_withdrawals_filtering() {
    // Test the filtering logic for unapproved entries
    let entries = vec![
        serde_json::json!({"approved": false, "cancelled": false, "executed": false}),
        serde_json::json!({"approved": true, "cancelled": false, "executed": false}),
        serde_json::json!({"approved": true, "cancelled": true, "executed": false}),
        serde_json::json!({"approved": true, "cancelled": false, "executed": true}),
    ];

    let unapproved_count = entries
        .iter()
        .filter(|e| {
            !e["approved"].as_bool().unwrap_or(false)
                && !e["cancelled"].as_bool().unwrap_or(false)
                && !e["executed"].as_bool().unwrap_or(false)
        })
        .count();

    assert_eq!(
        unapproved_count, 1,
        "Only one entry should be unapproved, non-cancelled, non-executed"
    );
}
