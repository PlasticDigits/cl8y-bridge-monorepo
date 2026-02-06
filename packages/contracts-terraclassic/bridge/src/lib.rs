//! CL8Y Bridge Contract - Cross-Chain Token Bridging for TerraClassic
//!
//! This contract enables bridging tokens between TerraClassic and EVM chains
//! using the watchtower security pattern.
//!
//! # Outgoing Flow (Lock)
//! 1. User locks tokens on TerraClassic by sending them to this contract
//! 2. Operators observe the lock event and submit proof to EVM bridge
//! 3. EVM bridge mints/releases equivalent tokens to user
//!
//! # Incoming Flow (Watchtower Pattern)
//! 1. User deposits tokens on EVM chain
//! 2. Operator calls `ApproveWithdraw` to create pending approval
//! 3. 5-minute delay window for canceler verification
//! 4. After delay, anyone calls `ExecuteWithdraw` to release tokens
//!
//! # Security
//! - Watchtower pattern with approve-delay-execute
//! - Canceler network for fraud prevention
//! - Per-token rate limiting (24h window)
//! - Nonce tracking to prevent replay attacks
//! - Emergency pause functionality

pub mod address_codec;
pub mod contract;
pub mod error;
mod execute;
pub mod fee_manager;
pub mod hash;
pub mod msg;
mod query;
pub mod state;

pub use crate::address_codec::UniversalAddress;
pub use crate::error::ContractError;
pub use crate::fee_manager::{calculate_fee, FeeConfig};
pub use crate::hash::{compute_transfer_hash, keccak256};
