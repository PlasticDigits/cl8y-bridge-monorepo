use anchor_lang::prelude::*;

#[error_code]
pub enum BridgeError {
    #[msg("Bridge is paused")]
    BridgePaused,
    #[msg("Unauthorized: caller is not the admin")]
    UnauthorizedAdmin,
    #[msg("Unauthorized: caller is not the operator")]
    UnauthorizedOperator,
    #[msg("Unauthorized: caller is not a registered canceler")]
    UnauthorizedCanceler,
    #[msg("Deposit amount must be greater than zero")]
    ZeroAmount,
    #[msg("Fee exceeds deposit amount")]
    FeeExceedsAmount,
    #[msg("Chain is not registered")]
    ChainNotRegistered,
    #[msg("Token is not registered for destination chain")]
    TokenNotRegistered,
    #[msg("Transfer hash mismatch")]
    HashMismatch,
    #[msg("Withdrawal is not approved")]
    NotApproved,
    #[msg("Withdrawal delay has not elapsed")]
    DelayNotElapsed,
    #[msg("Withdrawal has been cancelled")]
    WithdrawalCancelled,
    #[msg("Withdrawal has already been approved")]
    AlreadyApproved,
    #[msg("Withdrawal has already been executed")]
    AlreadyExecuted,
    #[msg("Withdrawal is not cancelled")]
    NotCancelled,
    #[msg("Invalid fee basis points (max 10000)")]
    InvalidFeeBps,
    #[msg("Invalid withdraw delay")]
    InvalidWithdrawDelay,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Invalid token mode")]
    InvalidTokenMode,
    #[msg("Canceler is already registered")]
    CancelerAlreadyRegistered,
    #[msg("Nonce already used")]
    NonceAlreadyUsed,
    #[msg("Recipient does not match the pending withdrawal")]
    WrongRecipient,
    #[msg("Token mint does not match the pending withdrawal")]
    TokenMintMismatch,
    #[msg("Invalid chain ID (must be non-zero)")]
    InvalidChainId,
    #[msg("Amount exceeds u64 maximum for SPL token transfer")]
    AmountExceedsU64,
    #[msg("Transfer hash has already been executed")]
    AlreadyExecutedHash,
}
