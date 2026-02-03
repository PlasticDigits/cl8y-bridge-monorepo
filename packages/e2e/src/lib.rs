use alloy::primitives::B256;
use std::fmt;
use std::time::Duration;

pub mod config;
pub mod deploy;
pub mod docker;
pub mod evm;
pub mod services;
pub mod setup;
pub mod teardown;
pub mod terra;
pub mod tests;
pub mod utils;

pub use config::E2eConfig;
pub use deploy::{
    deploy_evm_contracts, deploy_test_token, deploy_test_token_simple, get_token_balance,
    mint_test_tokens, EvmDeployment,
};
pub use docker::DockerCompose;
pub use evm::AnvilTimeClient;
pub use services::ServiceManager;
pub use setup::{E2eSetup, SetupResult, SetupStep};
pub use teardown::{E2eTeardown, TeardownOptions, TeardownResult};
pub use terra::TerraClient;
pub use tests::{
    run_all_tests, run_integration_tests, run_quick_tests, test_fraud_detection_full,
    test_real_evm_to_terra_transfer, test_real_terra_to_evm_transfer, IntegrationTestOptions,
};

/// Represents the outcome of a single test
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
    /// Create a new pass result
    pub fn pass(name: impl Into<String>, duration: Duration) -> Self {
        Self::Pass {
            name: name.into(),
            duration,
        }
    }

    /// Create a new fail result
    pub fn fail(name: impl Into<String>, error: impl Into<String>, duration: Duration) -> Self {
        Self::Fail {
            name: name.into(),
            error: error.into(),
            duration,
        }
    }

    /// Create a new skip result
    pub fn skip(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Skip {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Check if the result is a pass
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass { .. })
    }

    /// Check if the result is a fail
    pub fn is_fail(&self) -> bool {
        matches!(self, Self::Fail { .. })
    }

    /// Get the test name
    pub fn name(&self) -> &str {
        match self {
            Self::Pass { name, .. } => name,
            Self::Fail { name, .. } => name,
            Self::Skip { name, .. } => name,
        }
    }
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass { name, duration } => {
                write!(
                    f,
                    "\x1b[32mPASS\x1b[0m: {} ({:.2}ms)",
                    name,
                    duration.as_millis()
                )
            }
            Self::Fail {
                name,
                error,
                duration,
            } => {
                write!(
                    f,
                    "\x1b[31mFAIL\x1b[0m: {} - {}\n    ({:.2}ms)",
                    name,
                    error,
                    duration.as_millis()
                )
            }
            Self::Skip { name, reason } => {
                write!(f, "\x1b[33mSKIP\x1b[0m: {} - {}", name, reason)
            }
        }
    }
}

/// Aggregates test results and provides summary
#[derive(Debug, Clone)]
pub struct TestSuite {
    name: String,
    results: Vec<TestResult>,
    start_time: std::time::Instant,
}

impl TestSuite {
    /// Create a new test suite
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            results: Vec::new(),
            start_time: std::time::Instant::now(),
        }
    }

    /// Add a test result to the suite
    pub fn add_result(&mut self, result: TestResult) {
        self.results.push(result);
    }

    /// Get the number of passed tests
    pub fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.is_pass()).count()
    }

    /// Get the number of failed tests
    pub fn failed(&self) -> usize {
        self.results.iter().filter(|r| r.is_fail()).count()
    }

    /// Get the number of skipped tests
    pub fn skipped(&self) -> usize {
        self.results
            .iter()
            .filter(|r| !r.is_pass() && !r.is_fail())
            .count()
    }

    /// Get the total number of tests
    pub fn total(&self) -> usize {
        self.results.len()
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.is_pass())
    }

    /// Get the elapsed time for the test suite
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Print a summary of the test results
    pub fn print_summary(&self) {
        let elapsed = self.elapsed();
        let passed = self.passed();
        let failed = self.failed();
        let skipped = self.skipped();
        let total = self.total();

        println!();
        println!("Test Suite: {}", self.name);
        println!("----------------------------------------");
        println!("Total:   {}", total);
        println!("Passed:  \x1b[32m{}\x1b[0m", passed);
        println!("Failed:  \x1b[31m{}\x1b[0m", failed);
        println!("Skipped: \x1b[33m{}\x1b[0m", skipped);
        println!("Elapsed: {:.2}ms", elapsed.as_millis());
        println!("----------------------------------------");

        if failed > 0 {
            println!("\nFailed tests:");
            for result in &self.results {
                if result.is_fail() {
                    println!("  {}", result);
                }
            }
        }
    }
}

impl fmt::Display for TestSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestSuite({} - {} passed, {} failed)",
            self.name,
            self.passed(),
            self.failed()
        )
    }
}

/// Type-safe wrapper for chain keys (bytes32)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChainKey(pub B256);

impl ChainKey {
    /// Create a ChainKey from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(B256::from_slice(&bytes))
    }

    /// Get the raw bytes representation
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_ref()
    }

    /// Compute chain key for a COSMW chain (matches ChainRegistry.getChainKeyCOSMW)
    pub fn cosmw(chain_id: &str) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = b'c';
        bytes[1] = b'o';
        bytes[2] = b's';
        bytes[3] = b'm';
        bytes[4] = b'w';
        bytes[5] = b'_';

        let chain_id_bytes = chain_id.as_bytes();
        for (i, &byte) in chain_id_bytes.iter().enumerate() {
            bytes[6 + i] = byte;
        }

        Self::from_bytes(bytes)
    }

    /// Compute chain key for an EVM chain (matches ChainRegistry.getChainKeyEVM)
    pub fn evm(chain_id: u64) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = b'e';
        bytes[1] = b'v';
        bytes[2] = b'm';
        bytes[3] = b'_';

        for (i, byte) in chain_id.to_be_bytes().iter().enumerate() {
            bytes[4 + i] = *byte;
        }

        Self::from_bytes(bytes)
    }
}

impl From<B256> for ChainKey {
    fn from(b256: B256) -> Self {
        Self(b256)
    }
}

impl From<[u8; 32]> for ChainKey {
    fn from(bytes: [u8; 32]) -> Self {
        Self::from_bytes(bytes)
    }
}

impl fmt::Display for ChainKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode_upper(self.0))
    }
}

/// Type-safe wrapper for deposit nonces
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DepositNonce(pub u64);

impl DepositNonce {
    /// Create a new DepositNonce
    pub fn new(nonce: u64) -> Self {
        Self(nonce)
    }

    /// Get the next nonce
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl From<u64> for DepositNonce {
    fn from(nonce: u64) -> Self {
        Self::new(nonce)
    }
}

impl fmt::Display for DepositNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
