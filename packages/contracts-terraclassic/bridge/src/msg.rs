//! Message types for the CL8Y Bridge contract

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Timestamp, Uint128};
use common::AssetInfo;

/// Migrate message
#[cw_serde]
pub struct MigrateMsg {}

/// Instantiate message
#[cw_serde]
pub struct InstantiateMsg {
    /// Admin address for contract management
    pub admin: String,
    /// Initial relayer addresses
    pub relayers: Vec<String>,
    /// Minimum number of relayer signatures required
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

/// Execute messages
#[cw_serde]
pub enum ExecuteMsg {
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

    /// Release tokens from an incoming bridge transaction
    /// Called by relayers with signed proof
    Release {
        /// Nonce from the source chain transaction
        nonce: u64,
        /// Sender address on source chain (EVM address)
        sender: String,
        /// Recipient address on TerraClassic
        recipient: String,
        /// Token to release (denom for native)
        token: String,
        /// Amount to release
        amount: Uint128,
        /// Source chain ID
        source_chain_id: u64,
        /// Relayer signatures (as hex strings)
        signatures: Vec<String>,
    },

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

    /// Register a new relayer
    AddRelayer { relayer: String },

    /// Remove a relayer
    RemoveRelayer { relayer: String },

    /// Update minimum signatures required
    UpdateMinSignatures { min_signatures: u32 },

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

/// Query messages
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
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

    /// Returns list of registered relayers
    #[returns(RelayersResponse)]
    Relayers {},

    /// Check if a nonce has been used
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
}

// Response types

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
    pub active_relayers: u32,
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
pub struct RelayersResponse {
    pub relayers: Vec<Addr>,
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
