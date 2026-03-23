use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::{FaucetConfig, ClaimRecord, FaucetError};

#[derive(Accounts)]
pub struct ClaimSol<'info> {
    #[account(
        mut,
        seeds = [FaucetConfig::SEED],
        bump = faucet_config.bump,
    )]
    pub faucet_config: Account<'info, FaucetConfig>,

    #[account(
        init_if_needed,
        payer = claimer,
        space = 8 + ClaimRecord::INIT_SPACE,
        seeds = [ClaimRecord::SEED, claimer.key().as_ref(), b"native_sol"],
        bump,
    )]
    pub claim_record: Account<'info, ClaimRecord>,

    #[account(mut)]
    pub claimer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimSol>, lamports: u64) -> Result<()> {
    let config = &ctx.accounts.faucet_config;
    let record = &mut ctx.accounts.claim_record;

    let clock = Clock::get()?;
    if record.last_claimed_at > 0 {
        require!(
            clock.unix_timestamp >= record.last_claimed_at + config.cooldown_seconds,
            FaucetError::CooldownNotElapsed
        );
    }

    let faucet_balance = ctx.accounts.faucet_config.to_account_info().lamports();
    let rent_exempt = Rent::get()?.minimum_balance(8 + FaucetConfig::INIT_SPACE);
    let available = faucet_balance.saturating_sub(rent_exempt);
    require!(available >= lamports, FaucetError::InsufficientSol);

    let seeds: &[&[u8]] = &[FaucetConfig::SEED, &[config.bump]];
    system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.faucet_config.to_account_info(),
                to: ctx.accounts.claimer.to_account_info(),
            },
            &[seeds],
        ),
        lamports,
    )?;

    record.last_claimed_at = clock.unix_timestamp;
    record.bump = ctx.bumps.claim_record;

    Ok(())
}
