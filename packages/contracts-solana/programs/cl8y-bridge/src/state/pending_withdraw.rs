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
    pub approved: bool,
    pub approved_at: i64,
    pub cancelled: bool,
    pub executed: bool,
    pub bump: u8,
}

impl PendingWithdraw {
    pub const SEED: &'static [u8] = b"withdraw";
}
