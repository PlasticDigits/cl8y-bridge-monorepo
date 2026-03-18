use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TokenInterface, TokenAccount, Mint, TransferChecked, Burn};
use crate::state::{BridgeConfig, ChainEntry, DepositRecord, TokenMapping, TokenMode};
use crate::error::BridgeError;
use crate::hash::compute_transfer_hash;
use crate::address_codec::pubkey_to_bytes32;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct DepositSplParams {
    pub dest_chain: [u8; 4],
    pub dest_account: [u8; 32],
    pub amount: u64,
}

#[derive(Accounts)]
#[instruction(params: DepositSplParams)]
pub struct DepositSpl<'info> {
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
        seeds = [TokenMapping::SEED, params.dest_chain.as_ref(), token_mapping.dest_token.as_ref()],
        bump = token_mapping.bump,
    )]
    pub token_mapping: Account<'info, TokenMapping>,

    #[account(constraint = mint.key() == token_mapping.local_mint @ BridgeError::TokenNotRegistered)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = depositor,
    )]
    pub depositor_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Bridge-owned token account for lock/unlock and MintBurn fee collection
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bridge,
    )]
    pub bridge_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        seeds = [ChainEntry::SEED, params.dest_chain.as_ref()],
        bump = dest_chain_entry.bump,
    )]
    pub dest_chain_entry: Account<'info, ChainEntry>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<DepositSpl>, params: DepositSplParams) -> Result<()> {
    let bridge = &mut ctx.accounts.bridge;
    require!(!bridge.paused, BridgeError::BridgePaused);
    require!(params.amount > 0, BridgeError::ZeroAmount);

    let fee = (params.amount as u128)
        .checked_mul(bridge.fee_bps as u128)
        .ok_or(BridgeError::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(BridgeError::ArithmeticOverflow)? as u64;
    let net_amount = params.amount.checked_sub(fee).ok_or(BridgeError::FeeExceedsAmount)?;

    let token_mapping = &ctx.accounts.token_mapping;
    let decimals = ctx.accounts.mint.decimals;

    match token_mapping.mode {
        TokenMode::LockUnlock => {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.depositor_token_account.to_account_info(),
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.bridge_token_account.to_account_info(),
                        authority: ctx.accounts.depositor.to_account_info(),
                    },
                ),
                params.amount,
                decimals,
            )?;
        }
        TokenMode::MintBurn => {
            // Transfer fee to bridge token account so admin can withdraw it later
            if fee > 0 {
                token_interface::transfer_checked(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        TransferChecked {
                            from: ctx.accounts.depositor_token_account.to_account_info(),
                            mint: ctx.accounts.mint.to_account_info(),
                            to: ctx.accounts.bridge_token_account.to_account_info(),
                            authority: ctx.accounts.depositor.to_account_info(),
                        },
                    ),
                    fee,
                    decimals,
                )?;
            }
            // Burn only the net amount
            token_interface::burn(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Burn {
                        mint: ctx.accounts.mint.to_account_info(),
                        from: ctx.accounts.depositor_token_account.to_account_info(),
                        authority: ctx.accounts.depositor.to_account_info(),
                    },
                ),
                net_amount,
            )?;
        }
    }

    bridge.deposit_nonce += 1;
    let nonce = bridge.deposit_nonce;

    let src_account = pubkey_to_bytes32(&ctx.accounts.depositor.key());
    let net_amount_u128 = net_amount as u128;

    let transfer_hash = compute_transfer_hash(
        &bridge.chain_id,
        &params.dest_chain,
        &src_account,
        &params.dest_account,
        &token_mapping.dest_token,
        net_amount_u128,
        nonce,
    );

    let deposit = &mut ctx.accounts.deposit_record;
    deposit.transfer_hash = transfer_hash;
    deposit.src_account = ctx.accounts.depositor.key();
    deposit.dest_chain = params.dest_chain;
    deposit.dest_account = params.dest_account;
    deposit.token = token_mapping.dest_token;
    deposit.amount = net_amount_u128;
    deposit.nonce = nonce;
    deposit.timestamp = Clock::get()?.unix_timestamp;
    deposit.bump = ctx.bumps.deposit_record;

    emit!(DepositEvent {
        transfer_hash,
        src_account,
        dest_chain: params.dest_chain,
        dest_account: params.dest_account,
        token: token_mapping.dest_token,
        amount: net_amount_u128,
        fee: fee as u128,
        nonce,
    });

    Ok(())
}

use super::deposit_native::DepositEvent;
