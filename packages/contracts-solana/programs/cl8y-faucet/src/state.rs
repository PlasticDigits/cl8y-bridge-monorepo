use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct FaucetConfig {
    pub admin: Pubkey,
    pub claim_amount: u64,
    pub cooldown_seconds: i64,
    pub bump: u8,
}

impl FaucetConfig {
    pub const SEED: &'static [u8] = b"faucet";
}

#[account]
#[derive(InitSpace)]
pub struct ClaimRecord {
    pub last_claimed_at: i64,
    pub bump: u8,
}

impl ClaimRecord {
    pub const SEED: &'static [u8] = b"claim";
}

#[error_code]
pub enum FaucetError {
    #[msg("Cooldown period has not elapsed")]
    CooldownNotElapsed,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Insufficient SOL in faucet")]
    InsufficientSol,
}
