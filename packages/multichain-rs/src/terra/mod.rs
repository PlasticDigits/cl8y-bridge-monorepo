//! Terra Chain Support Module
//!
//! This module provides Terra Classic-specific functionality for interacting with
//! CosmWasm bridge contracts on Terra Classic (columbus-5, rebel-2, localterra).
//!
//! ## Submodules
//!
//! - `client` - Terra LCD/RPC client wrapper with signing capabilities
//! - `contracts` - Bridge contract message types
//! - `events` - Event attribute parsing from transaction logs
//! - `tokens` - CW20 send/transfer helpers

pub mod client;
pub mod contracts;
pub mod events;
pub mod tokens;

// Re-export commonly used items
pub use client::TerraClient;
pub use contracts::{ExecuteMsg, ExecuteMsgV2, QueryMsg};
