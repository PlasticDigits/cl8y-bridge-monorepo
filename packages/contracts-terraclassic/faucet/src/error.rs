use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: only admin can perform this action")]
    Unauthorized,

    #[error("Token not registered: {token}")]
    TokenNotRegistered { token: String },

    #[error("24h cooldown: claimable at {claimable_at}")]
    Cooldown { claimable_at: u64 },
}
