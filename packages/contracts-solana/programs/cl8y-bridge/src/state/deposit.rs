use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct DepositRecord {
    pub transfer_hash: [u8; 32],
    pub src_account: Pubkey,
    pub dest_chain: [u8; 4],
    pub dest_account: [u8; 32],
    pub token: [u8; 32],
    pub amount: u128,
    pub nonce: u64,
    pub timestamp: i64,
    pub bump: u8,
}

impl DepositRecord {
    pub const SEED: &'static [u8] = b"deposit";
}
