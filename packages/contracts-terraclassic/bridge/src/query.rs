//! Query handlers for the CL8Y Bridge contract.
//!
//! This module contains all query message handlers for retrieving contract state.

use cosmwasm_std::{Addr, Binary, Deps, Env, Order, StdError, StdResult, Uint128};
use cw_storage_plus::Bound;

use crate::hash::compute_transfer_id;
use crate::msg::{
    CancelersResponse, ChainResponse, ChainsResponse, ComputeHashResponse, ConfigResponse,
    DepositInfoResponse, IsCancelerResponse, LockedBalanceResponse, NonceResponse,
    NonceUsedResponse, OperatorsResponse, PendingAdminResponse, PeriodUsageResponse,
    RateLimitResponse, SimulationResponse, StatsResponse, StatusResponse, TokenResponse,
    TokensResponse, TransactionResponse, VerifyDepositResponse, WithdrawApprovalResponse,
    WithdrawDelayResponse,
};
use crate::state::{
    CANCELERS, CHAINS, CONFIG, DEPOSIT_BY_NONCE, DEPOSIT_HASHES, LOCKED_BALANCES, OPERATORS,
    OPERATOR_COUNT, OUTGOING_NONCE, PENDING_ADMIN, RATE_LIMITS, RATE_LIMIT_PERIOD, RATE_WINDOWS,
    STATS, TOKENS, TRANSACTIONS, USED_NONCES, WITHDRAW_APPROVALS, WITHDRAW_DELAY,
};

// ============================================================================
// Core Queries
// ============================================================================

/// Query contract configuration.
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        admin: config.admin,
        paused: config.paused,
        min_signatures: config.min_signatures,
        min_bridge_amount: config.min_bridge_amount,
        max_bridge_amount: config.max_bridge_amount,
        fee_bps: config.fee_bps,
        fee_collector: config.fee_collector,
    })
}

/// Query contract status summary.
pub fn query_status(deps: Deps) -> StdResult<StatusResponse> {
    let config = CONFIG.load(deps.storage)?;
    let operator_count = OPERATOR_COUNT.load(deps.storage)?;

    let chains: Vec<_> = CHAINS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    let tokens: Vec<_> = TOKENS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    Ok(StatusResponse {
        paused: config.paused,
        active_operators: operator_count,
        supported_chains: chains.len() as u32,
        supported_tokens: tokens.len() as u32,
    })
}

/// Query bridge statistics.
pub fn query_stats(deps: Deps) -> StdResult<StatsResponse> {
    let stats = STATS.load(deps.storage)?;
    Ok(StatsResponse {
        total_outgoing_txs: stats.total_outgoing_txs,
        total_incoming_txs: stats.total_incoming_txs,
        total_fees_collected: stats.total_fees_collected,
    })
}

// ============================================================================
// Chain Queries
// ============================================================================

/// Query a specific chain configuration.
pub fn query_chain(deps: Deps, chain_id: u64) -> StdResult<ChainResponse> {
    let chain_key = chain_id.to_string();
    let chain = CHAINS.load(deps.storage, chain_key)?;
    Ok(ChainResponse {
        chain_id: chain.chain_id,
        name: chain.name,
        bridge_address: chain.bridge_address,
        enabled: chain.enabled,
    })
}

/// Query paginated list of chains.
pub fn query_chains(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<ChainsResponse> {
    let limit = limit.unwrap_or(10).min(50) as usize;
    let start = start_after.map(|id| Bound::exclusive(id.to_string()));

    let chains: Vec<ChainResponse> = CHAINS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, chain) = item?;
            Ok(ChainResponse {
                chain_id: chain.chain_id,
                name: chain.name,
                bridge_address: chain.bridge_address,
                enabled: chain.enabled,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(ChainsResponse { chains })
}

// ============================================================================
// Token Queries
// ============================================================================

/// Query a specific token configuration.
pub fn query_token(deps: Deps, token: String) -> StdResult<TokenResponse> {
    let token_config = TOKENS.load(deps.storage, token)?;
    Ok(TokenResponse {
        token: token_config.token,
        is_native: token_config.is_native,
        evm_token_address: token_config.evm_token_address,
        terra_decimals: token_config.terra_decimals,
        evm_decimals: token_config.evm_decimals,
        enabled: token_config.enabled,
    })
}

/// Query paginated list of tokens.
pub fn query_tokens(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<TokensResponse> {
    let limit = limit.unwrap_or(10).min(50) as usize;
    let start = start_after.map(Bound::exclusive);

    let tokens: Vec<TokenResponse> = TOKENS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, token_config) = item?;
            Ok(TokenResponse {
                token: token_config.token,
                is_native: token_config.is_native,
                evm_token_address: token_config.evm_token_address,
                terra_decimals: token_config.terra_decimals,
                evm_decimals: token_config.evm_decimals,
                enabled: token_config.enabled,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(TokensResponse { tokens })
}

// ============================================================================
// Operator Queries
// ============================================================================

/// Query all operators and min signatures.
pub fn query_operators(deps: Deps) -> StdResult<OperatorsResponse> {
    let config = CONFIG.load(deps.storage)?;

    let operators: Vec<Addr> = OPERATORS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (addr, active) = item.ok()?;
            if active {
                Some(addr)
            } else {
                None
            }
        })
        .collect();

    Ok(OperatorsResponse {
        operators,
        min_signatures: config.min_signatures,
    })
}

// ============================================================================
// Nonce and Transaction Queries
// ============================================================================

/// Check if a nonce has been used.
pub fn query_nonce_used(deps: Deps, nonce: u64) -> StdResult<NonceUsedResponse> {
    let used = USED_NONCES.may_load(deps.storage, nonce)?.unwrap_or(false);
    Ok(NonceUsedResponse { nonce, used })
}

/// Get the current outgoing nonce.
pub fn query_current_nonce(deps: Deps) -> StdResult<NonceResponse> {
    let nonce = OUTGOING_NONCE.load(deps.storage)?;
    Ok(NonceResponse { nonce })
}

/// Query a specific transaction by nonce.
pub fn query_transaction(deps: Deps, nonce: u64) -> StdResult<TransactionResponse> {
    let tx = TRANSACTIONS.load(deps.storage, nonce)?;
    Ok(TransactionResponse {
        nonce: tx.nonce,
        sender: tx.sender,
        recipient: tx.recipient,
        token: tx.token,
        amount: tx.amount,
        dest_chain_id: tx.dest_chain_id,
        timestamp: tx.timestamp,
        is_outgoing: tx.is_outgoing,
    })
}

// ============================================================================
// Balance Queries
// ============================================================================

/// Query locked balance for a token.
pub fn query_locked_balance(deps: Deps, token: String) -> StdResult<LockedBalanceResponse> {
    let amount = LOCKED_BALANCES
        .may_load(deps.storage, token.clone())?
        .unwrap_or(Uint128::zero());
    Ok(LockedBalanceResponse { token, amount })
}

// ============================================================================
// Admin Queries
// ============================================================================

/// Query pending admin transfer.
pub fn query_pending_admin(deps: Deps) -> StdResult<Option<PendingAdminResponse>> {
    let pending = PENDING_ADMIN.may_load(deps.storage)?;
    Ok(pending.map(|p| PendingAdminResponse {
        new_address: p.new_address,
        execute_after: p.execute_after,
    }))
}

// ============================================================================
// Simulation Queries
// ============================================================================

/// Simulate a bridge operation.
pub fn query_simulate_bridge(
    deps: Deps,
    token: String,
    amount: Uint128,
    dest_chain_id: u64,
) -> StdResult<SimulationResponse> {
    let config = CONFIG.load(deps.storage)?;

    let chain_key = dest_chain_id.to_string();
    let _chain = CHAINS.load(deps.storage, chain_key)?;
    let _token_config = TOKENS.load(deps.storage, token)?;

    let fee_amount = amount.multiply_ratio(config.fee_bps as u128, 10000u128);
    let output_amount = amount - fee_amount;

    Ok(SimulationResponse {
        input_amount: amount,
        fee_amount,
        output_amount,
        fee_bps: config.fee_bps,
    })
}

// ============================================================================
// Watchtower Queries
// ============================================================================

/// Query a withdraw approval by hash.
pub fn query_withdraw_approval(
    deps: Deps,
    env: Env,
    withdraw_hash: Binary,
) -> StdResult<WithdrawApprovalResponse> {
    let hash_bytes: [u8; 32] = withdraw_hash
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid hash length"))?;

    let approval = WITHDRAW_APPROVALS.may_load(deps.storage, &hash_bytes)?;

    match approval {
        Some(a) => {
            let delay = WITHDRAW_DELAY.load(deps.storage)?;
            let elapsed = env.block.time.seconds() - a.approved_at.seconds();
            let remaining = if elapsed >= delay { 0 } else { delay - elapsed };

            Ok(WithdrawApprovalResponse {
                exists: true,
                src_chain_key: Binary::from(a.src_chain_key.to_vec()),
                token: a.token,
                recipient: a.recipient,
                dest_account: Binary::from(a.dest_account.to_vec()),
                amount: a.amount,
                nonce: a.nonce,
                fee: a.fee,
                fee_recipient: a.fee_recipient,
                approved_at: a.approved_at,
                is_approved: a.is_approved,
                deduct_from_amount: a.deduct_from_amount,
                cancelled: a.cancelled,
                executed: a.executed,
                delay_remaining: remaining,
            })
        }
        None => Ok(WithdrawApprovalResponse::default()),
    }
}

/// Compute a withdraw hash from parameters.
pub fn query_compute_withdraw_hash(
    src_chain_key: Binary,
    dest_chain_key: Binary,
    dest_token_address: Binary,
    dest_account: Binary,
    amount: Uint128,
    nonce: u64,
) -> StdResult<ComputeHashResponse> {
    let src: [u8; 32] = src_chain_key
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid src_chain_key length"))?;
    let dest: [u8; 32] = dest_chain_key
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid dest_chain_key length"))?;
    let token: [u8; 32] = dest_token_address
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid dest_token_address length"))?;
    let account: [u8; 32] = dest_account
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid dest_account length"))?;

    let hash = compute_transfer_id(&src, &dest, &token, &account, amount.u128(), nonce);

    Ok(ComputeHashResponse {
        hash: Binary::from(hash.to_vec()),
    })
}

// ============================================================================
// Deposit Queries
// ============================================================================

/// Query a deposit by hash.
pub fn query_deposit_hash(
    deps: Deps,
    deposit_hash: Binary,
) -> StdResult<Option<DepositInfoResponse>> {
    let hash_bytes: [u8; 32] = deposit_hash
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid hash length"))?;

    let deposit = DEPOSIT_HASHES.may_load(deps.storage, &hash_bytes)?;

    Ok(deposit.map(|d| DepositInfoResponse {
        deposit_hash: Binary::from(hash_bytes.to_vec()),
        dest_chain_key: Binary::from(d.dest_chain_key.to_vec()),
        dest_token_address: Binary::from(d.dest_token_address.to_vec()),
        dest_account: Binary::from(d.dest_account.to_vec()),
        amount: d.amount,
        nonce: d.nonce,
        deposited_at: d.deposited_at,
    }))
}

/// Query a deposit by nonce.
pub fn query_deposit_by_nonce(deps: Deps, nonce: u64) -> StdResult<Option<DepositInfoResponse>> {
    let hash = DEPOSIT_BY_NONCE.may_load(deps.storage, nonce)?;

    match hash {
        Some(hash_bytes) => {
            let deposit = DEPOSIT_HASHES.may_load(deps.storage, &hash_bytes)?;
            Ok(deposit.map(|d| DepositInfoResponse {
                deposit_hash: Binary::from(hash_bytes.to_vec()),
                dest_chain_key: Binary::from(d.dest_chain_key.to_vec()),
                dest_token_address: Binary::from(d.dest_token_address.to_vec()),
                dest_account: Binary::from(d.dest_account.to_vec()),
                amount: d.amount,
                nonce: d.nonce,
                deposited_at: d.deposited_at,
            }))
        }
        None => Ok(None),
    }
}

/// Verify a deposit against provided parameters.
pub fn query_verify_deposit(
    deps: Deps,
    deposit_hash: Binary,
    dest_chain_key: Binary,
    dest_token_address: Binary,
    dest_account: Binary,
    amount: Uint128,
    nonce: u64,
) -> StdResult<VerifyDepositResponse> {
    let hash_bytes: [u8; 32] = deposit_hash
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid hash length"))?;

    let deposit = DEPOSIT_HASHES.may_load(deps.storage, &hash_bytes)?;

    match deposit {
        Some(d) => {
            // Verify parameters match
            let dest_key_match = dest_chain_key.to_vec() == d.dest_chain_key.to_vec();
            let token_match = dest_token_address.to_vec() == d.dest_token_address.to_vec();
            let account_match = dest_account.to_vec() == d.dest_account.to_vec();
            let amount_match = amount == d.amount;
            let nonce_match = nonce == d.nonce;

            let matches =
                dest_key_match && token_match && account_match && amount_match && nonce_match;

            Ok(VerifyDepositResponse {
                exists: true,
                matches,
                deposit: Some(DepositInfoResponse {
                    deposit_hash: Binary::from(hash_bytes.to_vec()),
                    dest_chain_key: Binary::from(d.dest_chain_key.to_vec()),
                    dest_token_address: Binary::from(d.dest_token_address.to_vec()),
                    dest_account: Binary::from(d.dest_account.to_vec()),
                    amount: d.amount,
                    nonce: d.nonce,
                    deposited_at: d.deposited_at,
                }),
            })
        }
        None => Ok(VerifyDepositResponse {
            exists: false,
            matches: false,
            deposit: None,
        }),
    }
}

// ============================================================================
// Canceler Queries
// ============================================================================

/// Query all cancelers.
pub fn query_cancelers(deps: Deps) -> StdResult<CancelersResponse> {
    let cancelers: Vec<Addr> = CANCELERS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|item| {
            let (addr, active) = item.ok()?;
            if active {
                Some(addr)
            } else {
                None
            }
        })
        .collect();

    Ok(CancelersResponse { cancelers })
}

/// Check if an address is a canceler.
pub fn query_is_canceler(deps: Deps, address: String) -> StdResult<IsCancelerResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let is_canceler = CANCELERS.may_load(deps.storage, &addr)?.unwrap_or(false);
    Ok(IsCancelerResponse { is_canceler })
}

// ============================================================================
// Configuration Queries
// ============================================================================

/// Query the withdraw delay.
pub fn query_withdraw_delay(deps: Deps) -> StdResult<WithdrawDelayResponse> {
    let delay = WITHDRAW_DELAY.load(deps.storage)?;
    Ok(WithdrawDelayResponse {
        delay_seconds: delay,
    })
}

/// Query rate limit for a token.
pub fn query_rate_limit(deps: Deps, token: String) -> StdResult<Option<RateLimitResponse>> {
    let config = RATE_LIMITS.may_load(deps.storage, &token)?;
    Ok(config.map(|c| RateLimitResponse {
        token,
        max_per_transaction: c.max_per_transaction,
        max_per_period: c.max_per_period,
    }))
}

/// Query current period usage for a token.
pub fn query_period_usage(deps: Deps, env: Env, token: String) -> StdResult<PeriodUsageResponse> {
    let config = RATE_LIMITS.may_load(deps.storage, &token)?;
    let window = RATE_WINDOWS.may_load(deps.storage, &token)?;

    let (used, window_start) = match window {
        Some(w) => {
            // Check if window is expired
            if env.block.time.seconds() >= w.window_start.seconds() + RATE_LIMIT_PERIOD {
                (Uint128::zero(), env.block.time)
            } else {
                (w.used, w.window_start)
            }
        }
        None => (Uint128::zero(), env.block.time),
    };

    let max_per_period = config.map(|c| c.max_per_period).unwrap_or(Uint128::zero());

    let remaining = if max_per_period.is_zero() {
        Uint128::MAX // Unlimited
    } else if used >= max_per_period {
        Uint128::zero()
    } else {
        max_per_period - used
    };

    let period_ends_at =
        cosmwasm_std::Timestamp::from_seconds(window_start.seconds() + RATE_LIMIT_PERIOD);

    Ok(PeriodUsageResponse {
        token,
        current_period_start: window_start,
        used_amount: used,
        remaining_amount: remaining,
        period_ends_at,
    })
}
