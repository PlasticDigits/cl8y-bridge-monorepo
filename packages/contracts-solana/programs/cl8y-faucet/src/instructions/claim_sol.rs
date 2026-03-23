use crate::state::{ClaimRecord, FaucetConfig, FaucetError};
use anchor_lang::prelude::*;

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

    // FaucetConfig PDA holds account data — SystemProgram::transfer rejects `from` with data.
    // Move lamports directly (same pattern as bridge `withdraw_execute_native`).
    let faucet_info = ctx.accounts.faucet_config.to_account_info();
    let claimer_info = ctx.accounts.claimer.to_account_info();
    **faucet_info.try_borrow_mut_lamports()? = faucet_info
        .lamports()
        .checked_sub(lamports)
        .ok_or(FaucetError::ArithmeticOverflow)?;
    **claimer_info.try_borrow_mut_lamports()? = claimer_info
        .lamports()
        .checked_add(lamports)
        .ok_or(FaucetError::ArithmeticOverflow)?;

    record.last_claimed_at = clock.unix_timestamp;
    record.bump = ctx.bumps.claim_record;

    Ok(())
}
