//! State definitions for the CL8Y Bridge contract

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

/// Contract configuration
#[cw_serde]
pub struct Config {
    /// Admin address for contract management
    pub admin: Addr,
    /// Whether the bridge is currently paused
    pub paused: bool,
    /// Minimum number of relayer signatures required
    pub min_signatures: u32,
    /// Minimum bridge amount (in smallest unit)
    pub min_bridge_amount: Uint128,
    /// Maximum bridge amount per transaction (in smallest unit)
    pub max_bridge_amount: Uint128,
    /// Fee percentage (in basis points, e.g., 30 = 0.3%)
    pub fee_bps: u32,
    /// Fee collector address
    pub fee_collector: Addr,
}

/// Pending admin change proposal
#[cw_serde]
pub struct PendingAdmin {
    /// Proposed new admin address
    pub new_address: Addr,
    /// Block time when the change can be executed
    pub execute_after: Timestamp,
}

/// Supported chain configuration
#[cw_serde]
pub struct ChainConfig {
    /// EVM chain ID
    pub chain_id: u64,
    /// Human-readable chain name
    pub name: String,
    /// Bridge contract address on the EVM chain (as hex string)
    pub bridge_address: String,
    /// Whether this chain is currently enabled
    pub enabled: bool,
}

/// Supported token configuration
#[cw_serde]
pub struct TokenConfig {
    /// Token identifier (denom for native, contract address for CW20)
    pub token: String,
    /// Whether this is a native token
    pub is_native: bool,
    /// Corresponding token address on EVM chain (as hex string)
    pub evm_token_address: String,
    /// Decimals on TerraClassic
    pub terra_decimals: u8,
    /// Decimals on EVM chain
    pub evm_decimals: u8,
    /// Whether this token is currently enabled for bridging
    pub enabled: bool,
}

/// Bridge transaction record
#[cw_serde]
pub struct BridgeTransaction {
    /// Unique nonce for this transaction
    pub nonce: u64,
    /// Sender address on source chain
    pub sender: String,
    /// Recipient address on destination chain
    pub recipient: String,
    /// Token being bridged
    pub token: String,
    /// Amount being bridged
    pub amount: Uint128,
    /// Destination chain ID
    pub dest_chain_id: u64,
    /// Transaction timestamp
    pub timestamp: Timestamp,
    /// Whether this is an outgoing (lock) or incoming (release) transaction
    pub is_outgoing: bool,
}

/// Bridge statistics
#[cw_serde]
pub struct Stats {
    /// Total number of outgoing (lock) transactions
    pub total_outgoing_txs: u64,
    /// Total number of incoming (release) transactions
    pub total_incoming_txs: u64,
    /// Total fees collected (in native token)
    pub total_fees_collected: Uint128,
}

/// Contract name for cw2 migration info
pub const CONTRACT_NAME: &str = "crates.io:cl8y-bridge";
/// Contract version for cw2 migration info
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 7 days in seconds for admin change timelock
pub const ADMIN_TIMELOCK_DURATION: u64 = 604_800;

/// Primary config storage
pub const CONFIG: Item<Config> = Item::new("config");

/// Pending admin proposal (if any)
pub const PENDING_ADMIN: Item<PendingAdmin> = Item::new("pending_admin");

/// Bridge statistics
pub const STATS: Item<Stats> = Item::new("stats");

/// Registered relayer addresses
/// Key: relayer address, Value: whether active
pub const RELAYERS: Map<&Addr, bool> = Map::new("relayers");

/// Number of active relayers
pub const RELAYER_COUNT: Item<u32> = Item::new("relayer_count");

/// Supported chains configuration
/// Key: chain_id (u64 as string), Value: ChainConfig
pub const CHAINS: Map<String, ChainConfig> = Map::new("chains");

/// Supported tokens configuration
/// Key: token identifier, Value: TokenConfig
pub const TOKENS: Map<String, TokenConfig> = Map::new("tokens");

/// Outgoing nonce counter (for lock transactions)
pub const OUTGOING_NONCE: Item<u64> = Item::new("outgoing_nonce");

/// Used incoming nonces (to prevent replay attacks)
/// Key: nonce, Value: whether used
pub const USED_NONCES: Map<u64, bool> = Map::new("used_nonces");

/// Bridge transaction history
/// Key: nonce, Value: BridgeTransaction
pub const TRANSACTIONS: Map<u64, BridgeTransaction> = Map::new("transactions");

/// Token balances locked in the bridge
/// Key: token identifier, Value: locked amount
pub const LOCKED_BALANCES: Map<String, Uint128> = Map::new("locked_balances");
