use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, PendingWithdraw, CancelerEntry};
use crate::error::BridgeError;

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
    require!(ctx.accounts.canceler_entry.active, BridgeError::UnauthorizedCanceler);

    let pw = &mut ctx.accounts.pending_withdraw;
    require!(!pw.executed, BridgeError::AlreadyExecuted);

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
