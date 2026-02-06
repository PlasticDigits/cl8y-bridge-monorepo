//! EVM Chain Support Module
//!
//! This module provides EVM-specific functionality for interacting with
//! Bridge contracts on EVM-compatible chains (Ethereum, BSC, Polygon, etc.)
//!
//! ## Submodules
//!
//! - `client` - EVM RPC client wrapper (read-only and with signer)
//! - `contracts` - Bridge contract bindings using alloy sol! macro (Bridge, ChainRegistry, TokenRegistry, LockUnlock, MintBurn, ERC20)
//! - `events` - Event parsing for deposits, withdrawals, etc.
//! - `queries` - Typed query helpers for bridge, registry, and balance queries
//! - `signer` - Dedicated transaction signing with nonce/gas management
//! - `tokens` - ERC20 approve/transfer/balance helpers
//! - `watcher` - Event subscription and polling helpers

pub mod client;
pub mod contracts;
pub mod events;
pub mod queries;
pub mod signer;
pub mod tokens;
pub mod watcher;

// Re-export commonly used items
pub use client::{EvmClient, EvmClientConfig, EvmClientReadOnly, EvmClientWithSigner};
pub use contracts::{Bridge, ChainRegistry, IMintable, LockUnlock, MintBurn, TokenRegistry, ERC20};
pub use events::{
    DepositEvent, WithdrawApproveEvent, WithdrawCancelEvent, WithdrawExecuteEvent,
    WithdrawSubmitEvent,
};
pub use queries::EvmQueryClient;
pub use signer::{EvmSigner, EvmSignerConfig, RetryConfig};
pub use watcher::{BridgeEvent, EvmEventWatcher, WatcherConfig};
