//! Query handlers for the CL8Y Bridge contract.
//!
//! This module contains all query message handlers for retrieving contract state.

use cosmwasm_std::{Addr, Binary, Deps, Env, Order, StdError, StdResult, Uint128};
use cw_storage_plus::Bound;

use crate::fee_manager::{
    calculate_fee, calculate_fee_from_bps, get_effective_fee_bps, get_fee_type, has_custom_fee,
    FeeConfig, FEE_CONFIG,
};
use crate::hash::compute_xchain_hash_id;
use crate::msg::{
    AccountFeeResponse, AllowedCw20CodeIdsResponse, CalculateFeeResponse, CancelersResponse,
    ChainResponse, ChainsResponse, ComputeHashResponse, ConfigResponse, DepositInfoResponse,
    FeeConfigResponse, HasCustomFeeResponse, IncomingTokenMappingResponse,
    IncomingTokenMappingsResponse, IsCancelerResponse, LockedBalanceResponse, NonceResponse,
    NonceUsedResponse, OperatorsResponse, PendingAdminResponse, PendingWithdrawResponse,
    PendingWithdrawalEntry, PendingWithdrawalsResponse, PeriodUsageResponse, RateLimitResponse,
    SimulationResponse, StatsResponse, StatusResponse, ThisChainIdResponse,
    TokenDestMappingResponse, TokenResponse, TokenTypeResponse, TokensResponse,
    TransactionResponse, VerifyDepositResponse, WithdrawDelayResponse,
};
use crate::state::{
    ALLOWED_CW20_CODE_IDS, CANCELERS, CHAINS, CONFIG, DEPOSIT_BY_NONCE, DEPOSIT_HASHES,
    LOCKED_BALANCES, OPERATORS, OPERATOR_COUNT, OUTGOING_NONCE, PENDING_ADMIN, PENDING_WITHDRAWS,
    RATE_LIMITS, RATE_LIMIT_PERIOD, RATE_WINDOWS, STATS, THIS_CHAIN_ID, TOKENS,
    TOKEN_DEST_MAPPINGS, TOKEN_SRC_MAPPINGS, TRANSACTIONS, USED_NONCES, WITHDRAW_DELAY,
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
pub fn query_chain(deps: Deps, chain_id: Binary) -> StdResult<ChainResponse> {
    if chain_id.len() != 4 {
        return Err(StdError::generic_err("chain_id must be 4 bytes"));
    }
    let chain = CHAINS.load(deps.storage, &chain_id)?;
    Ok(ChainResponse {
        chain_id: Binary::from(chain.chain_id.to_vec()),
        identifier: chain.identifier,
        identifier_hash: Binary::from(chain.identifier_hash.to_vec()),
        enabled: chain.enabled,
    })
}

/// Query paginated list of chains.
pub fn query_chains(
    deps: Deps,
    start_after: Option<Binary>,
    limit: Option<u32>,
) -> StdResult<ChainsResponse> {
    let limit = limit.unwrap_or(10).min(50) as usize;
    let start: Option<Bound<&[u8]>> = start_after
        .as_ref()
        .map(|id| Bound::exclusive(id.as_slice()));

    let chains: Vec<ChainResponse> = CHAINS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, chain) = item?;
            Ok(ChainResponse {
                chain_id: Binary::from(chain.chain_id.to_vec()),
                identifier: chain.identifier,
                identifier_hash: Binary::from(chain.identifier_hash.to_vec()),
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
        min_bridge_amount: token_config.min_bridge_amount,
        max_bridge_amount: token_config.max_bridge_amount,
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
                min_bridge_amount: token_config.min_bridge_amount,
                max_bridge_amount: token_config.max_bridge_amount,
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
        dest_chain: Binary::from(tx.dest_chain.to_vec()),
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

/// Simulate a bridge operation using V2 fee config.
/// Fee is calculated per depositor (CL8Y discount, custom fees) when depositor is provided.
pub fn query_simulate_bridge(
    deps: Deps,
    token: String,
    amount: Uint128,
    dest_chain: Binary,
    depositor: Option<String>,
) -> StdResult<SimulationResponse> {
    let config = CONFIG.load(deps.storage)?;

    if dest_chain.len() != 4 {
        return Err(StdError::generic_err("dest_chain must be 4 bytes"));
    }
    let _chain = CHAINS.load(deps.storage, &dest_chain)?;
    let _token_config = TOKENS.load(deps.storage, token)?;

    let fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));

    let (fee_amount, fee_bps) = match depositor {
        Some(d) => {
            let addr = deps.api.addr_validate(&d)?;
            let bps = get_effective_fee_bps(deps, &fee_config, &addr)?;
            let fee = calculate_fee_from_bps(amount, bps);
            (fee, bps)
        }
        None => {
            let bps = fee_config.standard_fee_bps;
            let fee = calculate_fee_from_bps(amount, bps);
            (fee, bps)
        }
    };

    let output_amount = amount.checked_sub(fee_amount).unwrap_or(Uint128::zero());

    Ok(SimulationResponse {
        input_amount: amount,
        fee_amount,
        output_amount,
        fee_bps,
    })
}

// ============================================================================
// Watchtower Queries
// ============================================================================

/// Query a V2 pending withdrawal by hash.
pub fn query_pending_withdraw(
    deps: Deps,
    env: Env,
    xchain_hash_id: Binary,
) -> StdResult<PendingWithdrawResponse> {
    let hash_bytes: [u8; 32] = xchain_hash_id
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid hash length"))?;

    let pending = PENDING_WITHDRAWS.may_load(deps.storage, &hash_bytes)?;

    match pending {
        Some(w) => {
            // Calculate cancel window remaining
            let cancel_window_remaining = if w.approved && !w.cancelled {
                let cancel_window = WITHDRAW_DELAY.load(deps.storage).unwrap_or(300u64);
                let elapsed = env.block.time.seconds().saturating_sub(w.approved_at);
                cancel_window.saturating_sub(elapsed)
            } else {
                0
            };

            Ok(PendingWithdrawResponse {
                exists: true,
                src_chain: Binary::from(w.src_chain.to_vec()),
                src_account: Binary::from(w.src_account.to_vec()),
                dest_account: Binary::from(w.dest_account.to_vec()),
                token: w.token,
                recipient: w.recipient,
                amount: w.amount,
                nonce: w.nonce,
                src_decimals: w.src_decimals,
                dest_decimals: w.dest_decimals,
                operator_funds: w.operator_funds.clone(),
                submitted_at: w.submitted_at,
                approved_at: w.approved_at,
                approved: w.approved,
                cancelled: w.cancelled,
                executed: w.executed,
                cancel_window_remaining,
            })
        }
        None => Ok(PendingWithdrawResponse::default()),
    }
}

const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 30;

/// List pending withdrawals with cursor-based pagination.
///
/// Returns all non-executed entries from `PENDING_WITHDRAWS`, ordered by hash.
/// Operators use this to find unapproved submissions to approve.
/// Cancelers use this to find approved-but-not-executed entries to verify.
pub fn query_pending_withdrawals(
    deps: Deps,
    env: Env,
    start_after: Option<Binary>,
    limit: Option<u32>,
) -> StdResult<PendingWithdrawalsResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_ref().map(|b| Bound::exclusive(b.as_slice()));

    let cancel_window = WITHDRAW_DELAY.load(deps.storage).unwrap_or(300u64); // default 5 minutes if not set

    let withdrawals: Vec<PendingWithdrawalEntry> = PENDING_WITHDRAWS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (hash, w) = item?;

            let cancel_window_remaining = if w.approved && !w.cancelled {
                let elapsed = env.block.time.seconds().saturating_sub(w.approved_at);
                cancel_window.saturating_sub(elapsed)
            } else {
                0
            };

            Ok(PendingWithdrawalEntry {
                xchain_hash_id: Binary::from(hash),
                src_chain: Binary::from(w.src_chain.to_vec()),
                src_account: Binary::from(w.src_account.to_vec()),
                dest_account: Binary::from(w.dest_account.to_vec()),
                token: w.token,
                recipient: w.recipient,
                amount: w.amount,
                nonce: w.nonce,
                src_decimals: w.src_decimals,
                dest_decimals: w.dest_decimals,
                operator_funds: w.operator_funds.clone(),
                submitted_at: w.submitted_at,
                approved_at: w.approved_at,
                approved: w.approved,
                cancelled: w.cancelled,
                executed: w.executed,
                cancel_window_remaining,
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(PendingWithdrawalsResponse { withdrawals })
}

/// Compute a unified V2 cross-chain hash ID from 7-field parameters.
pub fn query_compute_xchain_hash_id(
    src_chain: Binary,
    dest_chain: Binary,
    src_account: Binary,
    dest_account: Binary,
    token: Binary,
    amount: Uint128,
    nonce: u64,
) -> StdResult<ComputeHashResponse> {
    let src_chain_bytes: [u8; 4] = src_chain
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid src_chain length, expected 4 bytes"))?;
    let dest_chain_bytes: [u8; 4] = dest_chain
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid dest_chain length, expected 4 bytes"))?;
    let src_account_bytes: [u8; 32] = src_account
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid src_account length"))?;
    let dest_account_bytes: [u8; 32] = dest_account
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid dest_account length"))?;
    let token_bytes: [u8; 32] = token
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid token length"))?;

    let hash = compute_xchain_hash_id(
        &src_chain_bytes,
        &dest_chain_bytes,
        &src_account_bytes,
        &dest_account_bytes,
        &token_bytes,
        amount.u128(),
        nonce,
    );

    Ok(ComputeHashResponse {
        hash: Binary::from(hash.to_vec()),
    })
}

// ============================================================================
// Deposit Queries
// ============================================================================

/// Query a deposit by hash.
pub fn query_xchain_hash_id(
    deps: Deps,
    xchain_hash_id: Binary,
) -> StdResult<Option<DepositInfoResponse>> {
    let hash_bytes: [u8; 32] = xchain_hash_id
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid hash length"))?;

    let deposit = DEPOSIT_HASHES.may_load(deps.storage, &hash_bytes)?;

    Ok(deposit.map(|d| DepositInfoResponse {
        xchain_hash_id: Binary::from(hash_bytes.to_vec()),
        src_chain: Binary::from(d.src_chain.to_vec()),
        dest_chain: Binary::from(d.dest_chain.to_vec()),
        src_account: Binary::from(d.src_account.to_vec()),
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
                xchain_hash_id: Binary::from(hash_bytes.to_vec()),
                src_chain: Binary::from(d.src_chain.to_vec()),
                dest_chain: Binary::from(d.dest_chain.to_vec()),
                src_account: Binary::from(d.src_account.to_vec()),
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
    xchain_hash_id: Binary,
    dest_token_address: Binary,
    dest_account: Binary,
    amount: Uint128,
    nonce: u64,
) -> StdResult<VerifyDepositResponse> {
    let hash_bytes: [u8; 32] = xchain_hash_id
        .to_vec()
        .try_into()
        .map_err(|_| StdError::generic_err("Invalid hash length"))?;

    let deposit = DEPOSIT_HASHES.may_load(deps.storage, &hash_bytes)?;

    match deposit {
        Some(d) => {
            // Verify parameters match
            let token_match = dest_token_address.to_vec() == d.dest_token_address.to_vec();
            let account_match = dest_account.to_vec() == d.dest_account.to_vec();
            let amount_match = amount == d.amount;
            let nonce_match = nonce == d.nonce;

            let matches = token_match && account_match && amount_match && nonce_match;

            Ok(VerifyDepositResponse {
                exists: true,
                matches,
                deposit: Some(DepositInfoResponse {
                    xchain_hash_id: Binary::from(hash_bytes.to_vec()),
                    src_chain: Binary::from(d.src_chain.to_vec()),
                    dest_chain: Binary::from(d.dest_chain.to_vec()),
                    src_account: Binary::from(d.src_account.to_vec()),
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

/// Query this chain's predetermined 4-byte V2 chain ID (set at instantiation).
pub fn query_this_chain_id(deps: Deps) -> StdResult<ThisChainIdResponse> {
    let chain_id = THIS_CHAIN_ID.load(deps.storage)?;
    Ok(ThisChainIdResponse {
        chain_id: Binary::from(chain_id.to_vec()),
    })
}

/// Query the withdraw delay.
pub fn query_withdraw_delay(deps: Deps) -> StdResult<WithdrawDelayResponse> {
    let delay = WITHDRAW_DELAY.load(deps.storage)?;
    Ok(WithdrawDelayResponse {
        delay_seconds: delay,
    })
}

/// Query allowed CW20 code IDs for token registration.
/// Empty list = no restriction (any CW20 allowed).
pub fn query_allowed_cw20_code_ids(deps: Deps) -> StdResult<AllowedCw20CodeIdsResponse> {
    let code_ids = ALLOWED_CW20_CODE_IDS
        .may_load(deps.storage)?
        .unwrap_or_default();
    Ok(AllowedCw20CodeIdsResponse { code_ids })
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

// ============================================================================
// Fee Queries (V2)
// ============================================================================

/// Query fee configuration.
pub fn query_fee_config(deps: Deps) -> StdResult<FeeConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));

    Ok(FeeConfigResponse {
        standard_fee_bps: fee_config.standard_fee_bps,
        discounted_fee_bps: fee_config.discounted_fee_bps,
        cl8y_threshold: fee_config.cl8y_threshold,
        cl8y_token: fee_config.cl8y_token,
        fee_recipient: fee_config.fee_recipient,
    })
}

/// Query account fee info.
pub fn query_account_fee(deps: Deps, account: String) -> StdResult<AccountFeeResponse> {
    let account_addr = deps.api.addr_validate(&account)?;
    let config = CONFIG.load(deps.storage)?;
    let fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));

    let fee_bps = get_effective_fee_bps(deps, &fee_config, &account_addr)?;
    let fee_type = get_fee_type(deps, &fee_config, &account_addr)?;

    Ok(AccountFeeResponse {
        account: account_addr,
        fee_bps,
        fee_type: fee_type.as_str().to_string(),
    })
}

/// Check if account has custom fee.
pub fn query_has_custom_fee(deps: Deps, account: String) -> StdResult<HasCustomFeeResponse> {
    let account_addr = deps.api.addr_validate(&account)?;
    let has_custom = has_custom_fee(deps, &account_addr)?;

    Ok(HasCustomFeeResponse {
        has_custom_fee: has_custom,
    })
}

/// Calculate fee for a specific depositor and amount.
pub fn query_calculate_fee(
    deps: Deps,
    depositor: String,
    amount: Uint128,
) -> StdResult<CalculateFeeResponse> {
    let depositor_addr = deps.api.addr_validate(&depositor)?;
    let config = CONFIG.load(deps.storage)?;
    let fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));

    let fee_amount = calculate_fee(deps, &fee_config, &depositor_addr, amount)?;
    let fee_bps = get_effective_fee_bps(deps, &fee_config, &depositor_addr)?;
    let fee_type = get_fee_type(deps, &fee_config, &depositor_addr)?;

    Ok(CalculateFeeResponse {
        depositor: depositor_addr,
        amount,
        fee_amount,
        fee_bps,
        fee_type: fee_type.as_str().to_string(),
    })
}

// ============================================================================
// Token Registry Queries (V2)
// ============================================================================

/// Query token type.
pub fn query_token_type(deps: Deps, token: String) -> StdResult<TokenTypeResponse> {
    let token_config = TOKENS.load(deps.storage, token.clone())?;

    Ok(TokenTypeResponse {
        token,
        token_type: token_config.token_type.as_str().to_string(),
    })
}

/// Query token destination mapping.
pub fn query_token_dest_mapping(
    deps: Deps,
    token: String,
    dest_chain: Binary,
) -> StdResult<Option<TokenDestMappingResponse>> {
    if dest_chain.len() != 4 {
        return Err(StdError::generic_err("dest_chain must be 4 bytes"));
    }
    let chain_key = hex::encode(&dest_chain);
    let mapping = TOKEN_DEST_MAPPINGS.may_load(deps.storage, (&token, &chain_key))?;

    Ok(mapping.map(|m| TokenDestMappingResponse {
        token,
        dest_chain: dest_chain.clone(),
        dest_token: Binary::from(m.dest_token.to_vec()),
        dest_decimals: m.dest_decimals,
    }))
}

// ============================================================================
// Incoming Token Registry Queries
// ============================================================================

/// Query a single incoming token mapping by source chain and token.
pub fn query_incoming_token_mapping(
    deps: Deps,
    src_chain: Binary,
    src_token: Binary,
) -> StdResult<Option<IncomingTokenMappingResponse>> {
    if src_chain.len() != 4 {
        return Err(StdError::generic_err("src_chain must be 4 bytes"));
    }
    if src_token.len() != 32 {
        return Err(StdError::generic_err("src_token must be 32 bytes"));
    }

    let src_chain_key = hex::encode(&src_chain);
    let src_token_key = hex::encode(&src_token);

    let mapping = TOKEN_SRC_MAPPINGS.may_load(deps.storage, (&src_chain_key, &src_token_key))?;

    Ok(mapping.map(|m| IncomingTokenMappingResponse {
        src_chain,
        src_token,
        local_token: m.local_token,
        src_decimals: m.src_decimals,
        enabled: m.enabled,
    }))
}

/// List all incoming token mappings (paginated).
///
/// Pagination cursor uses a composite key: "src_chain_hex:src_token_hex"
pub fn query_incoming_token_mappings(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<IncomingTokenMappingsResponse> {
    let limit = limit.unwrap_or(30).min(100) as usize;

    // Parse cursor into owned strings for the bound
    let start_pair: Option<(String, String)> = start_after.and_then(|cursor| {
        let parts: Vec<&str> = cursor.split(':').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    });

    let min_bound: Option<Bound<(&str, &str)>> = start_pair
        .as_ref()
        .map(|(c, t)| Bound::exclusive((c.as_str(), t.as_str())));

    let mappings: Vec<IncomingTokenMappingResponse> = TOKEN_SRC_MAPPINGS
        .range(deps.storage, min_bound, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let ((chain_hex, token_hex), mapping) = item?;
            let chain_bytes = hex::decode(&chain_hex).unwrap_or_default();
            let token_bytes = hex::decode(&token_hex).unwrap_or_default();
            Ok(IncomingTokenMappingResponse {
                src_chain: Binary::from(chain_bytes),
                src_token: Binary::from(token_bytes),
                local_token: mapping.local_token,
                src_decimals: mapping.src_decimals,
                enabled: mapping.enabled,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(IncomingTokenMappingsResponse { mappings })
}
