use anchor_lang::prelude::*;
use crate::state::FaucetConfig;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeParams {
    pub claim_amount: u64,
    pub cooldown_seconds: i64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + FaucetConfig::INIT_SPACE,
        seeds = [FaucetConfig::SEED],
        bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
    let config = &mut ctx.accounts.faucet_config;
    config.admin = ctx.accounts.admin.key();
    config.claim_amount = params.claim_amount;
    config.cooldown_seconds = params.cooldown_seconds;
    config.bump = ctx.bumps.faucet_config;
    Ok(())
}
