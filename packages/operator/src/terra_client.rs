//! Terra Classic LCD Client for transaction signing and broadcasting
//!
//! This module re-exports the Terra client from multichain-rs.
//! See `multichain-rs::terra::client` for the full implementation.

// Re-export everything from multichain-rs for backwards compatibility
pub use multichain_rs::terra::client::*;
