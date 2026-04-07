use crate::error::BridgeError;
use crate::state::{BridgeConfig, NonceUsed, PendingWithdraw};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawApproveParams {
    pub transfer_hash: [u8; 32],
}

#[derive(Accounts)]
#[instruction(params: WithdrawApproveParams)]
pub struct WithdrawApprove<'info> {
    #[account(
        mut,
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [PendingWithdraw::SEED, params.transfer_hash.as_ref()],
        bump = pending_withdraw.bump,
    )]
    pub pending_withdraw: Account<'info, PendingWithdraw>,

    #[account(
        init,
        payer = operator,
        space = 8 + NonceUsed::INIT_SPACE,
        seeds = [
            NonceUsed::SEED,
            pending_withdraw.src_chain.as_ref(),
            &pending_withdraw.nonce.to_le_bytes(),
        ],
        bump,
    )]
    pub nonce_used: Account<'info, NonceUsed>,

    #[account(mut)]
    pub operator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawApprove>, params: WithdrawApproveParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(!bridge.paused, BridgeError::BridgePaused);
    require!(
        ctx.accounts.operator.key() == bridge.operator,
        BridgeError::UnauthorizedOperator
    );

    let pw = &mut ctx.accounts.pending_withdraw;
    require!(!pw.approved, BridgeError::AlreadyApproved);
    require!(!pw.cancelled, BridgeError::WithdrawalCancelled);

    // Forward operator gas (EVM: msg.sender.call{value: operatorGas})
    let gas = pw.operator_gas;
    if gas > 0 {
        let bridge_info = ctx.accounts.bridge.to_account_info();
        let operator_info = ctx.accounts.operator.to_account_info();
        let rent_exempt = Rent::get()?.minimum_balance(8 + BridgeConfig::INIT_SPACE);
        let available = bridge_info.lamports().saturating_sub(rent_exempt);
        require!(
            available >= gas,
            BridgeError::OperatorGasTransferFailed
        );
        **bridge_info.try_borrow_mut_lamports()? = bridge_info
            .lamports()
            .checked_sub(gas)
            .ok_or(BridgeError::ArithmeticOverflow)?;
        **operator_info.try_borrow_mut_lamports()? = operator_info
            .lamports()
            .checked_add(gas)
            .ok_or(BridgeError::ArithmeticOverflow)?;
    }

    pw.approved = true;
    pw.approved_at = Clock::get()?.unix_timestamp;

    ctx.accounts.nonce_used.bump = ctx.bumps.nonce_used;

    emit!(WithdrawApproveEvent {
        transfer_hash: params.transfer_hash,
        approved_at: pw.approved_at,
    });

    Ok(())
}

#[event]
pub struct WithdrawApproveEvent {
    pub transfer_hash: [u8; 32],
    pub approved_at: i64,
}
