//! Multichain-RS: Shared Cross-Chain Library for CL8Y Bridge
//!
//! This crate provides shared functionality across the operator, canceler, and E2E test packages:
//!
//! - **Address Encoding/Decoding** - Universal address codec for EVM and Cosmos chains
//! - **Hash Computation** - Deposit/withdraw hash computation matching contract logic
//! - **Types** - Shared types like ChainId, UniversalAddress, FeeParams, ChainRegistration
//! - **EVM Module** - EVM client, contract bindings, event parsing, signing, watching
//! - **Terra Module** - Terra client, contract messages, event parsing, signing, queries
//! - **Testing Module** - Helpers for E2E tests (user EOA simulation, assertions)
//!
//! ## Usage
//!
//! ```toml
//! [dependencies]
//! multichain-rs = { path = "../multichain-rs" }
//! ```
//!
//! ## Feature Flags
//!
//! - `evm` - Enable EVM chain support (default)
//! - `terra` - Enable Terra chain support (default)
//! - `testing` - Enable testing utilities for E2E tests
//! - `full` - Enable all features

// Core modules (always available)
pub mod address_codec;
pub mod discovery;
pub mod hash;
pub mod redact;
pub mod types;

// Chain-specific modules (feature-gated)
#[cfg(feature = "evm")]
pub mod evm;

#[cfg(feature = "terra")]
pub mod terra;

// Testing utilities (feature-gated)
#[cfg(feature = "testing")]
pub mod testing;

// Re-export commonly used items at the crate root
pub use address_codec::{
    decode_bech32_address, encode_bech32_address, encode_evm_address, parse_evm_address,
    UniversalAddress, CHAIN_TYPE_BITCOIN, CHAIN_TYPE_COSMOS, CHAIN_TYPE_EVM, CHAIN_TYPE_SOLANA,
};

pub use hash::{
    address_to_bytes32, bytes32_to_address, bytes32_to_hex, bytes4_to_hex, compute_deposit_hash,
    compute_transfer_hash, compute_withdraw_hash, keccak256,
};

pub use discovery::{additional_chains, discover_chains, DiscoveredChain, KnownChain};
pub use types::{
    ChainId, ChainRegistration, EvmAddress, FeeCalculator, FeeParams, OperatorGasConfig, Status,
    TokenDestination, TokenRegistration, TokenType, WithdrawHash,
};
