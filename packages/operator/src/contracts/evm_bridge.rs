//! EVM Bridge contract ABI definitions (V2)
//!
//! This module re-exports EVM contract bindings from multichain-rs.
//! See `multichain-rs::evm::contracts` for the full implementation.
//!
//! ## V2 Changes (Phase 4)
//! - Uses 4-byte chain IDs (`bytes4`) instead of 32-byte chain keys
//! - User-initiated withdrawal flow with operator approval
//! - New event signatures

// Re-export everything from multichain-rs for backwards compatibility
pub use multichain_rs::evm::contracts::*;
