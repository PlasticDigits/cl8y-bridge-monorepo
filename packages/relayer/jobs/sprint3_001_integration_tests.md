---
context_files:
  - src/types.rs
  - src/config.rs
output_dir: tests/
output_file: integration_test.rs
---

# Integration Test Suite for Cross-Chain Transfers

Create integration tests for the CL8Y Bridge relayer that test end-to-end transfer flows.

## Requirements

Create a Rust integration test file that tests cross-chain transfers between Terra Classic and EVM chains.

### File Structure

The tests should be in `packages/relayer/tests/integration_test.rs`.

### Test Configuration

```rust
//! Integration tests for cross-chain transfers
//! 
//! Run with: cargo test --test integration_test -- --nocapture
//! 
//! Prerequisites:
//! - Anvil running on localhost:8545
//! - LocalTerra running on localhost:26657
//! - Contracts deployed and configured
//! - DATABASE_URL set
```

### Tests to Implement

1. **test_environment_setup** - Verify test environment is properly configured
   - Check EVM RPC connectivity
   - Check Terra RPC connectivity
   - Check database connectivity
   - Should be `#[ignore]` - run with `--ignored`

2. **test_terra_to_evm_transfer** - Test Terra → EVM transfer flow
   - Lock tokens on Terra via LCD API (mock or real call)
   - Verify lock event is stored in database
   - Verify approval would be created for EVM
   - Should be `#[ignore]` - run with `--ignored`

3. **test_evm_to_terra_transfer** - Test EVM → Terra transfer flow
   - Deposit tokens on EVM (mock or real call)
   - Verify deposit event is stored in database  
   - Verify release would be created for Terra
   - Should be `#[ignore]` - run with `--ignored`

4. **test_chain_key_computation** - Test chain key computations
   - Verify Terra chain key matches expected value
   - Verify EVM chain key matches expected value
   - This test should NOT be ignored

5. **test_address_encoding** - Test address encoding/decoding
   - Convert Terra address to bytes32
   - Convert EVM address to string format
   - This test should NOT be ignored

### Helper Modules

Create helper modules/functions:

```rust
mod helpers {
    // Environment configuration from env vars
    pub struct TestConfig {
        pub evm_rpc_url: String,
        pub terra_rpc_url: String,
        pub terra_lcd_url: String,
        pub database_url: String,
        pub evm_bridge_address: String,
        pub terra_bridge_address: String,
    }
    
    impl TestConfig {
        pub fn from_env() -> Option<Self> { ... }
    }
    
    // Check connectivity helpers
    pub async fn check_evm_connectivity(rpc_url: &str) -> bool { ... }
    pub async fn check_terra_connectivity(rpc_url: &str) -> bool { ... }
}
```

### Chain Key Constants

Use these expected values for testing:

```rust
// Terra Classic chain key: keccak256(abi.encode("COSMOS", "localterra", "terra"))
// EVM Anvil chain key: keccak256(abi.encode("EVM", 31337))
```

### Dependencies

The test file should use these crates (already in Cargo.toml):
- `tokio` with `test` feature for async tests
- `reqwest` for HTTP calls
- `serde_json` for JSON handling
- `alloy-primitives` for keccak256 and Address types

### Test Attributes

- Use `#[tokio::test]` for async tests
- Use `#[ignore]` for tests requiring running infrastructure
- Group related tests with clear documentation

### Output

Generate approximately 150 lines of well-structured Rust test code with:
- Clear documentation
- Proper error handling
- Helper functions to reduce duplication
- Both unit-style tests (chain key, address encoding) and integration tests (transfer flows)
