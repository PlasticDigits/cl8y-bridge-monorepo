use crate::error::BridgeError;
use crate::hash::compute_transfer_hash;
use crate::state::{BridgeConfig, ChainEntry, ExecutedHash, PendingWithdraw, TokenMapping};
use anchor_lang::prelude::*;
use anchor_lang::system_program;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawSubmitParams {
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    /// Remote token identifier (bytes32); must match registered [`TokenMapping::dest_token`].
    pub src_token: [u8; 32],
    pub dest_token: Pubkey,
    pub amount: u128,
    pub nonce: u64,
    /// Lamports escrowed for the operator; paid on approve (EVM `operatorGas`).
    pub operator_gas: u64,
}

#[derive(Accounts)]
#[instruction(params: WithdrawSubmitParams)]
pub struct WithdrawSubmit<'info> {
    #[account(
        mut,
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        seeds = [ChainEntry::SEED, params.src_chain.as_ref()],
        bump = src_chain_entry.bump,
    )]
    pub src_chain_entry: Account<'info, ChainEntry>,

    #[account(
        seeds = [
            TokenMapping::SEED,
            params.src_chain.as_ref(),
            params.src_token.as_ref(),
        ],
        bump = token_mapping.bump,
        constraint = token_mapping.local_mint == params.dest_token @ BridgeError::TokenMappingMismatch
    )]
    pub token_mapping: Account<'info, TokenMapping>,

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
    require!(params.amount > 0, BridgeError::ZeroAmount);
    require!(
        params.src_chain != bridge.chain_id,
        BridgeError::SameChainTransfer
    );

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

    let tm = &ctx.accounts.token_mapping;
    if params.operator_gas > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.recipient.to_account_info(),
                    to: ctx.accounts.bridge.to_account_info(),
                },
            ),
            params.operator_gas,
        )?;
    }

    let pw = &mut ctx.accounts.pending_withdraw;
    pw.transfer_hash = transfer_hash;
    pw.src_chain = params.src_chain;
    pw.src_account = params.src_account;
    pw.dest_account = ctx.accounts.recipient.key();
    pw.token = params.dest_token;
    pw.amount = params.amount;
    pw.nonce = params.nonce;
    pw.src_decimals = tm.src_decimals;
    pw.dest_decimals = tm.decimals;
    pw.operator_gas = params.operator_gas;
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
        operator_gas: params.operator_gas,
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
    pub operator_gas: u64,
}
