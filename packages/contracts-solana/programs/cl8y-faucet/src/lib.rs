use anchor_lang::prelude::*;

mod state;
mod instructions;

use instructions::*;

declare_id!("CL8YFaucet1111111111111111111111111111111111");

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

    pub fn claim_sol(ctx: Context<ClaimSol>, lamports: u64) -> Result<()> {
        instructions::claim_sol::handler(ctx, lamports)
    }
}
