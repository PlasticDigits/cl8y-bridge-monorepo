//! Message types for the CL8Y Bridge contract
//!
//! This module defines all messages for instantiation, execution, and queries,
//! including the watchtower pattern messages (v2.0).

use common::AssetInfo;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, Timestamp, Uint128};

// ============================================================================
// Instantiate & Migrate
// ============================================================================

/// Migrate message
#[cw_serde]
pub struct MigrateMsg {}

/// Instantiate message
#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address for contract management
    pub admin: String,
    /// Initial operator addresses
    pub operators: Vec<String>,
    /// Minimum number of operator signatures required (legacy compatibility)
    pub min_signatures: u32,
    /// Minimum bridge amount (in smallest unit)
    pub min_bridge_amount: Uint128,
    /// Maximum bridge amount per transaction
    pub max_bridge_amount: Uint128,
    /// Fee percentage in basis points (e.g., 30 = 0.3%)
    pub fee_bps: u32,
    /// Fee collector address
    pub fee_collector: String,
}

// ============================================================================
// Execute Messages
// ============================================================================

/// Execute messages
#[cw_serde]
pub enum ExecuteMsg {
    // ========================================================================
    // Outgoing Transfers (Lock)
    // ========================================================================
    /// Lock tokens for bridging to an EVM chain
    /// User sends native tokens with this message
    Lock {
        /// Destination chain ID
        dest_chain_id: u64,
        /// Recipient address on destination chain (EVM address as hex string)
        recipient: String,
    },

    /// Lock CW20 tokens for bridging (called via CW20 send)
    /// Implements CW20 Receiver interface
    Receive(cw20::Cw20ReceiveMsg),

    // ========================================================================
    // Watchtower Pattern (Incoming Transfers)
    // ========================================================================
    /// Approve a withdrawal (creates pending approval)
    ///
    /// Authorization: Operator only
    ///
    /// This creates a pending approval that cannot be executed until
    /// `withdraw_delay` seconds have passed. During this window, cancelers
    /// can verify the approval against the source chain and cancel if invalid.
    ApproveWithdraw {
        /// Source chain key (32 bytes, from ChainRegistry)
        src_chain_key: Binary,
        /// Token to withdraw (denom for native, contract for CW20)
        token: String,
        /// Recipient address on Terra Classic
        recipient: String,
        /// Destination account (32 bytes, for hash verification)
        dest_account: Binary,
        /// Amount to withdraw
        amount: Uint128,
        /// Nonce from source chain deposit
        nonce: u64,
        /// Fee amount (in uluna)
        fee: Uint128,
        /// Fee recipient address
        fee_recipient: String,
        /// If true, fee is deducted from withdrawal amount
        deduct_from_amount: bool,
    },

    /// Execute a withdrawal after delay has elapsed
    ///
    /// Authorization: Anyone (typically the recipient)
    ///
    /// The approval must:
    /// - Exist and be approved
    /// - Not be cancelled
    /// - Not be already executed
    /// - Have delay period elapsed
    ExecuteWithdraw {
        /// The 32-byte transferId hash
        withdraw_hash: Binary,
    },

    /// Cancel a pending withdrawal approval
    ///
    /// Authorization: Canceler or Admin
    ///
    /// Cancelers call this when they detect a fraudulent approval
    /// (e.g., no matching deposit on source chain, or parameter mismatch).
    CancelWithdrawApproval {
        /// The 32-byte transferId hash to cancel
        withdraw_hash: Binary,
    },

    /// Re-enable a cancelled approval (for reorg recovery)
    ///
    /// Authorization: Admin only
    ///
    /// If a legitimate approval was cancelled (e.g., due to source chain
    /// reorg that temporarily hid the deposit), admin can restore it.
    /// The delay timer resets when reenabled.
    ReenableWithdrawApproval {
        /// The 32-byte transferId hash to reenable
        withdraw_hash: Binary,
    },

    // ========================================================================
    // Canceler Management
    // ========================================================================
    /// Add a canceler address
    ///
    /// Authorization: Admin only
    AddCanceler {
        /// Address to grant canceler role
        address: String,
    },

    /// Remove a canceler address
    ///
    /// Authorization: Admin only
    RemoveCanceler {
        /// Address to revoke canceler role
        address: String,
    },

    // ========================================================================
    // Configuration
    // ========================================================================
    /// Set the global withdrawal delay
    ///
    /// Authorization: Admin only
    SetWithdrawDelay {
        /// New delay in seconds (minimum: 60, maximum: 86400)
        delay_seconds: u64,
    },

    /// Set rate limit for a token
    ///
    /// Authorization: Admin only
    SetRateLimit {
        /// Token to configure
        token: String,
        /// Maximum per single transaction (0 = unlimited)
        max_per_transaction: Uint128,
        /// Maximum per 24-hour period (0 = unlimited)
        max_per_period: Uint128,
    },

    // ========================================================================
    // Chain & Token Management
    // ========================================================================
    /// Add a new supported chain
    AddChain {
        chain_id: u64,
        name: String,
        bridge_address: String,
    },

    /// Update chain configuration
    UpdateChain {
        chain_id: u64,
        name: Option<String>,
        bridge_address: Option<String>,
        enabled: Option<bool>,
    },

    /// Add a new supported token
    AddToken {
        token: String,
        is_native: bool,
        evm_token_address: String,
        terra_decimals: u8,
        evm_decimals: u8,
    },

    /// Update token configuration
    UpdateToken {
        token: String,
        evm_token_address: Option<String>,
        enabled: Option<bool>,
    },

    // ========================================================================
    // Operator Management
    // ========================================================================
    /// Register a new operator
    AddOperator { operator: String },

    /// Remove an operator
    RemoveOperator { operator: String },

    /// Update minimum signatures required (legacy compatibility)
    UpdateMinSignatures { min_signatures: u32 },

    // ========================================================================
    // Bridge Configuration
    // ========================================================================
    /// Update bridge limits
    UpdateLimits {
        min_bridge_amount: Option<Uint128>,
        max_bridge_amount: Option<Uint128>,
    },

    /// Update fee configuration
    UpdateFees {
        fee_bps: Option<u32>,
        fee_collector: Option<String>,
    },

    // ========================================================================
    // Admin Operations
    // ========================================================================
    /// Pause the bridge (admin only)
    Pause {},

    /// Unpause the bridge (admin only)
    Unpause {},

    /// Initiate 7-day timelock for admin transfer
    ProposeAdmin { new_admin: String },

    /// Complete admin transfer after timelock
    AcceptAdmin {},

    /// Cancel pending admin change
    CancelAdminProposal {},

    /// Recover stuck assets (admin only, only when paused)
    RecoverAsset {
        asset: AssetInfo,
        amount: Uint128,
        recipient: String,
    },
}

/// CW20 receive hook message (for locking CW20 tokens)
#[cw_serde]
pub enum ReceiveMsg {
    /// Lock CW20 tokens for bridging
    Lock {
        dest_chain_id: u64,
        recipient: String,
    },
}

// ============================================================================
// Query Messages
// ============================================================================

/// Query messages
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // ========================================================================
    // Core Queries
    // ========================================================================
    /// Returns contract configuration
    #[returns(ConfigResponse)]
    Config {},

    /// Returns bridge status
    #[returns(StatusResponse)]
    Status {},

    /// Returns bridge statistics
    #[returns(StatsResponse)]
    Stats {},

    /// Returns information about a supported chain
    #[returns(ChainResponse)]
    Chain { chain_id: u64 },

    /// Returns all supported chains
    #[returns(ChainsResponse)]
    Chains {
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    /// Returns information about a supported token
    #[returns(TokenResponse)]
    Token { token: String },

    /// Returns all supported tokens
    #[returns(TokensResponse)]
    Tokens {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Returns list of registered operators
    #[returns(OperatorsResponse)]
    Operators {},

    /// Check if a nonce has been used (legacy)
    #[returns(NonceUsedResponse)]
    NonceUsed { nonce: u64 },

    /// Returns current outgoing nonce
    #[returns(NonceResponse)]
    CurrentNonce {},

    /// Returns a specific transaction
    #[returns(TransactionResponse)]
    Transaction { nonce: u64 },

    /// Returns locked balance for a token
    #[returns(LockedBalanceResponse)]
    LockedBalance { token: String },

    /// Returns pending admin proposal details
    #[returns(Option<PendingAdminResponse>)]
    PendingAdmin {},

    /// Simulate a bridge transaction (calculate fees)
    #[returns(SimulationResponse)]
    SimulateBridge {
        token: String,
        amount: Uint128,
        dest_chain_id: u64,
    },

    // ========================================================================
    // Watchtower Queries
    // ========================================================================
    /// Get withdrawal approval by hash
    #[returns(WithdrawApprovalResponse)]
    WithdrawApproval { withdraw_hash: Binary },

    /// Compute withdraw hash without storing (for verification)
    #[returns(ComputeHashResponse)]
    ComputeWithdrawHash {
        src_chain_key: Binary,
        dest_chain_key: Binary,
        dest_token_address: Binary,
        dest_account: Binary,
        amount: Uint128,
        nonce: u64,
    },

    /// Get deposit info by hash
    #[returns(Option<DepositInfoResponse>)]
    DepositHash { deposit_hash: Binary },

    /// Get deposit info by nonce (convenience lookup)
    #[returns(Option<DepositInfoResponse>)]
    DepositByNonce { nonce: u64 },

    /// Verify deposit hash matches expected parameters
    #[returns(VerifyDepositResponse)]
    VerifyDeposit {
        deposit_hash: Binary,
        dest_chain_key: Binary,
        dest_token_address: Binary,
        dest_account: Binary,
        amount: Uint128,
        nonce: u64,
    },

    // ========================================================================
    // Canceler Queries
    // ========================================================================
    /// List all active cancelers
    #[returns(CancelersResponse)]
    Cancelers {},

    /// Check if an address is a canceler
    #[returns(IsCancelerResponse)]
    IsCanceler { address: String },

    // ========================================================================
    // Configuration Queries
    // ========================================================================
    /// Get current withdraw delay
    #[returns(WithdrawDelayResponse)]
    WithdrawDelay {},

    /// Get rate limit config for a token
    #[returns(Option<RateLimitResponse>)]
    RateLimit { token: String },

    /// Get current period usage for a token
    #[returns(PeriodUsageResponse)]
    PeriodUsage { token: String },
}

// ============================================================================
// Response Types - Core
// ============================================================================

#[cw_serde]
pub struct ConfigResponse {
    pub admin: Addr,
    pub paused: bool,
    pub min_signatures: u32,
    pub min_bridge_amount: Uint128,
    pub max_bridge_amount: Uint128,
    pub fee_bps: u32,
    pub fee_collector: Addr,
}

#[cw_serde]
pub struct StatusResponse {
    pub paused: bool,
    pub active_operators: u32,
    pub supported_chains: u32,
    pub supported_tokens: u32,
}

#[cw_serde]
pub struct StatsResponse {
    pub total_outgoing_txs: u64,
    pub total_incoming_txs: u64,
    pub total_fees_collected: Uint128,
}

#[cw_serde]
pub struct ChainResponse {
    pub chain_id: u64,
    pub name: String,
    pub bridge_address: String,
    pub enabled: bool,
}

#[cw_serde]
pub struct ChainsResponse {
    pub chains: Vec<ChainResponse>,
}

#[cw_serde]
pub struct TokenResponse {
    pub token: String,
    pub is_native: bool,
    pub evm_token_address: String,
    pub terra_decimals: u8,
    pub evm_decimals: u8,
    pub enabled: bool,
}

#[cw_serde]
pub struct TokensResponse {
    pub tokens: Vec<TokenResponse>,
}

#[cw_serde]
pub struct OperatorsResponse {
    pub operators: Vec<Addr>,
    pub min_signatures: u32,
}

#[cw_serde]
pub struct NonceUsedResponse {
    pub nonce: u64,
    pub used: bool,
}

#[cw_serde]
pub struct NonceResponse {
    pub nonce: u64,
}

#[cw_serde]
pub struct TransactionResponse {
    pub nonce: u64,
    pub sender: String,
    pub recipient: String,
    pub token: String,
    pub amount: Uint128,
    pub dest_chain_id: u64,
    pub timestamp: Timestamp,
    pub is_outgoing: bool,
}

#[cw_serde]
pub struct LockedBalanceResponse {
    pub token: String,
    pub amount: Uint128,
}

#[cw_serde]
pub struct PendingAdminResponse {
    pub new_address: Addr,
    pub execute_after: Timestamp,
}

#[cw_serde]
pub struct SimulationResponse {
    pub input_amount: Uint128,
    pub fee_amount: Uint128,
    pub output_amount: Uint128,
    pub fee_bps: u32,
}

// ============================================================================
// Response Types - Watchtower
// ============================================================================

#[cw_serde]
pub struct WithdrawApprovalResponse {
    pub exists: bool,
    pub src_chain_key: Binary,
    pub token: String,
    pub recipient: Addr,
    pub dest_account: Binary,
    pub amount: Uint128,
    pub nonce: u64,
    pub fee: Uint128,
    pub fee_recipient: Addr,
    pub approved_at: Timestamp,
    pub is_approved: bool,
    pub deduct_from_amount: bool,
    pub cancelled: bool,
    pub executed: bool,
    /// Seconds remaining until executable (0 if ready)
    pub delay_remaining: u64,
}

impl Default for WithdrawApprovalResponse {
    fn default() -> Self {
        Self {
            exists: false,
            src_chain_key: Binary::default(),
            token: String::new(),
            recipient: Addr::unchecked(""),
            dest_account: Binary::default(),
            amount: Uint128::zero(),
            nonce: 0,
            fee: Uint128::zero(),
            fee_recipient: Addr::unchecked(""),
            approved_at: Timestamp::from_seconds(0),
            is_approved: false,
            deduct_from_amount: false,
            cancelled: false,
            executed: false,
            delay_remaining: 0,
        }
    }
}

#[cw_serde]
pub struct ComputeHashResponse {
    pub hash: Binary,
}

#[cw_serde]
pub struct DepositInfoResponse {
    pub deposit_hash: Binary,
    pub dest_chain_key: Binary,
    pub dest_token_address: Binary,
    pub dest_account: Binary,
    pub amount: Uint128,
    pub nonce: u64,
    pub deposited_at: Timestamp,
}

#[cw_serde]
pub struct VerifyDepositResponse {
    /// Whether the deposit exists
    pub exists: bool,
    /// Whether the parameters match
    pub matches: bool,
    /// Stored deposit info (if exists)
    pub deposit: Option<DepositInfoResponse>,
}

// ============================================================================
// Response Types - Canceler
// ============================================================================

#[cw_serde]
pub struct CancelersResponse {
    pub cancelers: Vec<Addr>,
}

#[cw_serde]
pub struct IsCancelerResponse {
    pub is_canceler: bool,
}

// ============================================================================
// Response Types - Configuration
// ============================================================================

#[cw_serde]
pub struct WithdrawDelayResponse {
    pub delay_seconds: u64,
}

#[cw_serde]
pub struct RateLimitResponse {
    pub token: String,
    pub max_per_transaction: Uint128,
    pub max_per_period: Uint128,
}

#[cw_serde]
pub struct PeriodUsageResponse {
    pub token: String,
    pub current_period_start: Timestamp,
    pub used_amount: Uint128,
    pub remaining_amount: Uint128,
    pub period_ends_at: Timestamp,
}
