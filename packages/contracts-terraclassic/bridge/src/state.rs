//! State definitions for the CL8Y Bridge contract
//!
//! This module defines all storage structures and state maps for the bridge,
//! including the watchtower security pattern state.

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

// ============================================================================
// Core Configuration
// ============================================================================

/// Contract configuration
#[cw_serde]
pub struct Config {
    /// Admin address for contract management
    pub admin: Addr,
    /// Whether the bridge is currently paused
    pub paused: bool,
    /// Minimum number of operator signatures required (legacy, kept for compatibility)
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
    /// 4-byte registered chain ID (auto-incremented on registration)
    pub chain_id: [u8; 4],
    /// Human-readable identifier (e.g., "evm_1", "terraclassic_columbus-5")
    pub identifier: String,
    /// keccak256(identifier) — for cross-chain verification
    pub identifier_hash: [u8; 32],
    /// Whether this chain is currently enabled
    pub enabled: bool,
}

/// Token type for bridge operations
#[cw_serde]
#[derive(Default)]
pub enum TokenType {
    /// Lock tokens in bridge on deposit, unlock on withdraw (for existing tokens)
    #[default]
    LockUnlock,
    /// Burn tokens on deposit, mint on withdraw (for bridge-native wrapped tokens)
    MintBurn,
}

impl TokenType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenType::LockUnlock => "lock_unlock",
            TokenType::MintBurn => "mint_burn",
        }
    }
}

/// Supported token configuration
#[cw_serde]
pub struct TokenConfig {
    /// Token identifier (denom for native, contract address for CW20)
    pub token: String,
    /// Whether this is a native token
    pub is_native: bool,
    /// Token type (LockUnlock or MintBurn)
    pub token_type: TokenType,
    /// Corresponding token address on EVM chain (as hex string)
    pub evm_token_address: String,
    /// Decimals on TerraClassic
    pub terra_decimals: u8,
    /// Decimals on EVM chain
    pub evm_decimals: u8,
    /// Whether this token is currently enabled for bridging
    pub enabled: bool,
}

/// Destination chain token mapping
#[cw_serde]
pub struct TokenDestMapping {
    /// Token address on destination chain (32 bytes)
    pub dest_token: [u8; 32],
    /// Token decimals on destination chain
    pub dest_decimals: u8,
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
    /// Destination chain (4-byte registered chain ID)
    pub dest_chain: [u8; 4],
    /// Transaction timestamp
    pub timestamp: Timestamp,
    /// Whether this is an outgoing (lock) or incoming (withdraw) transaction
    pub is_outgoing: bool,
}

/// Bridge statistics
#[cw_serde]
pub struct Stats {
    /// Total number of outgoing (lock) transactions
    pub total_outgoing_txs: u64,
    /// Total number of incoming (withdraw) transactions
    pub total_incoming_txs: u64,
    /// Total fees collected (in native token)
    pub total_fees_collected: Uint128,
}

// ============================================================================
// Watchtower Pattern Structures (v2.0)
// ============================================================================

/// Withdrawal approval tracking (keyed by transferId hash)
///
/// V2 pending withdrawal record. Created by user via `WithdrawSubmit`,
/// approved by operator via `WithdrawApprove`, then executed after cancel window.
#[cw_serde]
pub struct PendingWithdraw {
    /// Source chain ID (4 bytes)
    pub src_chain: [u8; 4],
    /// Source account (depositor on source chain, 32 bytes)
    pub src_account: [u8; 32],
    /// Destination account (recipient on this chain, 32 bytes)
    pub dest_account: [u8; 32],
    /// Token identifier on this chain (denom or CW20 contract address)
    pub token: String,
    /// Decoded recipient address on this chain
    pub recipient: Addr,
    /// Amount to withdraw (in source chain decimals; converted at execution time)
    pub amount: Uint128,
    /// Nonce from source chain deposit
    pub nonce: u64,
    /// Source chain token decimals (e.g. 18 for EVM ERC20)
    pub src_decimals: u8,
    /// Destination chain token decimals (e.g. 6 for Terra native)
    pub dest_decimals: u8,
    /// Operator gas tip (native tokens sent with WithdrawSubmit)
    pub operator_gas: Uint128,
    /// Block timestamp when submitted by user
    pub submitted_at: u64,
    /// Block timestamp when approved by operator (0 if not yet approved)
    pub approved_at: u64,
    /// Whether operator has approved
    pub approved: bool,
    /// Whether cancelled by canceler
    pub cancelled: bool,
    /// Whether executed (tokens released)
    pub executed: bool,
}

/// Deposit info for outgoing transfers (enables bidirectional verification)
///
/// When a user locks tokens, we compute and store the deposit hash so that
/// the destination chain can verify the deposit exists.
#[cw_serde]
pub struct DepositInfo {
    /// Source chain ID (4 bytes)
    pub src_chain: [u8; 4],
    /// Destination chain ID (4 bytes)
    pub dest_chain: [u8; 4],
    /// Source account (depositor) encoded as 32 bytes
    pub src_account: [u8; 32],
    /// Destination account (32 bytes)
    pub dest_account: [u8; 32],
    /// Token address on destination chain (32 bytes)
    pub dest_token_address: [u8; 32],
    /// Deposit amount (normalized to destination decimals)
    pub amount: Uint128,
    /// Unique nonce for this deposit
    pub nonce: u64,
    /// Block timestamp when deposit was made
    pub deposited_at: Timestamp,
    /// Legacy field: destination chain key (32 bytes) - kept for backward compat
    pub dest_chain_key: [u8; 32],
}

/// Rate limit configuration for a token
///
/// Implements a fixed 24-hour window rate limiting to match EVM parity.
#[cw_serde]
pub struct RateLimitConfig {
    /// Maximum amount per single transaction (0 = unlimited)
    pub max_per_transaction: Uint128,
    /// Maximum total amount per 24-hour period (0 = unlimited)
    pub max_per_period: Uint128,
}

/// Rate limit window tracking for a token
#[cw_serde]
pub struct RateLimitWindow {
    /// Timestamp when the current window started
    pub window_start: Timestamp,
    /// Amount used in the current window
    pub used: Uint128,
}

// ============================================================================
// Constants
// ============================================================================

/// Contract name for cw2 migration info
pub const CONTRACT_NAME: &str = "crates.io:cl8y-bridge";

/// Contract version for cw2 migration info (v2.0.0 for watchtower pattern)
pub const CONTRACT_VERSION: &str = "2.0.0";

/// 7 days in seconds for admin change timelock
pub const ADMIN_TIMELOCK_DURATION: u64 = 604_800;

/// Default withdrawal delay in seconds (5 minutes)
pub const DEFAULT_WITHDRAW_DELAY: u64 = 300;

/// Rate limit period in seconds (24 hours, matching EVM)
pub const RATE_LIMIT_PERIOD: u64 = 86_400;

// ============================================================================
// Core State Storage
// ============================================================================

/// Primary config storage
pub const CONFIG: Item<Config> = Item::new("config");

/// Pending admin proposal (if any)
pub const PENDING_ADMIN: Item<PendingAdmin> = Item::new("pending_admin");

/// Bridge statistics
pub const STATS: Item<Stats> = Item::new("stats");

/// Supported chains configuration
/// V2 chain registry — keyed by 4-byte chain ID
/// Key: 4-byte chain ID as &[u8], Value: ChainConfig
pub const CHAINS: Map<&[u8], ChainConfig> = Map::new("chains_v2");

/// Reverse lookup: identifier string → 4-byte chain ID
pub const CHAIN_BY_IDENTIFIER: Map<&str, [u8; 4]> = Map::new("chain_by_ident");

/// Supported tokens configuration
/// Key: token identifier, Value: TokenConfig
pub const TOKENS: Map<String, TokenConfig> = Map::new("tokens");

/// Outgoing nonce counter (for lock transactions)
pub const OUTGOING_NONCE: Item<u64> = Item::new("outgoing_nonce");

/// Used incoming nonces (legacy - kept for compatibility, not used in v2.0)
/// Key: nonce, Value: whether used
pub const USED_NONCES: Map<u64, bool> = Map::new("used_nonces");

/// Bridge transaction history
/// Key: nonce, Value: BridgeTransaction
pub const TRANSACTIONS: Map<u64, BridgeTransaction> = Map::new("transactions");

/// Token balances locked in the bridge
/// Key: token identifier, Value: locked amount
pub const LOCKED_BALANCES: Map<String, Uint128> = Map::new("locked_balances");

// ============================================================================
// Operator Management (renamed from RELAYERS)
// ============================================================================

/// Registered operator addresses (renamed from RELAYERS)
/// Key: operator address, Value: whether active
pub const OPERATORS: Map<&Addr, bool> = Map::new("operators");

/// Number of active operators (renamed from RELAYER_COUNT)
pub const OPERATOR_COUNT: Item<u32> = Item::new("operator_count");

// ============================================================================
// Watchtower Pattern State (v2.0)
// ============================================================================

/// This chain's predetermined 4-byte V2 chain ID (set during instantiation, matches EVM ChainRegistry)
pub const THIS_CHAIN_ID: Item<[u8; 4]> = Item::new("this_chain_id");

/// Global withdrawal delay in seconds (default: 300 = 5 minutes)
pub const WITHDRAW_DELAY: Item<u64> = Item::new("withdraw_delay");

/// Withdrawal approvals indexed by transferId hash
/// V2 pending withdrawals — user-initiated
/// Key: 32-byte withdraw hash as &[u8], Value: PendingWithdraw
pub const PENDING_WITHDRAWS: Map<&[u8], PendingWithdraw> = Map::new("pending_withdraws");

/// Tracks nonce usage per source chain to prevent duplicates
/// Key: (src_chain_key as &[u8], nonce), Value: bool (true if used)
pub const WITHDRAW_NONCE_USED: Map<(&[u8], u64), bool> = Map::new("withdraw_nonce_used");

/// Deposit hashes for outgoing transfers (enables verification)
/// Key: 32-byte transferId hash as &[u8], Value: DepositInfo
pub const DEPOSIT_HASHES: Map<&[u8], DepositInfo> = Map::new("deposit_hashes");

/// Deposit info indexed by nonce (for convenience lookups)
/// Key: nonce, Value: 32-byte deposit hash
pub const DEPOSIT_BY_NONCE: Map<u64, [u8; 32]> = Map::new("deposit_by_nonce");

/// Authorized canceler addresses
/// Key: Address reference, Value: bool (true if active canceler)
pub const CANCELERS: Map<&Addr, bool> = Map::new("cancelers");

/// Per-token rate limit configurations
/// Key: token identifier, Value: RateLimitConfig
pub const RATE_LIMITS: Map<&str, RateLimitConfig> = Map::new("rate_limits");

/// Per-token rate limit window tracking
/// Key: token identifier, Value: RateLimitWindow
pub const RATE_WINDOWS: Map<&str, RateLimitWindow> = Map::new("rate_windows");

/// Token destination chain mappings
/// Key: (token_identifier, dest_chain_id as string), Value: TokenDestMapping
pub const TOKEN_DEST_MAPPINGS: Map<(&str, &str), TokenDestMapping> =
    Map::new("token_dest_mappings");
