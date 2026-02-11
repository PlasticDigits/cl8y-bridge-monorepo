//! V2 Withdrawal flow handlers (user-initiated).
//!
//! This module implements the V2 withdrawal pattern:
//! 1. `WithdrawSubmit` — user creates a pending withdrawal (pays gas + operator tip)
//! 2. `WithdrawApprove` — operator verifies deposit and approves (receives gas tip)
//! 3. `WithdrawCancel` — canceler cancels within cancel window
//! 4. `WithdrawUncancel` — operator restores a cancelled withdrawal
//! 5. `WithdrawExecuteUnlock` — anyone executes (unlock mode) after cancel window
//! 6. `WithdrawExecuteMint` — anyone executes (mint mode) after cancel window

use cosmwasm_std::{
    to_json_binary, BankMsg, Binary, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, Storage,
    Uint128, Uint256, Uint512,
};
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg};

use crate::error::ContractError;
use crate::hash::{
    bytes32_to_hex, compute_transfer_hash, encode_terra_address, encode_token_address,
};
use crate::state::DEFAULT_WITHDRAW_DELAY;
use crate::state::{
    PendingWithdraw, RateLimitWindow, TokenType, CANCELERS, CHAINS, CONFIG, LOCKED_BALANCES,
    OPERATORS, PENDING_WITHDRAWS, RATE_LIMITS, RATE_LIMIT_PERIOD, RATE_WINDOWS, STATS,
    THIS_CHAIN_ID, TOKENS, TOKEN_SRC_MAPPINGS, WITHDRAW_DELAY, WITHDRAW_NONCE_USED,
};

/// Default rate limit when not configured: 0.1% of total supply, or 100 ether if supply is zero
const DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY: Uint128 = Uint128::new(100_000_000_000_000_000_000);

// ============================================================================
// WithdrawSubmit — User-initiated
// ============================================================================

/// User submits a withdrawal request on the destination chain.
///
/// The user provides parameters matching the deposit on the source chain.
/// All native tokens sent with the message are forwarded to the operator on approval.
#[allow(clippy::too_many_arguments)]
pub fn execute_withdraw_submit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    src_chain: Binary,
    src_account: Binary,
    token: String,
    recipient: String,
    amount: Uint128,
    nonce: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    if amount.is_zero() {
        return Err(ContractError::InvalidAmount {
            reason: "Amount must be greater than zero".to_string(),
        });
    }

    // Parse inputs
    let src_chain_bytes: [u8; 4] =
        src_chain
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: src_chain.len(),
            })?;
    let src_account_bytes: [u8; 32] =
        src_account
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: src_account.len(),
            })?;

    // Validate recipient and encode as bytes32
    let recipient_addr = deps.api.addr_validate(&recipient)?;
    let dest_account_bytes = encode_terra_address(deps.as_ref(), &recipient_addr)?;

    // Validate token is supported, enabled, and get decimals
    let token_config =
        TOKENS
            .may_load(deps.storage, token.clone())?
            .ok_or(ContractError::TokenNotSupported {
                token: token.clone(),
            })?;
    if !token_config.enabled {
        return Err(ContractError::TokenNotSupported {
            token: token.clone(),
        });
    }

    // Validate source chain is registered and enabled
    let chain = CHAINS.may_load(deps.storage, &src_chain_bytes)?.ok_or(
        ContractError::ChainNotRegistered {
            chain_id: format!("0x{}", hex::encode(src_chain_bytes)),
        },
    )?;
    if !chain.enabled {
        return Err(ContractError::ChainNotRegistered {
            chain_id: format!("0x{}", hex::encode(src_chain_bytes)),
        });
    }

    // Compute destination chain (this chain)
    let dest_chain = THIS_CHAIN_ID.load(deps.storage)?;

    // Encode token for hash computation
    let token_bytes32 = encode_token_address(deps.as_ref(), &token)?;

    // Validate incoming token mapping exists for this source chain
    let src_chain_key = hex::encode(src_chain_bytes);
    let src_token_key = hex::encode(token_bytes32);
    let src_mapping = TOKEN_SRC_MAPPINGS
        .may_load(deps.storage, (&src_chain_key, &src_token_key))?
        .ok_or(ContractError::TokenNotMappedForChain {
            chain_id: format!("0x{}", src_chain_key),
            token: token.clone(),
        })?;
    if !src_mapping.enabled {
        return Err(ContractError::TokenNotMappedForChain {
            chain_id: format!("0x{}", src_chain_key),
            token: token.clone(),
        });
    }

    // Compute withdraw hash (same hash format as the deposit on source chain)
    let withdraw_hash = compute_transfer_hash(
        &src_chain_bytes,
        &dest_chain,
        &src_account_bytes,
        &dest_account_bytes,
        &token_bytes32,
        amount.u128(),
        nonce,
    );

    // Check not already submitted
    if PENDING_WITHDRAWS
        .may_load(deps.storage, &withdraw_hash)?
        .is_some()
    {
        return Err(ContractError::WithdrawAlreadySubmitted);
    }

    let recipient = recipient_addr;

    // All native tokens sent with message are forwarded to operator on approve
    let operator_funds = info.funds.clone();

    // Store pending withdrawal
    let pending = PendingWithdraw {
        src_chain: src_chain_bytes,
        src_account: src_account_bytes,
        dest_account: dest_account_bytes,
        token: token.clone(),
        recipient: recipient.clone(),
        amount,
        nonce,
        src_decimals: src_mapping.src_decimals,
        dest_decimals: token_config.terra_decimals,
        operator_funds,
        submitted_at: env.block.time.seconds(),
        approved_at: 0,
        approved: false,
        cancelled: false,
        executed: false,
    };
    PENDING_WITHDRAWS.save(deps.storage, &withdraw_hash, &pending)?;

    Ok(Response::new()
        .add_attribute("action", "withdraw_submit")
        .add_attribute("withdraw_hash", bytes32_to_hex(&withdraw_hash))
        .add_attribute("token", token)
        .add_attribute("recipient", recipient.to_string())
        .add_attribute("amount", amount.to_string())
        .add_attribute("nonce", nonce.to_string()))
}

// ============================================================================
// WithdrawApprove — Operator
// ============================================================================

/// Operator approves a pending withdrawal after verifying source chain deposit.
pub fn execute_withdraw_approve(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Verify caller is operator
    let is_operator = OPERATORS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);
    if !is_operator && info.sender != config.admin {
        return Err(ContractError::UnauthorizedOperator);
    }

    let hash_bytes = parse_hash(&withdraw_hash)?;

    let mut pending = PENDING_WITHDRAWS
        .may_load(deps.storage, &hash_bytes)?
        .ok_or(ContractError::WithdrawNotFound)?;

    if pending.executed {
        return Err(ContractError::WithdrawAlreadyExecuted);
    }
    if pending.approved {
        return Err(ContractError::WithdrawAlreadyApproved);
    }

    // Approve and start cancel window
    pending.approved = true;
    pending.approved_at = env.block.time.seconds();
    PENDING_WITHDRAWS.save(deps.storage, &hash_bytes, &pending)?;

    // Mark nonce as used for source chain
    let nonce_key = (pending.src_chain.as_slice(), pending.nonce);
    WITHDRAW_NONCE_USED.save(deps.storage, nonce_key, &true)?;

    // Forward all operator funds to the approving operator
    let mut messages: Vec<CosmosMsg> = vec![];
    if !pending.operator_funds.is_empty() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: pending.operator_funds.clone(),
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "withdraw_approve")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes)))
}

// ============================================================================
// WithdrawCancel — Canceler
// ============================================================================

/// Canceler cancels a pending withdrawal within the cancel window.
/// Only cancelers may cancel; operators and admin cannot (RBAC separation).
pub fn execute_withdraw_cancel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    // Verify caller is canceler only (not operator or admin)
    let is_canceler = CANCELERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);
    if !is_canceler {
        return Err(ContractError::NotCanceler);
    }

    let hash_bytes = parse_hash(&withdraw_hash)?;

    let mut pending = PENDING_WITHDRAWS
        .may_load(deps.storage, &hash_bytes)?
        .ok_or(ContractError::WithdrawNotFound)?;

    if pending.executed {
        return Err(ContractError::WithdrawAlreadyExecuted);
    }
    if !pending.approved {
        return Err(ContractError::WithdrawNotApproved);
    }

    // Check within cancel window (use configurable WITHDRAW_DELAY)
    let cancel_window = WITHDRAW_DELAY
        .may_load(deps.storage)?
        .unwrap_or(DEFAULT_WITHDRAW_DELAY);
    let window_end = pending.approved_at + cancel_window;
    if env.block.time.seconds() > window_end {
        return Err(ContractError::CancelWindowExpired);
    }

    pending.cancelled = true;
    PENDING_WITHDRAWS.save(deps.storage, &hash_bytes, &pending)?;

    Ok(Response::new()
        .add_attribute("action", "withdraw_cancel")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes))
        .add_attribute("cancelled_by", info.sender.to_string()))
}

// ============================================================================
// WithdrawUncancel — Operator
// ============================================================================

/// Operator uncancels a cancelled withdrawal and resets the cancel window.
pub fn execute_withdraw_uncancel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let is_operator = OPERATORS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);
    if !is_operator && info.sender != config.admin {
        return Err(ContractError::UnauthorizedOperator);
    }

    let hash_bytes = parse_hash(&withdraw_hash)?;

    let mut pending = PENDING_WITHDRAWS
        .may_load(deps.storage, &hash_bytes)?
        .ok_or(ContractError::WithdrawNotFound)?;

    if pending.executed {
        return Err(ContractError::WithdrawAlreadyExecuted);
    }
    if !pending.cancelled {
        return Err(ContractError::WithdrawNotCancelled);
    }

    // Uncancel and reset approval time (restarts cancel window)
    pending.cancelled = false;
    pending.approved_at = env.block.time.seconds();
    PENDING_WITHDRAWS.save(deps.storage, &hash_bytes, &pending)?;

    Ok(Response::new()
        .add_attribute("action", "withdraw_uncancel")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes))
        .add_attribute("new_approved_at", env.block.time.seconds().to_string()))
}

// ============================================================================
// WithdrawExecuteUnlock — Anyone (after cancel window)
// ============================================================================

/// Execute a withdrawal by unlocking tokens (LockUnlock mode).
pub fn execute_withdraw_execute_unlock(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    let hash_bytes = parse_hash(&withdraw_hash)?;
    let cancel_window = WITHDRAW_DELAY
        .may_load(deps.storage)?
        .unwrap_or(DEFAULT_WITHDRAW_DELAY);
    let mut pending = load_and_validate_execution(deps.storage, &env, &hash_bytes, cancel_window)?;

    // Verify token type is LockUnlock
    let token_config = TOKENS.load(deps.storage, pending.token.clone())?;
    if !matches!(token_config.token_type, TokenType::LockUnlock) {
        return Err(ContractError::WrongTokenType {
            expected: "lock_unlock".to_string(),
        });
    }

    // Normalize amount from source chain decimals to destination chain decimals
    let payout_amount =
        normalize_decimals(pending.amount, pending.src_decimals, pending.dest_decimals)?;

    // Check rate limits (resolve limits first to avoid overlapping borrows)
    let limits = resolve_effective_rate_limits(deps.as_ref(), &pending.token)?;
    check_and_update_rate_limit(
        deps.storage,
        &env,
        &pending.token,
        payout_amount,
        limits.0,
        limits.1,
    )?;

    // Check liquidity
    let locked = LOCKED_BALANCES
        .may_load(deps.storage, pending.token.clone())?
        .unwrap_or(Uint128::zero());
    if locked < payout_amount {
        return Err(ContractError::InsufficientLiquidity);
    }

    // Update locked balance
    LOCKED_BALANCES.save(
        deps.storage,
        pending.token.clone(),
        &(locked - payout_amount),
    )?;

    // Mark as executed
    pending.executed = true;
    PENDING_WITHDRAWS.save(deps.storage, &hash_bytes, &pending)?;

    // Transfer tokens to recipient
    let mut messages: Vec<CosmosMsg> = vec![];
    if token_config.is_native {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: pending.recipient.to_string(),
            amount: vec![Coin {
                denom: pending.token.clone(),
                amount: payout_amount,
            }],
        }));
    } else {
        messages.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: pending.token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: pending.recipient.to_string(),
                amount: payout_amount,
            })?,
            funds: vec![],
        }));
    }

    // Update stats
    let mut stats = STATS.load(deps.storage)?;
    stats.total_incoming_txs += 1;
    STATS.save(deps.storage, &stats)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "withdraw_execute_unlock")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes))
        .add_attribute("recipient", pending.recipient.to_string())
        .add_attribute("token", pending.token)
        .add_attribute("amount", payout_amount.to_string()))
}

// ============================================================================
// WithdrawExecuteMint — Anyone (after cancel window)
// ============================================================================

/// Execute a withdrawal by minting tokens (MintBurn mode).
pub fn execute_withdraw_execute_mint(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    withdraw_hash: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    let hash_bytes = parse_hash(&withdraw_hash)?;
    let cancel_window = WITHDRAW_DELAY
        .may_load(deps.storage)?
        .unwrap_or(DEFAULT_WITHDRAW_DELAY);
    let mut pending = load_and_validate_execution(deps.storage, &env, &hash_bytes, cancel_window)?;

    // Verify token type is MintBurn
    let token_config = TOKENS.load(deps.storage, pending.token.clone())?;
    if !matches!(token_config.token_type, TokenType::MintBurn) {
        return Err(ContractError::WrongTokenType {
            expected: "mint_burn".to_string(),
        });
    }

    // Normalize amount from source chain decimals to destination chain decimals
    let payout_amount =
        normalize_decimals(pending.amount, pending.src_decimals, pending.dest_decimals)?;

    // Check rate limits (resolve limits first to avoid overlapping borrows)
    let limits = resolve_effective_rate_limits(deps.as_ref(), &pending.token)?;
    check_and_update_rate_limit(
        deps.storage,
        &env,
        &pending.token,
        payout_amount,
        limits.0,
        limits.1,
    )?;

    // Mark as executed
    pending.executed = true;
    PENDING_WITHDRAWS.save(deps.storage, &hash_bytes, &pending)?;

    // Mint CW20 tokens to recipient
    let messages: Vec<CosmosMsg> = vec![CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
        contract_addr: pending.token.clone(),
        msg: to_json_binary(&Cw20ExecuteMsg::Mint {
            recipient: pending.recipient.to_string(),
            amount: payout_amount,
        })?,
        funds: vec![],
    })];

    // Update stats
    let mut stats = STATS.load(deps.storage)?;
    stats.total_incoming_txs += 1;
    STATS.save(deps.storage, &stats)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "withdraw_execute_mint")
        .add_attribute("withdraw_hash", bytes32_to_hex(&hash_bytes))
        .add_attribute("recipient", pending.recipient.to_string())
        .add_attribute("token", pending.token)
        .add_attribute("amount", payout_amount.to_string()))
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Parse a 32-byte hash from Binary input.
fn parse_hash(withdraw_hash: &Binary) -> Result<[u8; 32], ContractError> {
    withdraw_hash
        .to_vec()
        .try_into()
        .map_err(|_| ContractError::InvalidHashLength {
            got: withdraw_hash.len(),
        })
}

/// Normalize amount from source chain decimals to destination chain decimals.
///
/// Uses Uint256 for intermediate computation to support high-decimal tokens (e.g., 18 decimals
/// with quadrillion+ quantities). Fails the transaction on overflow instead of clamping.
///
/// If `src_decimals == dest_decimals` (or both are 0), no conversion is needed.
/// If `src_decimals > dest_decimals`, divide (truncate towards zero).
/// If `src_decimals < dest_decimals`, multiply (scale up).
fn normalize_decimals(
    amount: Uint128,
    src_decimals: u8,
    dest_decimals: u8,
) -> Result<Uint128, ContractError> {
    if src_decimals == dest_decimals {
        return Ok(amount);
    }
    if src_decimals > dest_decimals {
        let divisor = 10u128.pow((src_decimals - dest_decimals) as u32);
        Ok(Uint128::new(amount.u128() / divisor))
    } else {
        let multiplier = Uint256::from(10u128.pow((dest_decimals - src_decimals) as u32));
        let amount_256 = Uint256::from(amount.u128());
        let result_512: Uint512 = amount_256.full_mul(multiplier);
        let result_128 =
            Uint128::try_from(result_512).map_err(|_| ContractError::AmountOverflow)?;
        Ok(result_128)
    }
}

/// Load a pending withdrawal and validate it's ready for execution.
fn load_and_validate_execution(
    storage: &dyn Storage,
    env: &Env,
    hash_bytes: &[u8; 32],
    cancel_window_seconds: u64,
) -> Result<PendingWithdraw, ContractError> {
    let pending = PENDING_WITHDRAWS
        .may_load(storage, hash_bytes)?
        .ok_or(ContractError::WithdrawNotFound)?;

    if pending.executed {
        return Err(ContractError::WithdrawAlreadyExecuted);
    }
    if !pending.approved {
        return Err(ContractError::WithdrawNotApproved);
    }
    if pending.cancelled {
        return Err(ContractError::WithdrawCancelled);
    }

    // Check cancel window has passed (exclusive: execute allowed only when timestamp > window_end)
    let window_end = pending.approved_at + cancel_window_seconds;
    if env.block.time.seconds() <= window_end {
        return Err(ContractError::CancelWindowActive {
            ends_at: window_end,
        });
    }

    Ok(pending)
}

/// Resolve effective rate limits (from config or default 0.1% of supply).
fn resolve_effective_rate_limits(
    deps: cosmwasm_std::Deps,
    token: &str,
) -> Result<(Uint128, Uint128), ContractError> {
    let config = RATE_LIMITS.may_load(deps.storage, token)?;

    if let Some(ref c) = config {
        return Ok((c.max_per_transaction, c.max_per_period));
    }

    // No limit configured: use default 0.1% of supply, or 100 ether if supply is zero
    let token_config = TOKENS.may_load(deps.storage, token.to_string())?.ok_or(
        ContractError::TokenNotSupported {
            token: token.to_string(),
        },
    )?;
    let supply = query_token_supply(deps, token, token_config.is_native)?;
    let period_limit = if supply.is_zero() {
        DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY
    } else {
        supply / Uint128::new(1000) // 0.1%
    };
    Ok((Uint128::zero(), period_limit)) // No per-tx limit when using default
}

/// Check and update rate limits for a token withdrawal.
pub fn check_and_update_rate_limit(
    storage: &mut dyn Storage,
    env: &Env,
    token: &str,
    amount: Uint128,
    max_per_transaction: Uint128,
    max_per_period: Uint128,
) -> Result<(), ContractError> {
    // Check per-transaction limit
    if !max_per_transaction.is_zero() && amount > max_per_transaction {
        return Err(ContractError::RateLimitExceeded {
            limit_type: "per_transaction".to_string(),
            limit: max_per_transaction,
            requested: amount,
        });
    }

    // Check per-period limit
    if max_per_period.is_zero() {
        return Ok(()); // No period limit (only when explicitly configured that way)
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
    if new_used > max_per_period {
        return Err(ContractError::RateLimitExceeded {
            limit_type: "per_period".to_string(),
            limit: max_per_period,
            requested: amount,
        });
    }

    window.used = new_used;
    RATE_WINDOWS.save(storage, token, &window)?;

    Ok(())
}

/// Query total supply of a token (native denom or CW20).
/// For native tokens, uses BankQuery::Supply when cosmwasm_1_2 (or cosmwasm_1_1) is enabled.
/// Otherwise falls back to DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY for native tokens.
fn query_token_supply(
    deps: cosmwasm_std::Deps,
    token: &str,
    is_native: bool,
) -> Result<Uint128, ContractError> {
    if is_native {
        #[cfg(feature = "cosmwasm_1_2")]
        {
            use cosmwasm_std::{BankQuery, QueryRequest};
            let res: cosmwasm_std::SupplyResponse =
                deps.querier.query(&QueryRequest::Bank(BankQuery::Supply {
                    denom: token.to_string(),
                }))?;
            Ok(res.amount.amount)
        }
        #[cfg(not(feature = "cosmwasm_1_2"))]
        {
            let _ = (deps, token);
            // BankQuery::Supply not available without cosmwasm_1_2
            Ok(Uint128::zero())
        }
    } else {
        let res: cw20::TokenInfoResponse = deps
            .querier
            .query_wasm_smart(token, &Cw20QueryMsg::TokenInfo {})?;
        Ok(res.total_supply)
    }
}
