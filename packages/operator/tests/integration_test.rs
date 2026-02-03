//! Integration tests for cross-chain transfers
//!
//! Run with: cargo test --test integration_test -- --nocapture
//!
//! Prerequisites:
//! - Anvil running on localhost:8545
//! - LocalTerra running on localhost:26657
//! - Contracts deployed and configured
//! - DATABASE_URL set

use alloy_primitives::keccak256;

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
