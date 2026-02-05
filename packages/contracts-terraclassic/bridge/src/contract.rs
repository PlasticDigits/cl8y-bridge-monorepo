//! CL8Y Bridge Contract - Entry Points
//!
//! This contract implements the watchtower security pattern for cross-chain bridging.
//! The implementation is modularized into:
//! - `execute/` - Execute message handlers
//! - `query` - Query message handlers

use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::execute::{
    execute_accept_admin, execute_add_canceler, execute_add_chain, execute_add_operator,
    execute_add_token, execute_approve_withdraw, execute_cancel_admin_proposal,
    execute_cancel_withdraw_approval, execute_execute_withdraw, execute_lock_native, execute_pause,
    execute_propose_admin, execute_receive, execute_recover_asset,
    execute_reenable_withdraw_approval, execute_remove_canceler, execute_remove_custom_account_fee,
    execute_remove_operator, execute_set_custom_account_fee, execute_set_fee_params,
    execute_set_rate_limit, execute_set_token_destination, execute_set_withdraw_delay,
    execute_unpause, execute_update_chain, execute_update_fees, execute_update_limits,
    execute_update_min_signatures, execute_update_token,
};
use crate::fee_manager::{FeeConfig, FEE_CONFIG};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::query::{
    query_account_fee, query_calculate_fee, query_cancelers, query_chain, query_chains,
    query_compute_withdraw_hash, query_config, query_current_nonce, query_deposit_by_nonce,
    query_deposit_hash, query_fee_config, query_has_custom_fee, query_is_canceler,
    query_locked_balance, query_nonce_used, query_operators, query_pending_admin,
    query_period_usage, query_rate_limit, query_simulate_bridge, query_stats, query_status,
    query_token, query_token_dest_mapping, query_token_type, query_tokens, query_transaction,
    query_verify_deposit, query_withdraw_approval, query_withdraw_delay,
};
use crate::state::{
    Config, Stats, CONFIG, CONTRACT_NAME, CONTRACT_VERSION, DEFAULT_WITHDRAW_DELAY, OPERATORS,
    OPERATOR_COUNT, OUTGOING_NONCE, STATS, WITHDRAW_DELAY,
};

// ============================================================================
// Instantiate
// ============================================================================

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate admin address
    let admin = deps.api.addr_validate(&msg.admin)?;
    let fee_collector = deps.api.addr_validate(&msg.fee_collector)?;

    // Validate operators
    if msg.operators.is_empty() {
        return Err(ContractError::InvalidAddress {
            reason: "At least one operator required".to_string(),
        });
    }

    if msg.min_signatures == 0 || msg.min_signatures > msg.operators.len() as u32 {
        return Err(ContractError::InsufficientSignatures {
            got: 0,
            required: msg.min_signatures,
        });
    }

    // Store config
    let config = Config {
        admin,
        paused: false,
        min_signatures: msg.min_signatures,
        min_bridge_amount: msg.min_bridge_amount,
        max_bridge_amount: msg.max_bridge_amount,
        fee_bps: msg.fee_bps,
        fee_collector,
    };
    CONFIG.save(deps.storage, &config)?;

    // Initialize operators
    let mut operator_count = 0u32;
    for operator_str in msg.operators {
        let operator = deps.api.addr_validate(&operator_str)?;
        OPERATORS.save(deps.storage, &operator, &true)?;
        operator_count += 1;
    }
    OPERATOR_COUNT.save(deps.storage, &operator_count)?;

    // Initialize stats
    let stats = Stats {
        total_outgoing_txs: 0,
        total_incoming_txs: 0,
        total_fees_collected: Uint128::zero(),
    };
    STATS.save(deps.storage, &stats)?;

    // Initialize nonce
    OUTGOING_NONCE.save(deps.storage, &0u64)?;

    // Initialize withdraw delay (watchtower pattern)
    WITHDRAW_DELAY.save(deps.storage, &DEFAULT_WITHDRAW_DELAY)?;

    // Initialize V2 fee config
    let fee_config = FeeConfig::default_with_recipient(config.fee_collector.clone());
    FEE_CONFIG.save(deps.storage, &fee_config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", config.admin)
        .add_attribute("operator_count", operator_count.to_string())
        .add_attribute("min_signatures", msg.min_signatures.to_string())
        .add_attribute("withdraw_delay", DEFAULT_WITHDRAW_DELAY.to_string()))
}

// ============================================================================
// Execute
// ============================================================================

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Outgoing transfers
        ExecuteMsg::Lock {
            dest_chain_id,
            recipient,
        } => execute_lock_native(deps, env, info, dest_chain_id, recipient),
        ExecuteMsg::Receive(cw20_msg) => execute_receive(deps, env, info, cw20_msg),

        // Watchtower pattern
        ExecuteMsg::ApproveWithdraw {
            src_chain_key,
            token,
            recipient,
            dest_account,
            amount,
            nonce,
            fee,
            fee_recipient,
            deduct_from_amount,
        } => execute_approve_withdraw(
            deps,
            env,
            info,
            src_chain_key,
            token,
            recipient,
            dest_account,
            amount,
            nonce,
            fee,
            fee_recipient,
            deduct_from_amount,
        ),
        ExecuteMsg::ExecuteWithdraw { withdraw_hash } => {
            execute_execute_withdraw(deps, env, info, withdraw_hash)
        }
        ExecuteMsg::CancelWithdrawApproval { withdraw_hash } => {
            execute_cancel_withdraw_approval(deps, info, withdraw_hash)
        }
        ExecuteMsg::ReenableWithdrawApproval { withdraw_hash } => {
            execute_reenable_withdraw_approval(deps, env, info, withdraw_hash)
        }

        // Canceler management
        ExecuteMsg::AddCanceler { address } => execute_add_canceler(deps, info, address),
        ExecuteMsg::RemoveCanceler { address } => execute_remove_canceler(deps, info, address),

        // Configuration
        ExecuteMsg::SetWithdrawDelay { delay_seconds } => {
            execute_set_withdraw_delay(deps, info, delay_seconds)
        }
        ExecuteMsg::SetRateLimit {
            token,
            max_per_transaction,
            max_per_period,
        } => execute_set_rate_limit(deps, info, token, max_per_transaction, max_per_period),

        // Chain & token management
        ExecuteMsg::AddChain {
            chain_id,
            name,
            bridge_address,
        } => execute_add_chain(deps, info, chain_id, name, bridge_address),
        ExecuteMsg::UpdateChain {
            chain_id,
            name,
            bridge_address,
            enabled,
        } => execute_update_chain(deps, info, chain_id, name, bridge_address, enabled),
        ExecuteMsg::AddToken {
            token,
            is_native,
            token_type,
            evm_token_address,
            terra_decimals,
            evm_decimals,
        } => execute_add_token(
            deps,
            info,
            token,
            is_native,
            token_type,
            evm_token_address,
            terra_decimals,
            evm_decimals,
        ),
        ExecuteMsg::UpdateToken {
            token,
            evm_token_address,
            enabled,
            token_type,
        } => execute_update_token(deps, info, token, evm_token_address, enabled, token_type),
        ExecuteMsg::SetTokenDestination {
            token,
            dest_chain_id,
            dest_token,
            dest_decimals,
        } => execute_set_token_destination(
            deps,
            info,
            token,
            dest_chain_id,
            dest_token,
            dest_decimals,
        ),

        // Operator management
        ExecuteMsg::AddOperator { operator } => execute_add_operator(deps, info, operator),
        ExecuteMsg::RemoveOperator { operator } => execute_remove_operator(deps, info, operator),
        ExecuteMsg::UpdateMinSignatures { min_signatures } => {
            execute_update_min_signatures(deps, info, min_signatures)
        }

        // Bridge configuration
        ExecuteMsg::UpdateLimits {
            min_bridge_amount,
            max_bridge_amount,
        } => execute_update_limits(deps, info, min_bridge_amount, max_bridge_amount),
        ExecuteMsg::UpdateFees {
            fee_bps,
            fee_collector,
        } => execute_update_fees(deps, info, fee_bps, fee_collector),
        ExecuteMsg::SetFeeParams {
            standard_fee_bps,
            discounted_fee_bps,
            cl8y_threshold,
            cl8y_token,
            fee_recipient,
        } => execute_set_fee_params(
            deps,
            info,
            standard_fee_bps,
            discounted_fee_bps,
            cl8y_threshold,
            cl8y_token,
            fee_recipient,
        ),
        ExecuteMsg::SetCustomAccountFee { account, fee_bps } => {
            execute_set_custom_account_fee(deps, info, account, fee_bps)
        }
        ExecuteMsg::RemoveCustomAccountFee { account } => {
            execute_remove_custom_account_fee(deps, info, account)
        }

        // Admin operations
        ExecuteMsg::Pause {} => execute_pause(deps, info),
        ExecuteMsg::Unpause {} => execute_unpause(deps, info),
        ExecuteMsg::ProposeAdmin { new_admin } => execute_propose_admin(deps, env, info, new_admin),
        ExecuteMsg::AcceptAdmin {} => execute_accept_admin(deps, env, info),
        ExecuteMsg::CancelAdminProposal {} => execute_cancel_admin_proposal(deps, info),
        ExecuteMsg::RecoverAsset {
            asset,
            amount,
            recipient,
        } => execute_recover_asset(deps, info, asset, amount, recipient),
    }
}

// ============================================================================
// Query
// ============================================================================

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        // Core queries
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Status {} => to_json_binary(&query_status(deps)?),
        QueryMsg::Stats {} => to_json_binary(&query_stats(deps)?),
        QueryMsg::Chain { chain_id } => to_json_binary(&query_chain(deps, chain_id)?),
        QueryMsg::Chains { start_after, limit } => {
            to_json_binary(&query_chains(deps, start_after, limit)?)
        }
        QueryMsg::Token { token } => to_json_binary(&query_token(deps, token)?),
        QueryMsg::Tokens { start_after, limit } => {
            to_json_binary(&query_tokens(deps, start_after, limit)?)
        }
        QueryMsg::Operators {} => to_json_binary(&query_operators(deps)?),
        QueryMsg::NonceUsed { nonce } => to_json_binary(&query_nonce_used(deps, nonce)?),
        QueryMsg::CurrentNonce {} => to_json_binary(&query_current_nonce(deps)?),
        QueryMsg::Transaction { nonce } => to_json_binary(&query_transaction(deps, nonce)?),
        QueryMsg::LockedBalance { token } => to_json_binary(&query_locked_balance(deps, token)?),
        QueryMsg::PendingAdmin {} => to_json_binary(&query_pending_admin(deps)?),
        QueryMsg::SimulateBridge {
            token,
            amount,
            dest_chain_id,
        } => to_json_binary(&query_simulate_bridge(deps, token, amount, dest_chain_id)?),

        // Watchtower queries
        QueryMsg::WithdrawApproval { withdraw_hash } => {
            to_json_binary(&query_withdraw_approval(deps, env, withdraw_hash)?)
        }
        QueryMsg::ComputeWithdrawHash {
            src_chain_key,
            dest_chain_key,
            dest_token_address,
            dest_account,
            amount,
            nonce,
        } => to_json_binary(&query_compute_withdraw_hash(
            src_chain_key,
            dest_chain_key,
            dest_token_address,
            dest_account,
            amount,
            nonce,
        )?),
        QueryMsg::DepositHash { deposit_hash } => {
            to_json_binary(&query_deposit_hash(deps, deposit_hash)?)
        }
        QueryMsg::DepositByNonce { nonce } => to_json_binary(&query_deposit_by_nonce(deps, nonce)?),
        QueryMsg::VerifyDeposit {
            deposit_hash,
            dest_chain_key,
            dest_token_address,
            dest_account,
            amount,
            nonce,
        } => to_json_binary(&query_verify_deposit(
            deps,
            deposit_hash,
            dest_chain_key,
            dest_token_address,
            dest_account,
            amount,
            nonce,
        )?),

        // Canceler queries
        QueryMsg::Cancelers {} => to_json_binary(&query_cancelers(deps)?),
        QueryMsg::IsCanceler { address } => to_json_binary(&query_is_canceler(deps, address)?),

        // Configuration queries
        QueryMsg::WithdrawDelay {} => to_json_binary(&query_withdraw_delay(deps)?),
        QueryMsg::RateLimit { token } => to_json_binary(&query_rate_limit(deps, token)?),
        QueryMsg::PeriodUsage { token } => to_json_binary(&query_period_usage(deps, env, token)?),

        // Fee queries (V2)
        QueryMsg::FeeConfig {} => to_json_binary(&query_fee_config(deps)?),
        QueryMsg::AccountFee { account } => to_json_binary(&query_account_fee(deps, account)?),
        QueryMsg::HasCustomFee { account } => to_json_binary(&query_has_custom_fee(deps, account)?),
        QueryMsg::CalculateFee { depositor, amount } => {
            to_json_binary(&query_calculate_fee(deps, depositor, amount)?)
        }

        // Token registry queries (V2)
        QueryMsg::TokenType { token } => to_json_binary(&query_token_type(deps, token)?),
        QueryMsg::TokenDestMapping {
            token,
            dest_chain_id,
        } => to_json_binary(&query_token_dest_mapping(deps, token, dest_chain_id)?),
    }
}

// ============================================================================
// Migrate
// ============================================================================

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Initialize withdraw delay if not set (for v1 -> v2 migration)
    if WITHDRAW_DELAY.may_load(deps.storage)?.is_none() {
        WITHDRAW_DELAY.save(deps.storage, &DEFAULT_WITHDRAW_DELAY)?;
    }

    // Initialize V2 fee config if not set
    if FEE_CONFIG.may_load(deps.storage)?.is_none() {
        let config = CONFIG.load(deps.storage)?;
        let fee_config = FeeConfig::default_with_recipient(config.fee_collector.clone());
        FEE_CONFIG.save(deps.storage, &fee_config)?;
    }

    Ok(Response::new()
        .add_attribute("action", "migrate")
        .add_attribute("version", CONTRACT_VERSION)
        .add_attribute("withdraw_delay", DEFAULT_WITHDRAW_DELAY.to_string()))
}
