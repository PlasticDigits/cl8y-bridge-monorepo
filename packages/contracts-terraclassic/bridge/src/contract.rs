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
    execute_accept_admin, execute_add_canceler, execute_add_operator, execute_add_token,
    execute_admin_fix_pending_decimals, execute_cancel_admin_proposal, execute_deposit_native,
    execute_pause, execute_propose_admin, execute_receive, execute_recover_asset,
    execute_register_chain, execute_remove_canceler, execute_remove_custom_account_fee,
    execute_remove_incoming_token_mapping, execute_remove_operator,
    execute_set_allowed_cw20_code_ids, execute_set_custom_account_fee, execute_set_fee_params,
    execute_set_incoming_token_mapping, execute_set_rate_limit, execute_set_token_destination,
    execute_set_withdraw_delay, execute_unpause, execute_unregister_chain, execute_update_chain,
    execute_update_limits, execute_update_min_signatures, execute_update_token,
    execute_withdraw_approve, execute_withdraw_cancel, execute_withdraw_execute_mint,
    execute_withdraw_execute_unlock, execute_withdraw_submit, execute_withdraw_uncancel,
};
use crate::fee_manager::{FeeConfig, FEE_CONFIG};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::query::{
    query_account_fee, query_all_custom_account_fees, query_all_rate_limits,
    query_all_token_dest_mappings, query_allowed_cw20_code_ids, query_calculate_fee,
    query_cancelers, query_chain, query_chains, query_compute_xchain_hash_id, query_config,
    query_current_nonce, query_deposit_by_nonce, query_fee_config, query_has_custom_fee,
    query_incoming_token_mapping, query_incoming_token_mappings, query_is_canceler,
    query_locked_balance, query_operators, query_pending_admin, query_pending_withdraw,
    query_pending_withdrawals, query_period_usage, query_rate_limit, query_simulate_bridge,
    query_stats, query_status, query_this_chain_id, query_token, query_token_dest_mapping,
    query_token_type, query_tokens, query_transaction, query_verify_deposit, query_withdraw_delay,
    query_xchain_hash_id,
};
use crate::state::{
    Config, Stats, CONFIG, CONTRACT_NAME, CONTRACT_VERSION, DEFAULT_WITHDRAW_DELAY, OPERATORS,
    OPERATOR_COUNT, OUTGOING_NONCE, STATS, THIS_CHAIN_ID, WITHDRAW_DELAY,
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

    // Initialize V2 fee config - use msg.fee_bps for standard_fee_bps to match EVM (0.5% = 50 bps default)
    let mut fee_config = FeeConfig::default_with_recipient(config.fee_collector.clone());
    fee_config.standard_fee_bps = msg.fee_bps as u64;
    FEE_CONFIG.save(deps.storage, &fee_config)?;

    // Set this chain's predetermined 4-byte chain ID
    if msg.this_chain_id.len() != 4 {
        return Err(ContractError::InvalidAddress {
            reason: format!(
                "this_chain_id must be exactly 4 bytes, got {}",
                msg.this_chain_id.len()
            ),
        });
    }
    let this_chain_id: [u8; 4] = [
        msg.this_chain_id[0],
        msg.this_chain_id[1],
        msg.this_chain_id[2],
        msg.this_chain_id[3],
    ];
    if this_chain_id == [0u8; 4] {
        return Err(ContractError::InvalidAddress {
            reason: "this_chain_id 0x00000000 is reserved/invalid".to_string(),
        });
    }
    THIS_CHAIN_ID.save(deps.storage, &this_chain_id)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", config.admin)
        .add_attribute("operator_count", operator_count.to_string())
        .add_attribute("min_signatures", msg.min_signatures.to_string())
        .add_attribute("withdraw_delay", DEFAULT_WITHDRAW_DELAY.to_string())
        .add_attribute("this_chain_id", format!("0x{}", hex::encode(this_chain_id))))
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
        ExecuteMsg::DepositNative {
            dest_chain,
            dest_account,
        } => execute_deposit_native(deps, env, info, dest_chain, dest_account),
        ExecuteMsg::Receive(cw20_msg) => execute_receive(deps, env, info, cw20_msg),

        // V2 Withdrawal flow
        ExecuteMsg::WithdrawSubmit {
            src_chain,
            src_account,
            token,
            recipient,
            amount,
            nonce,
        } => execute_withdraw_submit(
            deps,
            env,
            info,
            src_chain,
            src_account,
            token,
            recipient,
            amount,
            nonce,
        ),
        ExecuteMsg::WithdrawApprove { xchain_hash_id } => {
            execute_withdraw_approve(deps, env, info, xchain_hash_id)
        }
        ExecuteMsg::WithdrawCancel { xchain_hash_id } => {
            execute_withdraw_cancel(deps, env, info, xchain_hash_id)
        }
        ExecuteMsg::WithdrawUncancel { xchain_hash_id } => {
            execute_withdraw_uncancel(deps, env, info, xchain_hash_id)
        }
        ExecuteMsg::WithdrawExecuteUnlock { xchain_hash_id } => {
            execute_withdraw_execute_unlock(deps, env, info, xchain_hash_id)
        }
        ExecuteMsg::WithdrawExecuteMint { xchain_hash_id } => {
            execute_withdraw_execute_mint(deps, env, info, xchain_hash_id)
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
        ExecuteMsg::RegisterChain {
            identifier,
            chain_id,
        } => execute_register_chain(deps, info, identifier, chain_id),
        ExecuteMsg::UnregisterChain { chain_id } => execute_unregister_chain(deps, info, chain_id),
        ExecuteMsg::UpdateChain { chain_id, enabled } => {
            execute_update_chain(deps, info, chain_id, enabled)
        }
        ExecuteMsg::AddToken {
            token,
            is_native,
            token_type,
            terra_decimals,
            min_bridge_amount,
            max_bridge_amount,
        } => execute_add_token(
            deps,
            info,
            token,
            is_native,
            token_type,
            terra_decimals,
            min_bridge_amount,
            max_bridge_amount,
        ),
        ExecuteMsg::UpdateToken {
            token,
            enabled,
            token_type,
            min_bridge_amount,
            max_bridge_amount,
        } => execute_update_token(
            deps,
            info,
            token,
            enabled,
            token_type,
            min_bridge_amount,
            max_bridge_amount,
        ),
        ExecuteMsg::SetTokenDestination {
            token,
            dest_chain,
            dest_token,
            dest_decimals,
        } => {
            execute_set_token_destination(deps, info, token, dest_chain, dest_token, dest_decimals)
        }
        ExecuteMsg::SetAllowedCw20CodeIds { code_ids } => {
            execute_set_allowed_cw20_code_ids(deps, info, code_ids)
        }
        ExecuteMsg::SetIncomingTokenMapping {
            src_chain,
            src_token,
            local_token,
            src_decimals,
        } => execute_set_incoming_token_mapping(
            deps,
            info,
            src_chain,
            src_token,
            local_token,
            src_decimals,
        ),
        ExecuteMsg::RemoveIncomingTokenMapping {
            src_chain,
            src_token,
        } => execute_remove_incoming_token_mapping(deps, info, src_chain, src_token),

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
        ExecuteMsg::AdminFixPendingDecimals {
            xchain_hash_id,
            src_decimals,
        } => execute_admin_fix_pending_decimals(deps, info, xchain_hash_id, src_decimals),
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
        QueryMsg::CurrentNonce {} => to_json_binary(&query_current_nonce(deps)?),
        QueryMsg::Transaction { nonce } => to_json_binary(&query_transaction(deps, nonce)?),
        QueryMsg::LockedBalance { token } => to_json_binary(&query_locked_balance(deps, token)?),
        QueryMsg::PendingAdmin {} => to_json_binary(&query_pending_admin(deps)?),
        QueryMsg::SimulateBridge {
            token,
            amount,
            dest_chain,
            depositor,
        } => to_json_binary(&query_simulate_bridge(
            deps, token, amount, dest_chain, depositor,
        )?),

        // Withdrawal queries (V2)
        QueryMsg::PendingWithdraw { xchain_hash_id } => {
            to_json_binary(&query_pending_withdraw(deps, env, xchain_hash_id)?)
        }
        QueryMsg::PendingWithdrawals { start_after, limit } => {
            to_json_binary(&query_pending_withdrawals(deps, env, start_after, limit)?)
        }
        QueryMsg::ComputeXchainHashId {
            src_chain,
            dest_chain,
            src_account,
            dest_account,
            token,
            amount,
            nonce,
        } => to_json_binary(&query_compute_xchain_hash_id(
            src_chain,
            dest_chain,
            src_account,
            dest_account,
            token,
            amount,
            nonce,
        )?),
        QueryMsg::XchainHashId { xchain_hash_id } => {
            to_json_binary(&query_xchain_hash_id(deps, xchain_hash_id)?)
        }
        QueryMsg::DepositByNonce { nonce } => to_json_binary(&query_deposit_by_nonce(deps, nonce)?),
        QueryMsg::VerifyDeposit {
            xchain_hash_id,
            dest_token_address,
            dest_account,
            amount,
            nonce,
        } => to_json_binary(&query_verify_deposit(
            deps,
            xchain_hash_id,
            dest_token_address,
            dest_account,
            amount,
            nonce,
        )?),

        // Canceler queries
        QueryMsg::Cancelers {} => to_json_binary(&query_cancelers(deps)?),
        QueryMsg::IsCanceler { address } => to_json_binary(&query_is_canceler(deps, address)?),

        // Configuration queries
        QueryMsg::ThisChainId {} => to_json_binary(&query_this_chain_id(deps)?),
        QueryMsg::AllowedCw20CodeIds {} => to_json_binary(&query_allowed_cw20_code_ids(deps)?),
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
        QueryMsg::TokenDestMapping { token, dest_chain } => {
            to_json_binary(&query_token_dest_mapping(deps, token, dest_chain)?)
        }

        // Incoming token registry queries
        QueryMsg::IncomingTokenMapping {
            src_chain,
            src_token,
        } => to_json_binary(&query_incoming_token_mapping(deps, src_chain, src_token)?),
        QueryMsg::IncomingTokenMappings { start_after, limit } => {
            to_json_binary(&query_incoming_token_mappings(deps, start_after, limit)?)
        }

        // Enumeration queries (full state audit)
        QueryMsg::AllRateLimits { start_after, limit } => {
            to_json_binary(&query_all_rate_limits(deps, start_after, limit)?)
        }
        QueryMsg::AllCustomAccountFees { start_after, limit } => {
            to_json_binary(&query_all_custom_account_fees(deps, start_after, limit)?)
        }
        QueryMsg::AllTokenDestMappings { start_after, limit } => {
            to_json_binary(&query_all_token_dest_mappings(deps, start_after, limit)?)
        }
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
