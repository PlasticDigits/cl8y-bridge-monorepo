use crate::error::BridgeError;
use crate::state::{BridgeConfig, CancelerEntry, PendingWithdraw};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct WithdrawCancel<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [PendingWithdraw::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump = pending_withdraw.bump,
    )]
    pub pending_withdraw: Account<'info, PendingWithdraw>,

    #[account(
        seeds = [CancelerEntry::SEED, canceler.key().as_ref()],
        bump = canceler_entry.bump,
    )]
    pub canceler_entry: Account<'info, CancelerEntry>,

    pub canceler: Signer<'info>,
}

pub fn handler(ctx: Context<WithdrawCancel>) -> Result<()> {
    require!(
        ctx.accounts.canceler_entry.active,
        BridgeError::UnauthorizedCanceler
    );

    let bridge = &ctx.accounts.bridge;
    let pw = &mut ctx.accounts.pending_withdraw;
    require!(!pw.executed, BridgeError::AlreadyExecuted);
    require!(!pw.cancelled, BridgeError::WithdrawalCancelled);
    require!(pw.approved, BridgeError::NotApproved);

    let now = Clock::get()?.unix_timestamp;
    let window_end = pw
        .approved_at
        .checked_add(bridge.withdraw_delay)
        .ok_or(BridgeError::ArithmeticOverflow)?;
    require!(now <= window_end, BridgeError::CancelWindowExpired);

    pw.cancelled = true;

    emit!(WithdrawCancelEvent {
        transfer_hash: pw.transfer_hash,
        canceler: ctx.accounts.canceler.key(),
    });

    Ok(())
}

#[event]
pub struct WithdrawCancelEvent {
    pub transfer_hash: [u8; 32],
    pub canceler: Pubkey,
}
