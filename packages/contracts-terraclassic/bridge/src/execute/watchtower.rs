//! Watchtower pattern handlers.
//!
//! This module implements the watchtower security pattern for incoming transfers:
//! - `ApproveWithdraw` - Operator approves a withdrawal (starts delay timer)
//! - `ExecuteWithdraw` - Anyone executes after delay elapsed
//! - `CancelWithdrawApproval` - Canceler blocks a fraudulent approval
//! - `ReenableWithdrawApproval` - Admin reenables a mistakenly cancelled approval

use cosmwasm_std::{
    to_json_binary, BankMsg, Binary, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response,
    Storage, Uint128,
};
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::hash::{bytes32_to_hex, compute_transfer_id, encode_token_address, terra_chain_key};
use crate::state::{
    RateLimitWindow, WithdrawApproval, CANCELERS, CONFIG, LOCKED_BALANCES, OPERATORS, RATE_LIMITS,
    RATE_LIMIT_PERIOD, RATE_WINDOWS, STATS, TOKENS, WITHDRAW_APPROVALS, WITHDRAW_DELAY,
    WITHDRAW_NONCE_USED,
};

/// Execute handler for approving a withdrawal (operator only).
#[allow(clippy::too_many_arguments)]
pub fn execute_approve_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    src_chain_key: Binary,
    token: String,
    recipient: String,
    dest_account: Binary,
    amount: Uint128,
    nonce: u64,
    fee: Uint128,
    fee_recipient: String,
    deduct_from_amount: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    // Verify caller is operator
    let is_operator = OPERATORS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);
    if !is_operator && info.sender != config.admin {
        return Err(ContractError::UnauthorizedOperator);
    }

    // Validate inputs
    let src_chain_key_bytes: [u8; 32] =
        src_chain_key
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: src_chain_key.len(),
            })?;

    let dest_account_bytes: [u8; 32] =
        dest_account
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: dest_account.len(),
            })?;

    // Check nonce not already used for this source chain
    let nonce_key = (src_chain_key_bytes.as_slice(), nonce);
    let nonce_used = WITHDRAW_NONCE_USED
        .may_load(deps.storage, nonce_key)?
        .unwrap_or(false);
    if nonce_used {
        return Err(ContractError::NonceAlreadyApproved { nonce });
    }

    // Validate addresses
    let recipient_addr = deps.api.addr_validate(&recipient)?;
    let fee_recipient_addr = deps.api.addr_validate(&fee_recipient)?;

    // Check token is supported
    let _token_config = TOKENS
        .may_load(deps.storage, token.clone())?
        .ok_or(ContractError::TokenNotSupported {
            token: token.clone(),
        })?;

    // Compute withdraw hash (transferId)
    let dest_chain_key = terra_chain_key();
    let dest_token_address = encode_token_address(deps.as_ref(), &token)?;

    let withdraw_hash = compute_transfer_id(
        &src_chain_key_bytes,
        &dest_chain_key,
        &dest_token_address,
        &dest_account_bytes,
        amount.u128(),
        nonce,
    );

    // Create approval
    let approval = WithdrawApproval {
        src_chain_key: src_chain_key_bytes,
        token: token.clone(),
        recipient: recipient_addr.clone(),
        dest_account: dest_account_bytes,
        amount,
        nonce,
        fee,
        fee_recipient: fee_recipient_addr.clone(),
        approved_at: env.block.time,
        is_approved: true,
        deduct_from_amount,
        cancelled: false,
        executed: false,
    };

    // Store approval and mark nonce as used
    WITHDRAW_APPROVALS.save(deps.storage, &withdraw_hash, &approval)?;
    WITHDRAW_NONCE_USED.save(deps.storage, nonce_key, &true)?;

    let delay = WITHDRAW_DELAY.load(deps.storage)?;

    Ok(Response::new()
        .add_attribute("method", "approve_withdraw")
        .add_attribute("withdraw_hash", bytes32_to_hex(&withdraw_hash))
        .add_attribute("src_chain_key", bytes32_to_hex(&src_chain_key_bytes))
        .add_attribute("token", token)
        .add_attribute("recipient", recipient_addr.to_string())
        .add_attribute("amount", amount.to_string())
        .add_attribute("nonce", nonce.to_string())
        .add_attribute("fee", fee.to_string())
        .add_attribute("fee_recipient", fee_recipient_addr.to_string())
        .add_attribute("deduct_from_amount", deduct_from_amount.to_string())
        .add_attribute("approved_at", env.block.time.seconds().to_string())
        .add_attribute("delay_seconds", delay.to_string()))
}

/// Execute handler for executing an approved withdrawal after delay.
pub fn execute_execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    // Parse hash
    let hash_bytes: [u8; 32] =
        withdraw_hash
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: withdraw_hash.len(),
            })?;

    // Load approval
    let mut approval = WITHDRAW_APPROVALS
        .may_load(deps.storage, &hash_bytes)?
        .ok_or(ContractError::WithdrawNotApproved)?;

    // Verify state
    if !approval.is_approved {
        return Err(ContractError::WithdrawNotApproved);
    }
    if approval.cancelled {
        return Err(ContractError::ApprovalCancelled);
    }
    if approval.executed {
        return Err(ContractError::ApprovalAlreadyExecuted);
    }

    // Check delay has elapsed
    let delay = WITHDRAW_DELAY.load(deps.storage)?;
    let elapsed = env.block.time.seconds() - approval.approved_at.seconds();
    if elapsed < delay {
        return Err(ContractError::WithdrawDelayNotElapsed {
            remaining_seconds: delay - elapsed,
        });
    }

    // Check rate limits
    check_and_update_rate_limit(deps.storage, &env, &approval.token, approval.amount)?;

    // Handle fee payment
    let mut messages: Vec<CosmosMsg> = vec![];
    let net_amount: Uint128;

    if approval.deduct_from_amount {
        // Fee deducted from amount
        net_amount = approval.amount - approval.fee;
    } else {
        // Caller must send fee in uluna
        net_amount = approval.amount;

        if !approval.fee.is_zero() {
            let sent = info
                .funds
                .iter()
                .find(|c| c.denom == "uluna")
                .map(|c| c.amount)
                .unwrap_or(Uint128::zero());

            if sent < approval.fee {
                return Err(ContractError::InsufficientFee {
                    expected: approval.fee,
                    got: sent,
                });
            }

            // Transfer fee to fee_recipient
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: approval.fee_recipient.to_string(),
                amount: vec![Coin {
                    denom: "uluna".to_string(),
                    amount: approval.fee,
                }],
            }));
        }
    }

    // Check liquidity
    let locked = LOCKED_BALANCES
        .may_load(deps.storage, approval.token.clone())?
        .unwrap_or(Uint128::zero());
    if locked < net_amount {
        return Err(ContractError::InsufficientLiquidity);
    }

    // Update locked balance
    LOCKED_BALANCES.save(deps.storage, approval.token.clone(), &(locked - net_amount))?;

    // Mark as executed
    approval.executed = true;
    WITHDRAW_APPROVALS.save(deps.storage, &hash_bytes, &approval)?;

    // Get token config
    let token_config = TOKENS.load(deps.storage, approval.token.clone())?;

    // Transfer tokens to recipient
    if token_config.is_native {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: approval.recipient.to_string(),
            amount: vec![Coin {
                denom: approval.token.clone(),
                amount: net_amount,
            }],
        }));

        // Transfer fee if deducted from amount
        if approval.deduct_from_amount && !approval.fee.is_zero() {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: approval.fee_recipient.to_string(),
                amount: vec![Coin {
                    denom: approval.token.clone(),
                    amount: approval.fee,
                }],
            }));
        }
    } else {
        messages.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: approval.token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: approval.recipient.to_string(),
                amount: net_amount,
            })?,
            funds: vec![],
        }));

        // Transfer fee if deducted from amount
        if approval.deduct_from_amount && !approval.fee.is_zero() {
            messages.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
                contract_addr: approval.token.clone(),
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: approval.fee_recipient.to_string(),
                    amount: approval.fee,
                })?,
                funds: vec![],
            }));
        }
    }

    // Update stats
    let mut stats = STATS.load(deps.storage)?;
    stats.total_incoming_txs += 1;
    STATS.save(deps.storage, &stats)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "execute_withdraw")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes))
        .add_attribute("recipient", approval.recipient.to_string())
        .add_attribute("token", approval.token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", approval.fee.to_string()))
}

/// Execute handler for cancelling a withdrawal approval (canceler/operator/admin).
pub fn execute_cancel_withdraw_approval(
    deps: DepsMut,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Verify caller is canceler, operator, or admin
    let is_canceler = CANCELERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);
    let is_operator = OPERATORS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);
    let is_admin = info.sender == config.admin;

    if !is_canceler && !is_operator && !is_admin {
        return Err(ContractError::NotCanceler);
    }

    // Parse hash
    let hash_bytes: [u8; 32] =
        withdraw_hash
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: withdraw_hash.len(),
            })?;

    // Load approval
    let mut approval = WITHDRAW_APPROVALS
        .may_load(deps.storage, &hash_bytes)?
        .ok_or(ContractError::WithdrawNotApproved)?;

    // Verify state
    if !approval.is_approved {
        return Err(ContractError::WithdrawNotApproved);
    }
    if approval.cancelled {
        return Err(ContractError::ApprovalCancelled);
    }
    if approval.executed {
        return Err(ContractError::ApprovalAlreadyExecuted);
    }

    // Cancel
    approval.cancelled = true;
    WITHDRAW_APPROVALS.save(deps.storage, &hash_bytes, &approval)?;

    Ok(Response::new()
        .add_attribute("method", "cancel_withdraw_approval")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes))
        .add_attribute("cancelled_by", info.sender.to_string()))
}

/// Execute handler for reenabling a cancelled withdrawal approval (admin only).
pub fn execute_reenable_withdraw_approval(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only admin can reenable
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    // Parse hash
    let hash_bytes: [u8; 32] =
        withdraw_hash
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: withdraw_hash.len(),
            })?;

    // Load approval
    let mut approval = WITHDRAW_APPROVALS
        .may_load(deps.storage, &hash_bytes)?
        .ok_or(ContractError::WithdrawNotApproved)?;

    // Verify state
    if !approval.is_approved {
        return Err(ContractError::WithdrawNotApproved);
    }
    if !approval.cancelled {
        return Err(ContractError::ApprovalNotCancelled);
    }
    if approval.executed {
        return Err(ContractError::ApprovalAlreadyExecuted);
    }

    // Reenable and reset timer
    approval.cancelled = false;
    approval.approved_at = env.block.time;
    WITHDRAW_APPROVALS.save(deps.storage, &hash_bytes, &approval)?;

    Ok(Response::new()
        .add_attribute("method", "reenable_withdraw_approval")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes))
        .add_attribute("new_approved_at", env.block.time.seconds().to_string()))
}

// ============================================================================
// Rate Limiting Helper
// ============================================================================

/// Check and update rate limits for a token withdrawal.
pub fn check_and_update_rate_limit(
    storage: &mut dyn Storage,
    env: &Env,
    token: &str,
    amount: Uint128,
) -> Result<(), ContractError> {
    let config = RATE_LIMITS.may_load(storage, token)?;

    let Some(config) = config else {
        return Ok(()); // No limit configured
    };

    // Check per-transaction limit
    if !config.max_per_transaction.is_zero() && amount > config.max_per_transaction {
        return Err(ContractError::RateLimitExceeded {
            limit_type: "per_transaction".to_string(),
            limit: config.max_per_transaction,
            requested: amount,
        });
    }

    // Check per-period limit
    if config.max_per_period.is_zero() {
        return Ok(()); // No period limit
    }

    let mut window = RATE_WINDOWS
        .may_load(storage, token)?
        .unwrap_or(RateLimitWindow {
            window_start: env.block.time,
            used: Uint128::zero(),
        });

    // Reset if window expired (24 hours)
    if env.block.time.seconds() >= window.window_start.seconds() + RATE_LIMIT_PERIOD {
        window = RateLimitWindow {
            window_start: env.block.time,
            used: Uint128::zero(),
        };
    }

    let new_used = window.used + amount;
    if new_used > config.max_per_period {
        return Err(ContractError::RateLimitExceeded {
            limit_type: "per_period".to_string(),
            limit: config.max_per_period,
            requested: amount,
        });
    }

    window.used = new_used;
    RATE_WINDOWS.save(storage, token, &window)?;

    Ok(())
}
