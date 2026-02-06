//! Error types for the CL8Y Bridge contract
//!
//! This module defines all error types including the watchtower pattern errors (v2.0).

use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    // ========================================================================
    // Authorization Errors
    // ========================================================================
    #[error("Unauthorized: only admin can perform this action")]
    Unauthorized,

    #[error("Unauthorized: only operator can perform this action")]
    UnauthorizedOperator,

    #[error("Unauthorized: only pending admin can accept")]
    UnauthorizedPendingAdmin,

    #[error("Unauthorized: caller is not a canceler")]
    NotCanceler,

    // ========================================================================
    // Admin Errors
    // ========================================================================
    #[error("No pending admin change")]
    NoPendingAdmin,

    #[error("Timelock not expired: {remaining_seconds} seconds remaining")]
    TimelockNotExpired { remaining_seconds: u64 },

    // ========================================================================
    // Bridge State Errors
    // ========================================================================
    #[error("Bridge is paused")]
    BridgePaused,

    #[error("Invalid chain ID: {chain_id}")]
    InvalidChainId { chain_id: u64 },

    #[error("Chain not supported: {chain_id}")]
    ChainNotSupported { chain_id: u64 },

    #[error("Token not supported: {token}")]
    TokenNotSupported { token: String },

    #[error("Invalid token type: expected {expected}, got {actual}")]
    InvalidTokenType { expected: String, actual: String },

    #[error("Fee exceeds maximum: {fee_bps} bps (max 100)")]
    FeeExceedsMax { fee_bps: u64 },

    // ========================================================================
    // Nonce Errors
    // ========================================================================
    #[error("Nonce already used: {nonce}")]
    NonceAlreadyUsed { nonce: u64 },

    #[error("Invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },

    #[error("Nonce already approved for source chain: nonce {nonce}")]
    NonceAlreadyApproved { nonce: u64 },

    // ========================================================================
    // Signature Errors (legacy)
    // ========================================================================
    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Insufficient signatures: got {got}, need {required}")]
    InsufficientSignatures { got: u32, required: u32 },

    // ========================================================================
    // Amount & Funds Errors
    // ========================================================================
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

    #[error("Insufficient fee: expected {expected} uluna, got {got} uluna")]
    InsufficientFee { expected: Uint128, got: Uint128 },

    // ========================================================================
    // Operator Errors
    // ========================================================================
    #[error("Operator already registered")]
    OperatorAlreadyRegistered,

    #[error("Operator not registered")]
    OperatorNotRegistered,

    #[error("Cannot remove last operator")]
    CannotRemoveLastOperator,

    // ========================================================================
    // Withdrawal Flow Errors (V2)
    // ========================================================================
    #[error("Withdrawal not found")]
    WithdrawNotFound,

    #[error("Withdrawal not approved")]
    WithdrawNotApproved,

    #[error("Withdrawal already submitted")]
    WithdrawAlreadySubmitted,

    #[error("Withdrawal already executed")]
    WithdrawAlreadyExecuted,

    #[error("Withdrawal cancelled")]
    WithdrawCancelled,

    #[error("Withdrawal not cancelled (cannot uncancel)")]
    WithdrawNotCancelled,

    #[error("Cancel window still active, ends at {ends_at}")]
    CancelWindowActive { ends_at: u64 },

    #[error("Cancel window expired")]
    CancelWindowExpired,

    #[error("Withdraw data missing for hash")]
    WithdrawDataMissing,

    #[error("Invalid token type for operation: expected {expected}")]
    WrongTokenType { expected: String },

    // ========================================================================
    // Legacy Watchtower Errors (deprecated, kept for backward compat)
    // ========================================================================
    #[error("Withdrawal approval cancelled")]
    ApprovalCancelled,

    #[error("Withdrawal already executed (legacy)")]
    ApprovalAlreadyExecuted,

    #[error("Withdrawal delay not elapsed: {remaining_seconds} seconds remaining")]
    WithdrawDelayNotElapsed { remaining_seconds: u64 },

    #[error("Approval not cancelled (cannot reenable)")]
    ApprovalNotCancelled,

    // ========================================================================
    // Rate Limit Errors
    // ========================================================================
    #[error("Rate limit exceeded: {limit_type} limit is {limit}, requested {requested}")]
    RateLimitExceeded {
        limit_type: String,
        limit: Uint128,
        requested: Uint128,
    },

    // ========================================================================
    // Validation Errors
    // ========================================================================
    #[error("Invalid hash length: expected 32 bytes, got {got}")]
    InvalidHashLength { got: usize },

    #[error("Invalid withdraw delay: must be between 60 and 86400 seconds")]
    InvalidWithdrawDelay,

    // ========================================================================
    // Recovery Errors
    // ========================================================================
    #[error("Asset recovery only available when bridge is paused")]
    RecoveryNotAvailable,
}
