//! Common types for cross-chain operations
//!
//! This module re-exports common types from multichain-rs and provides
//! operator-specific types like the database-compatible Status enum.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

// Re-export common types from multichain-rs
pub use multichain_rs::types::{ChainId, EvmAddress};

// ============================================================================
// Operator-Specific Types (with sqlx integration)
// ============================================================================

/// Processing status for deposits, approvals, and releases
///
/// This type has sqlx::Type derive for database integration, which is
/// operator-specific. The multichain-rs Status type doesn't include sqlx.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum Status {
    Pending,
    Submitted,
    Confirmed,
    Failed,
    Cancelled,
    Reorged,
}

impl Status {
    /// Get the status as a lowercase string
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Submitted => "submitted",
            Status::Confirmed => "confirmed",
            Status::Failed => "failed",
            Status::Cancelled => "cancelled",
            Status::Reorged => "reorged",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_from_u32() {
        let id = ChainId::from_u32(1);
        assert_eq!(id.to_u32(), 1);
        assert_eq!(id.0, [0, 0, 0, 1]);
    }

    #[test]
    fn test_evm_address_from_hex() {
        let addr = EvmAddress::from_hex("0xdead000000000000000000000000000000000000").unwrap();
        assert_eq!(addr.0[0], 0xde);
        assert_eq!(addr.0[1], 0xad);
    }

    #[test]
    fn test_status_as_str() {
        assert_eq!(Status::Pending.as_str(), "pending");
        assert_eq!(Status::Submitted.as_str(), "submitted");
        assert_eq!(Status::Confirmed.as_str(), "confirmed");
        assert_eq!(Status::Failed.as_str(), "failed");
        assert_eq!(Status::Cancelled.as_str(), "cancelled");
        assert_eq!(Status::Reorged.as_str(), "reorged");
    }

    #[test]
    fn test_status_display() {
        assert_eq!(format!("{}", Status::Pending), "pending");
        assert_eq!(format!("{}", Status::Confirmed), "confirmed");
    }
}
