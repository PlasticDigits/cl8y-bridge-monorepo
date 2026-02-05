//! Testing Utilities Module
//!
//! Provides helpers specifically for E2E tests, including user EOA simulation,
//! mock deposit creation, and common test assertions.
//!
//! ## Submodules
//!
//! - `user_eoa` - Simulate user deposits/withdrawals as regular EOAs
//! - `mock_deposits` - Create test deposit scenarios
//! - `assertions` - Common test assertions

pub mod assertions;
pub mod mock_deposits;
pub mod user_eoa;

// Re-export commonly used items
pub use assertions::*;
pub use mock_deposits::*;
pub use user_eoa::*;
