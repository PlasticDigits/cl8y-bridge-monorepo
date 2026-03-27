use anchor_lang::prelude::*;

mod instructions;
mod state;

use instructions::*;

declare_id!("B9zRqdnkfrMjLiW8n5Ejw6KSR9DmQscogpijoi5qyyY2");

#[program]
pub mod cl8y_faucet {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
        instructions::initialize::handler(ctx, params)
    }

    pub fn register_mint(ctx: Context<RegisterMint>) -> Result<()> {
        instructions::register_mint::handler(ctx)
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        instructions::claim::handler(ctx)
    }
}
