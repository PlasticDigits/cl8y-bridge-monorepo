use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PendingWithdraw {
    pub transfer_hash: [u8; 32],
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    pub dest_account: Pubkey,
    pub token: Pubkey,
    pub amount: u128,
    pub nonce: u64,
    /// Decimals of the token on the source chain (from token mapping).
    pub src_decimals: u8,
    /// Decimals of the local (destination) token mint.
    pub dest_decimals: u8,
    /// Lamports escrowed on submit; paid to operator on approve (EVM `operatorGas`).
    pub operator_gas: u64,
    pub approved: bool,
    pub approved_at: i64,
    pub cancelled: bool,
    pub executed: bool,
    pub bump: u8,
}

impl PendingWithdraw {
    pub const SEED: &'static [u8] = b"withdraw";
}
