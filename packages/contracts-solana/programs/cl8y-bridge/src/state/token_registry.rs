use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum TokenMode {
    LockUnlock,
    MintBurn,
}

#[account]
#[derive(InitSpace)]
pub struct TokenMapping {
    pub local_mint: Pubkey,
    pub dest_chain: [u8; 4],
    pub dest_token: [u8; 32],
    pub mode: TokenMode,
    /// Decimals of `local_mint` on Solana (destination side).
    pub decimals: u8,
    /// Decimals of `dest_token` on the remote chain (source side for incoming withdrawals).
    pub src_decimals: u8,
    pub accrued_fees: u64,
    pub bump: u8,
}

impl TokenMapping {
    pub const SEED: &'static [u8] = b"token";
}
