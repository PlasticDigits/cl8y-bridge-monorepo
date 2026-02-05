//! Terra Classic bridge contract message definitions
//!
//! This module re-exports Terra contract message types from multichain-rs.
//! See `multichain-rs::terra::contracts` for the full implementation.
//!
//! ## Message Versions
//!
//! - **V1 (Legacy)**: `ExecuteMsg` - Operator submits full withdrawal parameters
//! - **V2 (New)**: `ExecuteMsgV2` - User-initiated withdrawals, operator just approves hash

// Re-export everything from multichain-rs for backwards compatibility
pub use multichain_rs::terra::contracts::*;
