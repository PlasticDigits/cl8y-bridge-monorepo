//! Message types for the CL8Y Bridge contract
//!
//! This module defines all messages for instantiation, execution, and queries,
//! including the watchtower pattern messages (v2.0).

use common::AssetInfo;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, Coin, Timestamp, Uint128};

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
    /// This chain's predetermined 4-byte chain ID (Binary of exactly 4 bytes)
    pub this_chain_id: Binary,
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
    /// Deposit native tokens for bridging (locks them on Terra)
    DepositNative {
        /// Destination chain (4-byte registered chain ID)
        dest_chain: Binary,
        /// Destination account on the target chain (32-byte universal address)
        dest_account: Binary,
    },

    /// Deposit CW20 tokens for bridging (called via CW20 send)
    /// Implements CW20 Receiver interface
    Receive(cw20::Cw20ReceiveMsg),

    // ========================================================================
    // V2 Withdrawal Flow (User-Initiated)
    // ========================================================================
    /// Submit a withdrawal request (user-initiated)
    ///
    /// Authorization: Anyone (user pays gas + optional operator tip)
    ///
    /// Creates a `PendingWithdraw` record. The user sends native token (uluna)
    /// as an operator gas tip, which is paid to the operator upon approval.
    WithdrawSubmit {
        /// Source chain ID (4 bytes)
        src_chain: Binary,
        /// Source account (depositor on source chain, 32 bytes)
        src_account: Binary,
        /// Token to withdraw (denom for native, contract for CW20)
        token: String,
        /// Recipient address on this chain (Terra address string)
        recipient: String,
        /// Amount to withdraw
        amount: Uint128,
        /// Nonce from source chain deposit
        nonce: u64,
    },

    /// Approve a pending withdrawal (operator only)
    ///
    /// Authorization: Operator only
    ///
    /// Operator verifies the deposit exists on the source chain and approves.
    /// Starts the cancel window timer. Operator receives the gas tip.
    WithdrawApprove {
        /// The 32-byte withdraw hash
        withdraw_hash: Binary,
    },

    /// Cancel a pending withdrawal (within cancel window)
    ///
    /// Authorization: Canceler only
    ///
    /// Can only cancel after approval and within the cancel window.
    WithdrawCancel {
        /// The 32-byte withdraw hash
        withdraw_hash: Binary,
    },

    /// Uncancel a cancelled withdrawal
    ///
    /// Authorization: Operator only
    ///
    /// Restores a cancelled withdrawal and resets the cancel window timer.
    WithdrawUncancel {
        /// The 32-byte withdraw hash
        withdraw_hash: Binary,
    },

    /// Execute withdrawal by unlocking tokens (LockUnlock mode)
    ///
    /// Authorization: Anyone (after cancel window expires)
    ///
    /// Releases locked tokens to the recipient.
    WithdrawExecuteUnlock {
        /// The 32-byte withdraw hash
        withdraw_hash: Binary,
    },

    /// Execute withdrawal by minting tokens (MintBurn mode)
    ///
    /// Authorization: Anyone (after cancel window expires)
    ///
    /// Mints wrapped tokens to the recipient.
    WithdrawExecuteMint {
        /// The 32-byte withdraw hash
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
    /// Set the global withdrawal delay (cancel window for approved withdrawals).
    /// Valid range: 15 seconds (minimum) to 86400 seconds (24 hours, maximum).
    ///
    /// Authorization: Admin only
    SetWithdrawDelay {
        /// New delay in seconds (15–86400)
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
    /// Register a new chain with a predetermined 4-byte chain ID
    ///
    /// Authorization: Admin only
    RegisterChain {
        /// Human-readable identifier (e.g., "evm_1", "terraclassic_columbus-5")
        identifier: String,
        /// Predetermined 4-byte chain ID (Binary of exactly 4 bytes, must not be 0x00000000)
        chain_id: Binary,
    },

    /// Unregister an existing chain
    ///
    /// Authorization: Admin only
    UnregisterChain {
        /// 4-byte registered chain ID to remove
        chain_id: Binary,
    },

    /// Update chain configuration (enable/disable)
    ///
    /// Authorization: Admin only
    UpdateChain {
        /// 4-byte registered chain ID
        chain_id: Binary,
        enabled: Option<bool>,
    },

    /// Add a new supported token
    AddToken {
        token: String,
        is_native: bool,
        /// Token type: "lock_unlock" (default) or "mint_burn"
        token_type: Option<String>,
        evm_token_address: String,
        terra_decimals: u8,
        evm_decimals: u8,
    },

    /// Update token configuration
    UpdateToken {
        token: String,
        evm_token_address: Option<String>,
        enabled: Option<bool>,
        /// Update token type: "lock_unlock" or "mint_burn"
        token_type: Option<String>,
    },

    /// Set destination chain token mapping (outgoing: local token → dest chain)
    SetTokenDestination {
        /// Local token identifier
        token: String,
        /// Destination chain (4-byte registered chain ID)
        dest_chain: Binary,
        /// Destination token address (32 bytes hex)
        dest_token: String,
        /// Destination token decimals
        dest_decimals: u8,
    },

    /// Set allowed CW20 code IDs for token registration
    ///
    /// When non-empty, only CW20 contracts instantiated from these code IDs
    /// can be registered via AddToken. Empty list = no restriction (backward compatible).
    /// Typical: CW20 base and CW20-mintable code IDs from your deployment.
    ///
    /// Authorization: Admin only
    SetAllowedCw20CodeIds {
        /// List of allowed code IDs (e.g. [cw20_code_id, cw20_mintable_code_id])
        code_ids: Vec<u64>,
    },

    /// Set incoming token mapping (incoming: source chain token → local token)
    ///
    /// Registers a mapping so the contract can validate during WithdrawSubmit
    /// that the specified token is expected from the given source chain.
    SetIncomingTokenMapping {
        /// Source chain ID (4 bytes)
        src_chain: Binary,
        /// Token bytes32 on the source chain (32 bytes)
        src_token: Binary,
        /// Local Terra token denom (e.g., "uluna")
        local_token: String,
        /// Token decimals on the source chain
        src_decimals: u8,
    },

    /// Remove an incoming token mapping
    RemoveIncomingTokenMapping {
        /// Source chain ID (4 bytes)
        src_chain: Binary,
        /// Token bytes32 on the source chain (32 bytes)
        src_token: Binary,
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

    /// Update fee configuration (legacy - for backwards compatibility)
    UpdateFees {
        fee_bps: Option<u32>,
        fee_collector: Option<String>,
    },

    /// Set V2 fee parameters (CL8Y discount support)
    SetFeeParams {
        standard_fee_bps: Option<u64>,
        discounted_fee_bps: Option<u64>,
        cl8y_threshold: Option<Uint128>,
        cl8y_token: Option<String>,
        fee_recipient: Option<String>,
    },

    /// Set custom account fee (admin only)
    SetCustomAccountFee { account: String, fee_bps: u64 },

    /// Remove custom account fee (admin only)
    RemoveCustomAccountFee { account: String },

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

/// CW20 receive hook message (for locking/burning CW20 tokens)
#[cw_serde]
pub enum ReceiveMsg {
    /// Lock CW20 tokens for bridging (LockUnlock mode)
    DepositCw20Lock {
        dest_chain: Binary,
        dest_account: Binary,
    },
    /// Burn CW20 mintable tokens for bridging (MintBurn mode)
    DepositCw20MintableBurn {
        dest_chain: Binary,
        dest_account: Binary,
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

    /// Returns information about a registered chain
    #[returns(ChainResponse)]
    Chain { chain_id: Binary },

    /// Returns all supported chains
    #[returns(ChainsResponse)]
    Chains {
        start_after: Option<Binary>,
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

    /// Simulate a bridge transaction (calculate fees using V2 fee config).
    /// If depositor is provided, uses CL8Y discount and custom fee rules.
    #[returns(SimulationResponse)]
    SimulateBridge {
        token: String,
        amount: Uint128,
        dest_chain: Binary,
        /// Optional depositor address for fee calculation (standard fee if omitted)
        depositor: Option<String>,
    },

    // ========================================================================
    // Withdrawal Queries (V2)
    // ========================================================================
    /// Get pending withdrawal by hash
    #[returns(PendingWithdrawResponse)]
    PendingWithdraw { withdraw_hash: Binary },

    /// List pending withdrawals with cursor-based pagination
    ///
    /// Returns all pending withdrawal entries (regardless of status).
    /// Operators use this to find unapproved submissions to approve.
    /// Cancelers use this to find approved-but-not-executed entries to verify.
    #[returns(PendingWithdrawalsResponse)]
    PendingWithdrawals {
        /// Cursor: the withdraw_hash of the last item from the previous page
        start_after: Option<Binary>,
        /// Max entries to return (default 10, max 30)
        limit: Option<u32>,
    },

    /// Compute withdraw hash without storing (for verification) - V1 legacy
    #[returns(ComputeHashResponse)]
    ComputeWithdrawHash {
        src_chain_key: Binary,
        dest_chain_key: Binary,
        dest_token_address: Binary,
        dest_account: Binary,
        amount: Uint128,
        nonce: u64,
    },

    /// Compute unified transfer hash (V2 7-field)
    #[returns(ComputeHashResponse)]
    ComputeTransferHash {
        /// Source chain ID (4 bytes)
        src_chain: Binary,
        /// Destination chain ID (4 bytes)
        dest_chain: Binary,
        /// Source account (32 bytes)
        src_account: Binary,
        /// Destination account (32 bytes)
        dest_account: Binary,
        /// Token address (32 bytes)
        token: Binary,
        /// Amount
        amount: Uint128,
        /// Nonce
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
    /// Get this chain's predetermined 4-byte V2 chain ID (set at instantiation)
    #[returns(ThisChainIdResponse)]
    ThisChainId {},

    /// Get allowed CW20 code IDs (empty = no restriction)
    #[returns(AllowedCw20CodeIdsResponse)]
    AllowedCw20CodeIds {},

    /// Get current withdraw delay
    #[returns(WithdrawDelayResponse)]
    WithdrawDelay {},

    /// Get rate limit config for a token
    #[returns(Option<RateLimitResponse>)]
    RateLimit { token: String },

    /// Get current period usage for a token
    #[returns(PeriodUsageResponse)]
    PeriodUsage { token: String },

    // ========================================================================
    // Fee Queries (V2)
    // ========================================================================
    /// Get fee configuration
    #[returns(FeeConfigResponse)]
    FeeConfig {},

    /// Get account fee info
    #[returns(AccountFeeResponse)]
    AccountFee { account: String },

    /// Check if account has custom fee
    #[returns(HasCustomFeeResponse)]
    HasCustomFee { account: String },

    /// Calculate fee for a specific depositor and amount
    #[returns(CalculateFeeResponse)]
    CalculateFee { depositor: String, amount: Uint128 },

    // ========================================================================
    // Token Registry Queries (V2)
    // ========================================================================
    /// Get token type
    #[returns(TokenTypeResponse)]
    TokenType { token: String },

    /// Get token destination mapping (outgoing)
    #[returns(Option<TokenDestMappingResponse>)]
    TokenDestMapping { token: String, dest_chain: Binary },

    /// Get incoming token mapping (source chain token → local token)
    #[returns(Option<IncomingTokenMappingResponse>)]
    IncomingTokenMapping {
        /// Source chain ID (4 bytes)
        src_chain: Binary,
        /// Token bytes32 on the source chain (32 bytes)
        src_token: Binary,
    },

    /// List all incoming token mappings (paginated)
    #[returns(IncomingTokenMappingsResponse)]
    IncomingTokenMappings {
        /// Pagination cursor: hex-encoded composite key "src_chain_hex:src_token_hex"
        start_after: Option<String>,
        limit: Option<u32>,
    },
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
    pub chain_id: Binary,
    pub identifier: String,
    pub identifier_hash: Binary,
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
    pub dest_chain: Binary,
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
    pub fee_bps: u64,
}

// ============================================================================
// Response Types - Withdrawal (V2)
// ============================================================================

#[cw_serde]
pub struct PendingWithdrawResponse {
    pub exists: bool,
    pub src_chain: Binary,
    pub src_account: Binary,
    pub dest_account: Binary,
    pub token: String,
    pub recipient: Addr,
    pub amount: Uint128,
    pub nonce: u64,
    pub src_decimals: u8,
    pub dest_decimals: u8,
    pub operator_funds: Vec<Coin>,
    pub submitted_at: u64,
    pub approved_at: u64,
    pub approved: bool,
    pub cancelled: bool,
    pub executed: bool,
    /// Seconds remaining in cancel window (0 if expired or not yet approved)
    pub cancel_window_remaining: u64,
}

impl Default for PendingWithdrawResponse {
    fn default() -> Self {
        Self {
            exists: false,
            src_chain: Binary::default(),
            src_account: Binary::default(),
            dest_account: Binary::default(),
            token: String::new(),
            recipient: Addr::unchecked(""),
            amount: Uint128::zero(),
            nonce: 0,
            src_decimals: 0,
            dest_decimals: 0,
            operator_funds: vec![],
            submitted_at: 0,
            approved_at: 0,
            approved: false,
            cancelled: false,
            executed: false,
            cancel_window_remaining: 0,
        }
    }
}

/// A single entry in the paginated pending withdrawals list
#[cw_serde]
pub struct PendingWithdrawalEntry {
    /// The 32-byte withdraw hash (key in storage)
    pub withdraw_hash: Binary,
    pub src_chain: Binary,
    pub src_account: Binary,
    pub dest_account: Binary,
    pub token: String,
    pub recipient: Addr,
    pub amount: Uint128,
    pub nonce: u64,
    pub src_decimals: u8,
    pub dest_decimals: u8,
    pub operator_funds: Vec<Coin>,
    pub submitted_at: u64,
    pub approved_at: u64,
    pub approved: bool,
    pub cancelled: bool,
    pub executed: bool,
    /// Seconds remaining in cancel window (0 if expired or not yet approved)
    pub cancel_window_remaining: u64,
}

/// Response for the PendingWithdrawals paginated list query
#[cw_serde]
pub struct PendingWithdrawalsResponse {
    pub withdrawals: Vec<PendingWithdrawalEntry>,
}

#[cw_serde]
pub struct ComputeHashResponse {
    pub hash: Binary,
}

#[cw_serde]
pub struct DepositInfoResponse {
    pub deposit_hash: Binary,
    pub dest_chain_key: Binary,
    pub src_account: Binary,
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
pub struct ThisChainIdResponse {
    /// This chain's predetermined 4-byte V2 chain ID (Binary)
    pub chain_id: Binary,
}

#[cw_serde]
pub struct AllowedCw20CodeIdsResponse {
    /// Allowed CW20 code IDs (empty = no restriction)
    pub code_ids: Vec<u64>,
}

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

// ============================================================================
// Response Types - Fee (V2)
// ============================================================================

#[cw_serde]
pub struct FeeConfigResponse {
    pub standard_fee_bps: u64,
    pub discounted_fee_bps: u64,
    pub cl8y_threshold: Uint128,
    pub cl8y_token: Option<Addr>,
    pub fee_recipient: Addr,
}

#[cw_serde]
pub struct AccountFeeResponse {
    pub account: Addr,
    pub fee_bps: u64,
    pub fee_type: String,
}

#[cw_serde]
pub struct HasCustomFeeResponse {
    pub has_custom_fee: bool,
}

#[cw_serde]
pub struct CalculateFeeResponse {
    pub depositor: Addr,
    pub amount: Uint128,
    pub fee_amount: Uint128,
    pub fee_bps: u64,
    pub fee_type: String,
}

// ============================================================================
// Response Types - Token Registry (V2)
// ============================================================================

#[cw_serde]
pub struct TokenTypeResponse {
    pub token: String,
    pub token_type: String,
}

#[cw_serde]
pub struct TokenDestMappingResponse {
    pub token: String,
    pub dest_chain: Binary,
    pub dest_token: Binary,
    pub dest_decimals: u8,
}

#[cw_serde]
pub struct IncomingTokenMappingResponse {
    /// Source chain ID (4 bytes)
    pub src_chain: Binary,
    /// Source token bytes32 (32 bytes)
    pub src_token: Binary,
    /// Local Terra denom
    pub local_token: String,
    /// Token decimals on the source chain
    pub src_decimals: u8,
    /// Whether this mapping is enabled
    pub enabled: bool,
}

#[cw_serde]
pub struct IncomingTokenMappingsResponse {
    pub mappings: Vec<IncomingTokenMappingResponse>,
}
