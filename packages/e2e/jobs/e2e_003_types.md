---
output_dir: src/
output_file: lib.rs
verify: true
---

# Library Root and Core Types

## Requirements

Create the library root that exports all modules and defines core types for the E2E test framework.

## Module Declarations

```rust
pub mod config;
// These will be added in later batches:
// pub mod contracts;
// pub mod docker;
// pub mod setup;
// pub mod tests;
// pub mod utils;

pub use config::E2eConfig;
```

## Core Types

### TestResult

Represents the outcome of a single test:

```rust
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum TestResult {
    Pass {
        name: String,
        duration: Duration,
    },
    Fail {
        name: String,
        error: String,
        duration: Duration,
    },
    Skip {
        name: String,
        reason: String,
    },
}

impl TestResult {
    pub fn pass(name: impl Into<String>, duration: Duration) -> Self;
    pub fn fail(name: impl Into<String>, error: impl Into<String>, duration: Duration) -> Self;
    pub fn skip(name: impl Into<String>, reason: impl Into<String>) -> Self;
    
    pub fn is_pass(&self) -> bool;
    pub fn is_fail(&self) -> bool;
    pub fn name(&self) -> &str;
}
```

### TestSuite

Aggregates test results and provides summary:

```rust
pub struct TestSuite {
    name: String,
    results: Vec<TestResult>,
    start_time: std::time::Instant,
}

impl TestSuite {
    pub fn new(name: impl Into<String>) -> Self;
    pub fn add_result(&mut self, result: TestResult);
    pub fn passed(&self) -> usize;
    pub fn failed(&self) -> usize;
    pub fn skipped(&self) -> usize;
    pub fn total(&self) -> usize;
    pub fn all_passed(&self) -> bool;
    pub fn elapsed(&self) -> Duration;
    pub fn print_summary(&self);
}
```

### ChainKey

Type-safe wrapper for chain keys (bytes32):

```rust
use alloy::primitives::B256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChainKey(pub B256);

impl ChainKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self;
    pub fn as_bytes(&self) -> &[u8; 32];
    
    /// Compute chain key for a COSMW chain (matches ChainRegistry.getChainKeyCOSMW)
    pub fn cosmw(chain_id: &str) -> Self;
    
    /// Compute chain key for an EVM chain (matches ChainRegistry.getChainKeyEVM)
    pub fn evm(chain_id: u64) -> Self;
}
```

### DepositNonce

Type-safe wrapper for deposit nonces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DepositNonce(pub u64);

impl DepositNonce {
    pub fn new(nonce: u64) -> Self;
    pub fn next(&self) -> Self;
}
```

## Display Implementations

Implement `std::fmt::Display` for `TestResult` and `TestSuite` to enable nice terminal output with colors.

Use ANSI color codes:
- Green (`\x1b[32m`) for PASS
- Red (`\x1b[31m`) for FAIL
- Yellow (`\x1b[33m`) for SKIP
- Reset (`\x1b[0m`) after colors

## Constraints

- All types must derive `Debug` and `Clone`
- Use `#[derive(Copy)]` where appropriate (small value types)
- No external dependencies in this file except `alloy::primitives`
- Implement `From` traits for easy conversion between types
- Add doc comments (`///`) for all public items
