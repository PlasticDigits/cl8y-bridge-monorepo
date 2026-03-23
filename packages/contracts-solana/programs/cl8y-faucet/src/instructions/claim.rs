use anchor_lang::prelude::*;
use anchor_spl::token::{self, MintTo, Token, Mint, TokenAccount};
use crate::state::{FaucetConfig, ClaimRecord, FaucetError};

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(
        seeds = [FaucetConfig::SEED],
        bump = faucet_config.bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    #[account(
        init_if_needed,
        payer = claimer,
        space = 8 + ClaimRecord::INIT_SPACE,
        seeds = [ClaimRecord::SEED, claimer.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub claim_record: Account<'info, ClaimRecord>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = claimer,
    )]
    pub claimer_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub claimer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Claim>) -> Result<()> {
    let config = &ctx.accounts.faucet_config;
    let record = &mut ctx.accounts.claim_record;

    let clock = Clock::get()?;
    if record.last_claimed_at > 0 {
        require!(
            clock.unix_timestamp >= record.last_claimed_at + config.cooldown_seconds,
            FaucetError::CooldownNotElapsed
        );
    }

    let seeds: &[&[u8]] = &[FaucetConfig::SEED, &[config.bump]];
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.claimer_token_account.to_account_info(),
                authority: ctx.accounts.faucet_config.to_account_info(),
            },
            &[seeds],
        ),
        config.claim_amount,
    )?;

    record.last_claimed_at = clock.unix_timestamp;
    record.bump = ctx.bumps.claim_record;

    Ok(())
}
