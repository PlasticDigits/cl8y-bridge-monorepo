use anchor_lang::prelude::*;
use anchor_spl::token::{self, SetAuthority, Token, Mint};
use crate::state::{FaucetConfig, FaucetError};

#[derive(Accounts)]
pub struct RegisterMint<'info> {
    #[account(
        seeds = [FaucetConfig::SEED],
        bump = faucet_config.bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = admin.key() == faucet_config.admin @ FaucetError::Unauthorized,
    )]
    pub admin: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<RegisterMint>) -> Result<()> {
    let faucet_pda = ctx.accounts.faucet_config.key();
    token::set_authority(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            SetAuthority {
                account_or_mint: ctx.accounts.mint.to_account_info(),
                current_authority: ctx.accounts.admin.to_account_info(),
            },
        ),
        token::spl_token::instruction::AuthorityType::MintTokens,
        Some(faucet_pda),
    )?;
    Ok(())
}
