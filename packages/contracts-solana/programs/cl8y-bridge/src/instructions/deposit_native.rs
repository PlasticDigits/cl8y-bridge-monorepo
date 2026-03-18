use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::state::{BridgeConfig, ChainEntry, DepositRecord};
use crate::error::BridgeError;
use crate::hash::compute_transfer_hash;
use crate::address_codec::pubkey_to_bytes32;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct DepositNativeParams {
    pub dest_chain: [u8; 4],
    pub dest_account: [u8; 32],
    pub dest_token: [u8; 32],
    pub amount: u64,
}

#[derive(Accounts)]
#[instruction(params: DepositNativeParams)]
pub struct DepositNative<'info> {
    #[account(
        mut,
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = depositor,
        space = 8 + DepositRecord::INIT_SPACE,
        seeds = [DepositRecord::SEED, (bridge.deposit_nonce + 1).to_le_bytes().as_ref()],
        bump,
    )]
    pub deposit_record: Account<'info, DepositRecord>,

    #[account(
        seeds = [ChainEntry::SEED, params.dest_chain.as_ref()],
        bump = dest_chain_entry.bump,
    )]
    pub dest_chain_entry: Account<'info, ChainEntry>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<DepositNative>, params: DepositNativeParams) -> Result<()> {
    let bridge = &mut ctx.accounts.bridge;
    require!(!bridge.paused, BridgeError::BridgePaused);
    require!(params.amount > 0, BridgeError::ZeroAmount);

    let fee = (params.amount as u128)
        .checked_mul(bridge.fee_bps as u128)
        .ok_or(BridgeError::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(BridgeError::ArithmeticOverflow)? as u64;
    let net_amount = params.amount.checked_sub(fee).ok_or(BridgeError::FeeExceedsAmount)?;

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.depositor.to_account_info(),
                to: bridge.to_account_info(),
            },
        ),
        params.amount,
    )?;

    bridge.deposit_nonce += 1;
    let nonce = bridge.deposit_nonce;

    let src_account = pubkey_to_bytes32(&ctx.accounts.depositor.key());
    let net_amount_u128 = net_amount as u128;

    let transfer_hash = compute_transfer_hash(
        &bridge.chain_id,
        &params.dest_chain,
        &src_account,
        &params.dest_account,
        &params.dest_token,
        net_amount_u128,
        nonce,
    );

    let deposit = &mut ctx.accounts.deposit_record;
    deposit.transfer_hash = transfer_hash;
    deposit.src_account = ctx.accounts.depositor.key();
    deposit.dest_chain = params.dest_chain;
    deposit.dest_account = params.dest_account;
    deposit.token = params.dest_token;
    deposit.amount = net_amount_u128;
    deposit.nonce = nonce;
    deposit.timestamp = Clock::get()?.unix_timestamp;
    deposit.bump = ctx.bumps.deposit_record;

    emit!(DepositEvent {
        transfer_hash,
        src_account,
        dest_chain: params.dest_chain,
        dest_account: params.dest_account,
        token: params.dest_token,
        amount: net_amount_u128,
        fee: fee as u128,
        nonce,
    });

    Ok(())
}

#[event]
pub struct DepositEvent {
    pub transfer_hash: [u8; 32],
    pub src_account: [u8; 32],
    pub dest_chain: [u8; 4],
    pub dest_account: [u8; 32],
    pub token: [u8; 32],
    pub amount: u128,
    pub fee: u128,
    pub nonce: u64,
}
