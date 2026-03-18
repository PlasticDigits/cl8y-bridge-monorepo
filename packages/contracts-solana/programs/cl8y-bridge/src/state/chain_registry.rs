use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct ChainEntry {
    pub chain_id: [u8; 4],
    #[max_len(64)]
    pub identifier: String,
    pub bump: u8,
}

impl ChainEntry {
    pub const SEED: &'static [u8] = b"chain";
}
