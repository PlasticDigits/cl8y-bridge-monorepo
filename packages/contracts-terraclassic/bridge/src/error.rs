//! Error types for the CL8Y Bridge contract

use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: only admin can perform this action")]
    Unauthorized,

    #[error("Unauthorized: only relayer can perform this action")]
    UnauthorizedRelayer,

    #[error("Unauthorized: only pending admin can accept")]
    UnauthorizedPendingAdmin,

    #[error("No pending admin change")]
    NoPendingAdmin,

    #[error("Timelock not expired: {remaining_seconds} seconds remaining")]
    TimelockNotExpired { remaining_seconds: u64 },

    #[error("Bridge is paused")]
    BridgePaused,

    #[error("Invalid chain ID: {chain_id}")]
    InvalidChainId { chain_id: u64 },

    #[error("Chain not supported: {chain_id}")]
    ChainNotSupported { chain_id: u64 },

    #[error("Token not supported: {token}")]
    TokenNotSupported { token: String },

    #[error("Nonce already used: {nonce}")]
    NonceAlreadyUsed { nonce: u64 },

    #[error("Invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Insufficient signatures: got {got}, need {required}")]
    InsufficientSignatures { got: u32, required: u32 },

    #[error("Invalid address: {reason}")]
    InvalidAddress { reason: String },

    #[error("No funds sent")]
    NoFundsSent,

    #[error("Invalid amount: {reason}")]
    InvalidAmount { reason: String },

    #[error("Minimum bridge amount is {min_amount}")]
    BelowMinimumAmount { min_amount: String },

    #[error("Maximum bridge amount is {max_amount}")]
    AboveMaximumAmount { max_amount: String },

    #[error("Insufficient bridge liquidity")]
    InsufficientLiquidity,

    #[error("Relayer already registered")]
    RelayerAlreadyRegistered,

    #[error("Relayer not registered")]
    RelayerNotRegistered,

    #[error("Cannot remove last relayer")]
    CannotRemoveLastRelayer,

    #[error("Asset recovery only available when bridge is paused")]
    RecoveryNotAvailable,
}
