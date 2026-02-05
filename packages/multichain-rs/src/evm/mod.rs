//! EVM Chain Support Module
//!
//! This module provides EVM-specific functionality for interacting with
//! Bridge contracts on EVM-compatible chains (Ethereum, BSC, Polygon, etc.)
//!
//! ## Submodules
//!
//! - `client` - EVM RPC client wrapper
//! - `contracts` - Bridge contract bindings using alloy sol! macro
//! - `events` - Event parsing for deposits, withdrawals, etc.
//! - `tokens` - ERC20 approve/transfer helpers

pub mod client;
pub mod contracts;
pub mod events;
pub mod tokens;

// Re-export commonly used items
pub use client::{EvmClient, EvmClientConfig, EvmClientReadOnly, EvmClientWithSigner};
pub use contracts::{Bridge, ERC20};
pub use events::{DepositEvent, WithdrawApproveEvent, WithdrawExecuteEvent, WithdrawSubmitEvent};
