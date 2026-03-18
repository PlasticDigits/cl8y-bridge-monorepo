use anchor_lang::prelude::*;

pub mod error;
pub mod hash;
pub mod address_codec;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("CL8YBr1dg3So1ana111111111111111111111111111");

#[program]
pub mod cl8y_bridge {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
        instructions::initialize::handler(ctx, params)
    }

    pub fn deposit_native(ctx: Context<DepositNative>, params: DepositNativeParams) -> Result<()> {
        instructions::deposit_native::handler(ctx, params)
    }

    pub fn deposit_spl(ctx: Context<DepositSpl>, params: DepositSplParams) -> Result<()> {
        instructions::deposit_spl::handler(ctx, params)
    }

    pub fn withdraw_submit(ctx: Context<WithdrawSubmit>, params: WithdrawSubmitParams) -> Result<()> {
        instructions::withdraw_submit::handler(ctx, params)
    }

    pub fn withdraw_approve(ctx: Context<WithdrawApprove>, params: WithdrawApproveParams) -> Result<()> {
        instructions::withdraw_approve::handler(ctx, params)
    }

    pub fn withdraw_execute(ctx: Context<WithdrawExecute>) -> Result<()> {
        instructions::withdraw_execute::handler(ctx)
    }

    pub fn withdraw_execute_native(ctx: Context<WithdrawExecuteNative>) -> Result<()> {
        instructions::withdraw_execute_native::handler(ctx)
    }

    pub fn withdraw_cancel(ctx: Context<WithdrawCancel>) -> Result<()> {
        instructions::withdraw_cancel::handler(ctx)
    }

    pub fn withdraw_reenable(ctx: Context<WithdrawReenable>) -> Result<()> {
        instructions::withdraw_reenable::handler(ctx)
    }

    pub fn register_chain(ctx: Context<RegisterChain>, params: RegisterChainParams) -> Result<()> {
        instructions::register_chain::handler(ctx, params)
    }

    pub fn register_token(ctx: Context<RegisterToken>, params: RegisterTokenParams) -> Result<()> {
        instructions::register_token::handler(ctx, params)
    }

    pub fn set_config(ctx: Context<SetConfig>, params: SetConfigParams) -> Result<()> {
        instructions::set_config::handler(ctx, params)
    }

    pub fn add_canceler(ctx: Context<AddCanceler>, params: AddCancelerParams) -> Result<()> {
        instructions::add_canceler::handler(ctx, params)
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>, params: WithdrawFeesParams) -> Result<()> {
        instructions::withdraw_fees::handler(ctx, params)
    }
}
