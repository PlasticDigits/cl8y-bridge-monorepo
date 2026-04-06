use anchor_lang::prelude::*;

mod instructions;
mod state;

use instructions::*;

declare_id!("7uqKHSr6naMr3LakCSRkZpiRTBboALh4ZgZdw7hvTwG4");

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
