use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::{BridgeConfig, PendingWithdraw, ExecutedHash};
use crate::error::BridgeError;

#[derive(Accounts)]
pub struct WithdrawExecuteNative<'info> {
    #[account(
        mut,
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [PendingWithdraw::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump = pending_withdraw.bump,
        close = recipient,
    )]
    pub pending_withdraw: Account<'info, PendingWithdraw>,

    #[account(
        init,
        payer = recipient,
        space = 8 + ExecutedHash::INIT_SPACE,
        seeds = [ExecutedHash::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump,
    )]
    pub executed_hash: Account<'info, ExecutedHash>,

    #[account(mut)]
    pub recipient: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawExecuteNative>) -> Result<()> {
    let bridge = &ctx.accounts.bridge;

    require!(!bridge.paused, BridgeError::BridgePaused);

    {
        let pw = &ctx.accounts.pending_withdraw;
        require!(pw.approved, BridgeError::NotApproved);
        require!(!pw.cancelled, BridgeError::WithdrawalCancelled);
        require!(!pw.executed, BridgeError::AlreadyExecuted);
        require!(pw.dest_account == ctx.accounts.recipient.key(), BridgeError::WrongRecipient);

        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp >= pw.approved_at + bridge.withdraw_delay,
            BridgeError::DelayNotElapsed
        );
    }

    let pw = &mut ctx.accounts.pending_withdraw;
    pw.executed = true;

    let amount: u64 = pw.amount
        .try_into()
        .map_err(|_| BridgeError::AmountExceedsU64)?;
    let transfer_hash = pw.transfer_hash;
    let dest_account = pw.dest_account;

    ctx.accounts.executed_hash.bump = ctx.bumps.executed_hash;

    let bridge_seeds: &[&[u8]] = &[BridgeConfig::SEED, &[bridge.bump]];

    system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.bridge.to_account_info(),
                to: ctx.accounts.recipient.to_account_info(),
            },
            &[bridge_seeds],
        ),
        amount,
    )?;

    emit!(WithdrawExecuteNativeEvent {
        transfer_hash,
        recipient: dest_account,
        amount: pw.amount,
    });

    Ok(())
}

#[event]
pub struct WithdrawExecuteNativeEvent {
    pub transfer_hash: [u8; 32],
    pub recipient: Pubkey,
    pub amount: u128,
}
