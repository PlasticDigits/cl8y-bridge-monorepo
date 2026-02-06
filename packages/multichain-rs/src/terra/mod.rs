//! Terra Chain Support Module
//!
//! This module provides Terra Classic-specific functionality for interacting with
//! CosmWasm bridge contracts on Terra Classic (columbus-5, rebel-2, localterra).
//!
//! ## Submodules
//!
//! - `client` - Terra LCD/RPC client wrapper with signing capabilities
//! - `contracts` - Bridge contract message types (V1 and V2)
//! - `events` - Event attribute parsing from transaction logs
//! - `queries` - Query helpers for balances, config, registry
//! - `signer` - Dedicated transaction signing with sequence management
//! - `tokens` - CW20 token helpers
//! - `watcher` - Event subscription and polling helpers

pub mod client;
pub mod contracts;
pub mod events;
pub mod queries;
pub mod signer;
pub mod tokens;
pub mod watcher;

// Re-export commonly used items
pub use client::TerraClient;
pub use contracts::{ExecuteMsg, ExecuteMsgV2, QueryMsg};
pub use events::{
    TerraDepositEvent, TerraWithdrawApproveEvent, TerraWithdrawCancelEvent,
    TerraWithdrawExecuteEvent, TerraWithdrawSubmitEvent, WasmEvent,
};
pub use queries::TerraQueryClient;
pub use signer::{TerraRetryConfig, TerraSigner, TerraSignerConfig};
pub use watcher::{TerraBridgeEvent, TerraEventWatcher, TerraWatcherConfig};
