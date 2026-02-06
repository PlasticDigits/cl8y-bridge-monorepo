//! Testing Utilities Module
//!
//! Provides helpers specifically for E2E tests, including user EOA simulation,
//! mock deposit creation, and common test assertions.
//!
//! ## Submodules
//!
//! - `user_eoa` - Simulate EVM user deposits/withdrawals as regular EOAs
//! - `terra_user` - Simulate Terra user deposits/withdrawals as regular EOAs
//! - `mock_deposits` - Create test deposit scenarios
//! - `assertions` - Common test assertions

pub mod assertions;
pub mod mock_deposits;
pub mod terra_user;
pub mod user_eoa;

// Re-export commonly used items
pub use assertions::*;
pub use mock_deposits::*;
pub use terra_user::*;
pub use user_eoa::*;
