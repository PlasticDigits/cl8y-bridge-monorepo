//! CL8Y Bridge Contract Implementation
//!
//! This contract implements the watchtower security pattern for cross-chain bridging.

use cosmwasm_std::{
    entry_point, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Order, Response, StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::hash::{
    bytes32_to_hex, compute_transfer_id, encode_token_address, evm_chain_key, hex_to_bytes32,
    terra_chain_key,
};
use crate::msg::{
    CancelersResponse, ChainResponse, ChainsResponse, ComputeHashResponse, ConfigResponse,
    DepositInfoResponse, ExecuteMsg, InstantiateMsg, IsCancelerResponse, LockedBalanceResponse,
    MigrateMsg, NonceResponse, NonceUsedResponse, OperatorsResponse, PendingAdminResponse,
    PeriodUsageResponse, QueryMsg, RateLimitResponse, ReceiveMsg, SimulationResponse,
    StatsResponse, StatusResponse, TokenResponse, TokensResponse, TransactionResponse,
    VerifyDepositResponse, WithdrawApprovalResponse, WithdrawDelayResponse,
};
use crate::state::{
    BridgeTransaction, ChainConfig, Config, DepositInfo, PendingAdmin, RateLimitConfig,
    RateLimitWindow, Stats, TokenConfig, WithdrawApproval, ADMIN_TIMELOCK_DURATION, CANCELERS,
    CHAINS, CONFIG, CONTRACT_NAME, CONTRACT_VERSION, DEFAULT_WITHDRAW_DELAY, DEPOSIT_BY_NONCE,
    DEPOSIT_HASHES, LOCKED_BALANCES, OPERATORS, OPERATOR_COUNT, OUTGOING_NONCE, PENDING_ADMIN,
    RATE_LIMITS, RATE_LIMIT_PERIOD, RATE_WINDOWS, STATS, TOKENS, TRANSACTIONS, USED_NONCES,
    WITHDRAW_APPROVALS, WITHDRAW_DELAY, WITHDRAW_NONCE_USED,
};
use common::AssetInfo;

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
            evm_token_address,
            terra_decimals,
            evm_decimals,
        } => execute_add_token(
            deps,
            info,
            token,
            is_native,
            evm_token_address,
            terra_decimals,
            evm_decimals,
        ),
        ExecuteMsg::UpdateToken {
            token,
            evm_token_address,
            enabled,
        } => execute_update_token(deps, info, token, evm_token_address, enabled),

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
// Outgoing Transfer Handlers
// ============================================================================

fn execute_lock_native(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dest_chain_id: u64,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    // Check destination chain
    let chain_key = dest_chain_id.to_string();
    let chain = CHAINS
        .may_load(deps.storage, chain_key.clone())?
        .ok_or(ContractError::ChainNotSupported {
            chain_id: dest_chain_id,
        })?;

    if !chain.enabled {
        return Err(ContractError::ChainNotSupported {
            chain_id: dest_chain_id,
        });
    }

    // Validate funds
    if info.funds.is_empty() {
        return Err(ContractError::NoFundsSent);
    }

    if info.funds.len() > 1 {
        return Err(ContractError::InvalidAmount {
            reason: "Only one token type allowed per transaction".to_string(),
        });
    }

    let coin = &info.funds[0];
    let token = coin.denom.clone();
    let amount = coin.amount;

    // Check token is supported
    let token_config = TOKENS
        .may_load(deps.storage, token.clone())?
        .ok_or(ContractError::TokenNotSupported {
            token: token.clone(),
        })?;

    if !token_config.enabled {
        return Err(ContractError::TokenNotSupported {
            token: token.clone(),
        });
    }

    // Validate amount
    if amount < config.min_bridge_amount {
        return Err(ContractError::BelowMinimumAmount {
            min_amount: config.min_bridge_amount.to_string(),
        });
    }

    if amount > config.max_bridge_amount {
        return Err(ContractError::AboveMaximumAmount {
            max_amount: config.max_bridge_amount.to_string(),
        });
    }

    // Calculate fee
    let fee_amount = amount.multiply_ratio(config.fee_bps as u128, 10000u128);
    let net_amount = amount - fee_amount;

    // Update locked balance
    let current_locked = LOCKED_BALANCES
        .may_load(deps.storage, token.clone())?
        .unwrap_or(Uint128::zero());
    LOCKED_BALANCES.save(deps.storage, token.clone(), &(current_locked + net_amount))?;

    // Increment nonce
    let nonce = OUTGOING_NONCE.load(deps.storage)?;
    OUTGOING_NONCE.save(deps.storage, &(nonce + 1))?;

    // Store transaction
    let tx = BridgeTransaction {
        nonce,
        sender: info.sender.to_string(),
        recipient: recipient.clone(),
        token: token.clone(),
        amount: net_amount,
        dest_chain_id,
        timestamp: env.block.time,
        is_outgoing: true,
    };
    TRANSACTIONS.save(deps.storage, nonce, &tx)?;

    // Compute and store deposit hash for verification
    let dest_chain_key = evm_chain_key(dest_chain_id);
    let dest_token_address = hex_to_bytes32(&token_config.evm_token_address)
        .map_err(|e| ContractError::InvalidAddress { reason: e.to_string() })?;
    let dest_account = hex_to_bytes32(&recipient)
        .map_err(|e| ContractError::InvalidAddress { reason: e.to_string() })?;

    let deposit_info = DepositInfo {
        dest_chain_key,
        dest_token_address,
        dest_account,
        amount: net_amount,
        nonce,
        deposited_at: env.block.time,
    };

    let deposit_hash = compute_transfer_id(
        &terra_chain_key(),
        &dest_chain_key,
        &dest_token_address,
        &dest_account,
        net_amount.u128(),
        nonce,
    );

    DEPOSIT_HASHES.save(deps.storage, &deposit_hash, &deposit_info)?;
    DEPOSIT_BY_NONCE.save(deps.storage, nonce, &deposit_hash)?;

    // Update stats
    let mut stats = STATS.load(deps.storage)?;
    stats.total_outgoing_txs += 1;
    stats.total_fees_collected += fee_amount;
    STATS.save(deps.storage, &stats)?;

    // Send fee to collector
    let mut messages: Vec<CosmosMsg> = vec![];
    if !fee_amount.is_zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: config.fee_collector.to_string(),
            amount: vec![Coin {
                denom: token.clone(),
                amount: fee_amount,
            }],
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "lock")
        .add_attribute("nonce", nonce.to_string())
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token", token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("dest_chain_id", dest_chain_id.to_string())
        .add_attribute("deposit_hash", bytes32_to_hex(&deposit_hash)))
}

fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    let token = info.sender.to_string();
    let amount = cw20_msg.amount;
    let sender = deps.api.addr_validate(&cw20_msg.sender)?;

    let receive_msg: ReceiveMsg = cosmwasm_std::from_json(&cw20_msg.msg)?;

    match receive_msg {
        ReceiveMsg::Lock {
            dest_chain_id,
            recipient,
        } => {
            // Check destination chain
            let chain_key = dest_chain_id.to_string();
            let chain = CHAINS
                .may_load(deps.storage, chain_key.clone())?
                .ok_or(ContractError::ChainNotSupported {
                    chain_id: dest_chain_id,
                })?;

            if !chain.enabled {
                return Err(ContractError::ChainNotSupported {
                    chain_id: dest_chain_id,
                });
            }

            // Check token
            let token_config = TOKENS
                .may_load(deps.storage, token.clone())?
                .ok_or(ContractError::TokenNotSupported {
                    token: token.clone(),
                })?;

            if !token_config.enabled {
                return Err(ContractError::TokenNotSupported {
                    token: token.clone(),
                });
            }

            // Validate amount
            if amount < config.min_bridge_amount {
                return Err(ContractError::BelowMinimumAmount {
                    min_amount: config.min_bridge_amount.to_string(),
                });
            }

            if amount > config.max_bridge_amount {
                return Err(ContractError::AboveMaximumAmount {
                    max_amount: config.max_bridge_amount.to_string(),
                });
            }

            // Calculate fee
            let fee_amount = amount.multiply_ratio(config.fee_bps as u128, 10000u128);
            let net_amount = amount - fee_amount;

            // Update locked balance
            let current_locked = LOCKED_BALANCES
                .may_load(deps.storage, token.clone())?
                .unwrap_or(Uint128::zero());
            LOCKED_BALANCES.save(deps.storage, token.clone(), &(current_locked + net_amount))?;

            // Increment nonce
            let nonce = OUTGOING_NONCE.load(deps.storage)?;
            OUTGOING_NONCE.save(deps.storage, &(nonce + 1))?;

            // Store transaction
            let tx = BridgeTransaction {
                nonce,
                sender: sender.to_string(),
                recipient: recipient.clone(),
                token: token.clone(),
                amount: net_amount,
                dest_chain_id,
                timestamp: env.block.time,
                is_outgoing: true,
            };
            TRANSACTIONS.save(deps.storage, nonce, &tx)?;

            // Compute and store deposit hash
            let dest_chain_key = evm_chain_key(dest_chain_id);
            let dest_token_address = hex_to_bytes32(&token_config.evm_token_address)
                .map_err(|e| ContractError::InvalidAddress { reason: e.to_string() })?;
            let dest_account = hex_to_bytes32(&recipient)
                .map_err(|e| ContractError::InvalidAddress { reason: e.to_string() })?;

            let deposit_info = DepositInfo {
                dest_chain_key,
                dest_token_address,
                dest_account,
                amount: net_amount,
                nonce,
                deposited_at: env.block.time,
            };

            let deposit_hash = compute_transfer_id(
                &terra_chain_key(),
                &dest_chain_key,
                &dest_token_address,
                &dest_account,
                net_amount.u128(),
                nonce,
            );

            DEPOSIT_HASHES.save(deps.storage, &deposit_hash, &deposit_info)?;
            DEPOSIT_BY_NONCE.save(deps.storage, nonce, &deposit_hash)?;

            // Update stats
            let mut stats = STATS.load(deps.storage)?;
            stats.total_outgoing_txs += 1;
            stats.total_fees_collected += fee_amount;
            STATS.save(deps.storage, &stats)?;

            // Send fee to collector
            let mut messages: Vec<CosmosMsg> = vec![];
            if !fee_amount.is_zero() {
                messages.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
                    contract_addr: token.clone(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: config.fee_collector.to_string(),
                        amount: fee_amount,
                    })?,
                    funds: vec![],
                }));
            }

            Ok(Response::new()
                .add_messages(messages)
                .add_attribute("method", "lock_cw20")
                .add_attribute("nonce", nonce.to_string())
                .add_attribute("sender", sender)
                .add_attribute("recipient", recipient)
                .add_attribute("token", token)
                .add_attribute("amount", net_amount.to_string())
                .add_attribute("fee", fee_amount.to_string())
                .add_attribute("dest_chain_id", dest_chain_id.to_string())
                .add_attribute("deposit_hash", bytes32_to_hex(&deposit_hash)))
        }
    }
}

// ============================================================================
// Watchtower Pattern Handlers
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn execute_approve_withdraw(
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
    let src_chain_key_bytes: [u8; 32] = src_chain_key
        .to_vec()
        .try_into()
        .map_err(|_| ContractError::InvalidHashLength {
            got: src_chain_key.len(),
        })?;

    let dest_account_bytes: [u8; 32] = dest_account
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

fn execute_execute_withdraw(
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
    let hash_bytes: [u8; 32] = withdraw_hash
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

fn execute_cancel_withdraw_approval(
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
    let hash_bytes: [u8; 32] = withdraw_hash
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

fn execute_reenable_withdraw_approval(
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
    let hash_bytes: [u8; 32] = withdraw_hash
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
// Rate Limiting
// ============================================================================

fn check_and_update_rate_limit(
    storage: &mut dyn cosmwasm_std::Storage,
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

// ============================================================================
// Canceler Management
// ============================================================================

fn execute_add_canceler(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let canceler_addr = deps.api.addr_validate(&address)?;
    CANCELERS.save(deps.storage, &canceler_addr, &true)?;

    Ok(Response::new()
        .add_attribute("method", "add_canceler")
        .add_attribute("canceler", address))
}

fn execute_remove_canceler(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let canceler_addr = deps.api.addr_validate(&address)?;
    CANCELERS.remove(deps.storage, &canceler_addr);

    Ok(Response::new()
        .add_attribute("method", "remove_canceler")
        .add_attribute("canceler", address))
}

// ============================================================================
// Configuration Handlers
// ============================================================================

fn execute_set_withdraw_delay(
    deps: DepsMut,
    info: MessageInfo,
    delay_seconds: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    // Validate range (60 seconds to 24 hours)
    if delay_seconds < 60 || delay_seconds > 86400 {
        return Err(ContractError::InvalidWithdrawDelay);
    }

    WITHDRAW_DELAY.save(deps.storage, &delay_seconds)?;

    Ok(Response::new()
        .add_attribute("method", "set_withdraw_delay")
        .add_attribute("delay_seconds", delay_seconds.to_string()))
}

fn execute_set_rate_limit(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    max_per_transaction: Uint128,
    max_per_period: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let rate_config = RateLimitConfig {
        max_per_transaction,
        max_per_period,
    };
    RATE_LIMITS.save(deps.storage, &token, &rate_config)?;

    Ok(Response::new()
        .add_attribute("method", "set_rate_limit")
        .add_attribute("token", token)
        .add_attribute("max_per_transaction", max_per_transaction.to_string())
        .add_attribute("max_per_period", max_per_period.to_string()))
}

// ============================================================================
// Chain & Token Management
// ============================================================================

fn execute_add_chain(
    deps: DepsMut,
    info: MessageInfo,
    chain_id: u64,
    name: String,
    bridge_address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let chain_key = chain_id.to_string();
    let chain = ChainConfig {
        chain_id,
        name: name.clone(),
        bridge_address,
        enabled: true,
    };
    CHAINS.save(deps.storage, chain_key, &chain)?;

    Ok(Response::new()
        .add_attribute("method", "add_chain")
        .add_attribute("chain_id", chain_id.to_string())
        .add_attribute("name", name))
}

fn execute_update_chain(
    deps: DepsMut,
    info: MessageInfo,
    chain_id: u64,
    name: Option<String>,
    bridge_address: Option<String>,
    enabled: Option<bool>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let chain_key = chain_id.to_string();
    let mut chain = CHAINS
        .may_load(deps.storage, chain_key.clone())?
        .ok_or(ContractError::ChainNotSupported { chain_id })?;

    if let Some(n) = name {
        chain.name = n;
    }
    if let Some(addr) = bridge_address {
        chain.bridge_address = addr;
    }
    if let Some(e) = enabled {
        chain.enabled = e;
    }

    CHAINS.save(deps.storage, chain_key, &chain)?;

    Ok(Response::new()
        .add_attribute("method", "update_chain")
        .add_attribute("chain_id", chain_id.to_string()))
}

fn execute_add_token(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    is_native: bool,
    evm_token_address: String,
    terra_decimals: u8,
    evm_decimals: u8,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let token_config = TokenConfig {
        token: token.clone(),
        is_native,
        evm_token_address,
        terra_decimals,
        evm_decimals,
        enabled: true,
    };
    TOKENS.save(deps.storage, token.clone(), &token_config)?;

    Ok(Response::new()
        .add_attribute("method", "add_token")
        .add_attribute("token", token)
        .add_attribute("is_native", is_native.to_string()))
}

fn execute_update_token(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    evm_token_address: Option<String>,
    enabled: Option<bool>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let mut token_config = TOKENS
        .may_load(deps.storage, token.clone())?
        .ok_or(ContractError::TokenNotSupported {
            token: token.clone(),
        })?;

    if let Some(addr) = evm_token_address {
        token_config.evm_token_address = addr;
    }
    if let Some(e) = enabled {
        token_config.enabled = e;
    }

    TOKENS.save(deps.storage, token.clone(), &token_config)?;

    Ok(Response::new()
        .add_attribute("method", "update_token")
        .add_attribute("token", token))
}

// ============================================================================
// Operator Management
// ============================================================================

fn execute_add_operator(
    deps: DepsMut,
    info: MessageInfo,
    operator: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let operator_addr = deps.api.addr_validate(&operator)?;
    let existing = OPERATORS
        .may_load(deps.storage, &operator_addr)?
        .unwrap_or(false);
    if existing {
        return Err(ContractError::OperatorAlreadyRegistered);
    }

    OPERATORS.save(deps.storage, &operator_addr, &true)?;
    let count = OPERATOR_COUNT.load(deps.storage)?;
    OPERATOR_COUNT.save(deps.storage, &(count + 1))?;

    Ok(Response::new()
        .add_attribute("method", "add_operator")
        .add_attribute("operator", operator))
}

fn execute_remove_operator(
    deps: DepsMut,
    info: MessageInfo,
    operator: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let operator_addr = deps.api.addr_validate(&operator)?;
    let existing = OPERATORS
        .may_load(deps.storage, &operator_addr)?
        .unwrap_or(false);
    if !existing {
        return Err(ContractError::OperatorNotRegistered);
    }

    let count = OPERATOR_COUNT.load(deps.storage)?;
    if count <= 1 {
        return Err(ContractError::CannotRemoveLastOperator);
    }
    if count <= config.min_signatures {
        return Err(ContractError::InsufficientSignatures {
            got: count - 1,
            required: config.min_signatures,
        });
    }

    OPERATORS.remove(deps.storage, &operator_addr);
    OPERATOR_COUNT.save(deps.storage, &(count - 1))?;

    Ok(Response::new()
        .add_attribute("method", "remove_operator")
        .add_attribute("operator", operator))
}

fn execute_update_min_signatures(
    deps: DepsMut,
    info: MessageInfo,
    min_signatures: u32,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let count = OPERATOR_COUNT.load(deps.storage)?;
    if min_signatures == 0 || min_signatures > count {
        return Err(ContractError::InsufficientSignatures {
            got: count,
            required: min_signatures,
        });
    }

    config.min_signatures = min_signatures;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "update_min_signatures")
        .add_attribute("min_signatures", min_signatures.to_string()))
}

// ============================================================================
// Bridge Configuration
// ============================================================================

fn execute_update_limits(
    deps: DepsMut,
    info: MessageInfo,
    min_bridge_amount: Option<Uint128>,
    max_bridge_amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    if let Some(min) = min_bridge_amount {
        config.min_bridge_amount = min;
    }
    if let Some(max) = max_bridge_amount {
        config.max_bridge_amount = max;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "update_limits")
        .add_attribute("min_bridge_amount", config.min_bridge_amount.to_string())
        .add_attribute("max_bridge_amount", config.max_bridge_amount.to_string()))
}

fn execute_update_fees(
    deps: DepsMut,
    info: MessageInfo,
    fee_bps: Option<u32>,
    fee_collector: Option<String>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    if let Some(bps) = fee_bps {
        config.fee_bps = bps;
    }
    if let Some(collector) = fee_collector {
        config.fee_collector = deps.api.addr_validate(&collector)?;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "update_fees")
        .add_attribute("fee_bps", config.fee_bps.to_string())
        .add_attribute("fee_collector", config.fee_collector.to_string()))
}

// ============================================================================
// Admin Operations
// ============================================================================

fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    config.paused = true;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("method", "pause"))
}

fn execute_unpause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    config.paused = false;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("method", "unpause"))
}

fn execute_propose_admin(
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
        .add_attribute(
            "execute_after",
            pending.execute_after.seconds().to_string(),
        ))
}

fn execute_accept_admin(
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

fn execute_cancel_admin_proposal(
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

fn execute_recover_asset(
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
                msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
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
    }
}

// Core query implementations

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
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

fn query_status(deps: Deps) -> StdResult<StatusResponse> {
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

fn query_stats(deps: Deps) -> StdResult<StatsResponse> {
    let stats = STATS.load(deps.storage)?;
    Ok(StatsResponse {
        total_outgoing_txs: stats.total_outgoing_txs,
        total_incoming_txs: stats.total_incoming_txs,
        total_fees_collected: stats.total_fees_collected,
    })
}

fn query_chain(deps: Deps, chain_id: u64) -> StdResult<ChainResponse> {
    let chain_key = chain_id.to_string();
    let chain = CHAINS.load(deps.storage, chain_key)?;
    Ok(ChainResponse {
        chain_id: chain.chain_id,
        name: chain.name,
        bridge_address: chain.bridge_address,
        enabled: chain.enabled,
    })
}

fn query_chains(
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

fn query_token(deps: Deps, token: String) -> StdResult<TokenResponse> {
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

fn query_tokens(
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

fn query_operators(deps: Deps) -> StdResult<OperatorsResponse> {
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

fn query_nonce_used(deps: Deps, nonce: u64) -> StdResult<NonceUsedResponse> {
    let used = USED_NONCES.may_load(deps.storage, nonce)?.unwrap_or(false);
    Ok(NonceUsedResponse { nonce, used })
}

fn query_current_nonce(deps: Deps) -> StdResult<NonceResponse> {
    let nonce = OUTGOING_NONCE.load(deps.storage)?;
    Ok(NonceResponse { nonce })
}

fn query_transaction(deps: Deps, nonce: u64) -> StdResult<TransactionResponse> {
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

fn query_locked_balance(deps: Deps, token: String) -> StdResult<LockedBalanceResponse> {
    let amount = LOCKED_BALANCES
        .may_load(deps.storage, token.clone())?
        .unwrap_or(Uint128::zero());
    Ok(LockedBalanceResponse { token, amount })
}

fn query_pending_admin(deps: Deps) -> StdResult<Option<PendingAdminResponse>> {
    let pending = PENDING_ADMIN.may_load(deps.storage)?;
    Ok(pending.map(|p| PendingAdminResponse {
        new_address: p.new_address,
        execute_after: p.execute_after,
    }))
}

fn query_simulate_bridge(
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

// Watchtower query implementations

fn query_withdraw_approval(
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

fn query_compute_withdraw_hash(
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

fn query_deposit_hash(deps: Deps, deposit_hash: Binary) -> StdResult<Option<DepositInfoResponse>> {
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

fn query_deposit_by_nonce(deps: Deps, nonce: u64) -> StdResult<Option<DepositInfoResponse>> {
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

fn query_verify_deposit(
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

            let matches = dest_key_match && token_match && account_match && amount_match && nonce_match;

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

// Canceler query implementations

fn query_cancelers(deps: Deps) -> StdResult<CancelersResponse> {
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

fn query_is_canceler(deps: Deps, address: String) -> StdResult<IsCancelerResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let is_canceler = CANCELERS.may_load(deps.storage, &addr)?.unwrap_or(false);
    Ok(IsCancelerResponse { is_canceler })
}

// Configuration query implementations

fn query_withdraw_delay(deps: Deps) -> StdResult<WithdrawDelayResponse> {
    let delay = WITHDRAW_DELAY.load(deps.storage)?;
    Ok(WithdrawDelayResponse {
        delay_seconds: delay,
    })
}

fn query_rate_limit(deps: Deps, token: String) -> StdResult<Option<RateLimitResponse>> {
    let config = RATE_LIMITS.may_load(deps.storage, &token)?;
    Ok(config.map(|c| RateLimitResponse {
        token,
        max_per_transaction: c.max_per_transaction,
        max_per_period: c.max_per_period,
    }))
}

fn query_period_usage(deps: Deps, env: Env, token: String) -> StdResult<PeriodUsageResponse> {
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

    let max_per_period = config
        .map(|c| c.max_per_period)
        .unwrap_or(Uint128::zero());

    let remaining = if max_per_period.is_zero() {
        Uint128::MAX // Unlimited
    } else if used >= max_per_period {
        Uint128::zero()
    } else {
        max_per_period - used
    };

    let period_ends_at = cosmwasm_std::Timestamp::from_seconds(
        window_start.seconds() + RATE_LIMIT_PERIOD,
    );

    Ok(PeriodUsageResponse {
        token,
        current_period_start: window_start,
        used_amount: used,
        remaining_amount: remaining,
        period_ends_at,
    })
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

    Ok(Response::new()
        .add_attribute("method", "migrate")
        .add_attribute("version", CONTRACT_VERSION)
        .add_attribute("withdraw_delay", DEFAULT_WITHDRAW_DELAY.to_string()))
}
