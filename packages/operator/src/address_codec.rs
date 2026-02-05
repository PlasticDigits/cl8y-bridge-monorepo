//! Universal Cross-Chain Address Encoding
//!
//! This module re-exports address encoding/decoding from multichain-rs.
//! See `multichain-rs::address_codec` for the full implementation.
//!
//! ## Address Format
//!
//! All addresses are stored as 32 bytes with format:
//! ```text
//! | Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
//! ```
//!
//! ## Chain Type Codes
//!
//! - `0x00000001`: EVM (Ethereum, BSC, Polygon, etc.)
//! - `0x00000002`: Cosmos/Terra (Terra Classic, Osmosis)
//! - `0x00000003`: Solana (future)
//! - `0x00000004`: Bitcoin (future)

// Re-export everything from multichain-rs for backwards compatibility
pub use multichain_rs::address_codec::*;
