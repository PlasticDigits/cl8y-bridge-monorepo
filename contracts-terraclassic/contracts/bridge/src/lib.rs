//! CL8Y Bridge Contract - Cross-Chain Token Bridging for TerraClassic
//!
//! This contract enables bridging tokens between TerraClassic and EVM chains.
//!
//! # Flow
//! 1. User locks tokens on TerraClassic by sending them to this contract
//! 2. Relayers observe the lock event and submit proof to EVM bridge
//! 3. EVM bridge mints/releases equivalent tokens to user
//!
//! # Reverse Flow
//! 1. User burns/locks tokens on EVM chain
//! 2. Relayers observe and submit proof to this contract
//! 3. This contract releases locked tokens to user on TerraClassic
//!
//! # Security
//! - Multi-signature relayer validation
//! - Nonce tracking to prevent replay attacks
//! - Emergency pause functionality

pub mod contract;
pub mod error;
pub mod msg;
pub mod state;

pub use crate::error::ContractError;
