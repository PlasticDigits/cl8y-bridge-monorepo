//! Admin operations handlers.
//!
//! This module handles:
//! - Pause/unpause contract
//! - Admin transfer (propose/accept/cancel)
//! - Asset recovery (emergency)

use cosmwasm_std::{BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, Uint128};
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::state::{PendingAdmin, ADMIN_TIMELOCK_DURATION, CONFIG, PENDING_ADMIN};
use common::AssetInfo;

// ============================================================================
// Pause/Unpause
// ============================================================================

/// Pause the contract (stops all transfers).
pub fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    config.paused = true;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("method", "pause"))
}

/// Unpause the contract (resumes transfers).
pub fn execute_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    config.paused = false;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("method", "unpause"))
}

// ============================================================================
// Admin Transfer
// ============================================================================

/// Propose a new admin (starts timelock).
pub fn execute_propose_admin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let new_admin_addr = deps.api.addr_validate(&new_admin)?;
    let pending = PendingAdmin {
        new_address: new_admin_addr.clone(),
        execute_after: env.block.time.plus_seconds(ADMIN_TIMELOCK_DURATION),
    };
    PENDING_ADMIN.save(deps.storage, &pending)?;

    Ok(Response::new()
        .add_attribute("method", "propose_admin")
        .add_attribute("new_admin", new_admin_addr.to_string())
        .add_attribute("execute_after", pending.execute_after.seconds().to_string()))
}

/// Accept pending admin role (after timelock).
pub fn execute_accept_admin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let pending = PENDING_ADMIN
        .may_load(deps.storage)?
        .ok_or(ContractError::NoPendingAdmin)?;

    if info.sender != pending.new_address {
        return Err(ContractError::UnauthorizedPendingAdmin);
    }

    if env.block.time < pending.execute_after {
        let remaining = pending.execute_after.seconds() - env.block.time.seconds();
        return Err(ContractError::TimelockNotExpired {
            remaining_seconds: remaining,
        });
    }

    let mut config = CONFIG.load(deps.storage)?;
    config.admin = pending.new_address.clone();
    CONFIG.save(deps.storage, &config)?;
    PENDING_ADMIN.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("method", "accept_admin")
        .add_attribute("new_admin", pending.new_address.to_string()))
}

/// Cancel pending admin proposal.
pub fn execute_cancel_admin_proposal(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    PENDING_ADMIN.remove(deps.storage);

    Ok(Response::new().add_attribute("method", "cancel_admin_proposal"))
}

// ============================================================================
// Asset Recovery
// ============================================================================

/// Recover stuck assets (emergency, requires paused state).
///
/// LOCKED_BALANCES is intentionally not updated here. Updating it could cause
/// underflow if the recovered amount exceeds the tracked locked balance (e.g.
/// due to prior inconsistencies or dust). The admin must reconcile balances
/// separately if needed after recovery.
pub fn execute_recover_asset(
    deps: DepsMut,
    info: MessageInfo,
    asset: AssetInfo,
    amount: Uint128,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    if !config.paused {
        return Err(ContractError::RecoveryNotAvailable);
    }

    let recipient_addr = deps.api.addr_validate(&recipient)?;

    let messages: Vec<CosmosMsg> = match asset {
        AssetInfo::Native { denom } => {
            vec![CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient_addr.to_string(),
                amount: vec![Coin { denom, amount }],
            })]
        }
        AssetInfo::Cw20 { contract_addr } => {
            vec![CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: cosmwasm_std::to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: recipient_addr.to_string(),
                    amount,
                })?,
                funds: vec![],
            })]
        }
    };

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "recover_asset")
        .add_attribute("recipient", recipient)
        .add_attribute("amount", amount.to_string()))
}
