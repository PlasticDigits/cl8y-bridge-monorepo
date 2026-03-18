use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, PendingWithdraw, ExecutedHash};
use crate::error::BridgeError;
use crate::hash::compute_transfer_hash;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawSubmitParams {
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    pub dest_token: Pubkey,
    pub amount: u128,
    pub nonce: u64,
}

#[derive(Accounts)]
#[instruction(params: WithdrawSubmitParams)]
pub struct WithdrawSubmit<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = recipient,
        space = 8 + PendingWithdraw::INIT_SPACE,
        seeds = [PendingWithdraw::SEED, &compute_transfer_hash(
            &params.src_chain,
            &bridge.chain_id,
            &params.src_account,
            &recipient.key().to_bytes(),
            &params.dest_token.to_bytes(),
            params.amount,
            params.nonce,
        )],
        bump,
    )]
    pub pending_withdraw: Account<'info, PendingWithdraw>,

    /// Must not exist -- proves this transfer hash has never been executed
    /// CHECK: We only verify this account does not exist (data is empty)
    #[account(
        seeds = [ExecutedHash::SEED, &compute_transfer_hash(
            &params.src_chain,
            &bridge.chain_id,
            &params.src_account,
            &recipient.key().to_bytes(),
            &params.dest_token.to_bytes(),
            params.amount,
            params.nonce,
        )],
        bump,
    )]
    pub executed_hash_check: AccountInfo<'info>,

    #[account(mut)]
    pub recipient: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawSubmit>, params: WithdrawSubmitParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(!bridge.paused, BridgeError::BridgePaused);

    // Reject if this transfer hash was already executed (close+reinit protection)
    require!(
        ctx.accounts.executed_hash_check.data_is_empty(),
        BridgeError::AlreadyExecutedHash
    );

    let dest_account = ctx.accounts.recipient.key().to_bytes();
    let token_bytes = params.dest_token.to_bytes();

    let transfer_hash = compute_transfer_hash(
        &params.src_chain,
        &bridge.chain_id,
        &params.src_account,
        &dest_account,
        &token_bytes,
        params.amount,
        params.nonce,
    );

    let pw = &mut ctx.accounts.pending_withdraw;
    pw.transfer_hash = transfer_hash;
    pw.src_chain = params.src_chain;
    pw.src_account = params.src_account;
    pw.dest_account = ctx.accounts.recipient.key();
    pw.token = params.dest_token;
    pw.amount = params.amount;
    pw.nonce = params.nonce;
    pw.approved = false;
    pw.approved_at = 0;
    pw.cancelled = false;
    pw.executed = false;
    pw.bump = ctx.bumps.pending_withdraw;

    emit!(WithdrawSubmitEvent {
        transfer_hash,
        src_chain: params.src_chain,
        dest_account,
        token: token_bytes,
        amount: params.amount,
        nonce: params.nonce,
    });

    Ok(())
}

#[event]
pub struct WithdrawSubmitEvent {
    pub transfer_hash: [u8; 32],
    pub src_chain: [u8; 4],
    pub dest_account: [u8; 32],
    pub token: [u8; 32],
    pub amount: u128,
    pub nonce: u64,
}
