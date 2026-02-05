//! Outgoing transfer handlers (Lock, Burn, and Receive).
//!
//! These handlers process tokens being locked or burned on Terra for bridging to other chains.
//! Integrates with fee_manager for CL8Y holder discounts and custom account fees.

use cosmwasm_std::{
    to_json_binary, BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, Uint128,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::error::ContractError;
use crate::fee_manager::{calculate_fee, get_fee_type, FeeConfig, FEE_CONFIG};
use crate::hash::{
    bytes32_to_hex, compute_transfer_id, evm_chain_key, hex_to_bytes32, terra_chain_key,
};
use crate::msg::ReceiveMsg;
use crate::state::{
    BridgeTransaction, DepositInfo, TokenType, CHAINS, CONFIG, DEPOSIT_BY_NONCE, DEPOSIT_HASHES,
    LOCKED_BALANCES, OUTGOING_NONCE, STATS, TOKENS, TRANSACTIONS,
};

/// Execute handler for locking native tokens (uluna, etc.)
pub fn execute_lock_native(
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
    let chain = CHAINS.may_load(deps.storage, chain_key.clone())?.ok_or(
        ContractError::ChainNotSupported {
            chain_id: dest_chain_id,
        },
    )?;

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

    // Calculate fee using V2 fee manager (with CL8Y discount and custom fees)
    let fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));
    let fee_amount = calculate_fee(deps.as_ref(), &fee_config, &info.sender, amount)?;
    let net_amount = amount - fee_amount;
    let fee_type = get_fee_type(deps.as_ref(), &fee_config, &info.sender)?;

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
    let dest_token_address = hex_to_bytes32(&token_config.evm_token_address).map_err(|e| {
        ContractError::InvalidAddress {
            reason: e.to_string(),
        }
    })?;
    let dest_account = hex_to_bytes32(&recipient).map_err(|e| ContractError::InvalidAddress {
        reason: e.to_string(),
    })?;

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

    // Send fee to collector (use fee_config.fee_recipient if available)
    let fee_recipient = fee_config.fee_recipient.to_string();
    let mut messages: Vec<CosmosMsg> = vec![];
    if !fee_amount.is_zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            to_address: fee_recipient.clone(),
            amount: vec![Coin {
                denom: token.clone(),
                amount: fee_amount,
            }],
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "deposit_native")
        .add_attribute("nonce", nonce.to_string())
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token", token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("fee_type", fee_type.as_str())
        .add_attribute("dest_chain_id", dest_chain_id.to_string())
        .add_attribute("deposit_hash", bytes32_to_hex(&deposit_hash)))
}

/// Execute handler for receiving CW20 tokens to lock or burn
pub fn execute_receive(
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
        } => execute_cw20_lock(
            deps,
            env,
            config,
            token,
            amount,
            sender,
            dest_chain_id,
            recipient,
        ),
        ReceiveMsg::Burn {
            dest_chain_id,
            recipient,
        } => execute_cw20_burn(
            deps,
            env,
            config,
            token,
            amount,
            sender,
            dest_chain_id,
            recipient,
        ),
    }
}

/// Internal handler for locking CW20 tokens (LockUnlock mode)
fn execute_cw20_lock(
    deps: DepsMut,
    env: Env,
    config: crate::state::Config,
    token: String,
    amount: Uint128,
    sender: cosmwasm_std::Addr,
    dest_chain_id: u64,
    recipient: String,
) -> Result<Response, ContractError> {
    // Check destination chain
    let chain_key = dest_chain_id.to_string();
    let chain = CHAINS.may_load(deps.storage, chain_key.clone())?.ok_or(
        ContractError::ChainNotSupported {
            chain_id: dest_chain_id,
        },
    )?;

    if !chain.enabled {
        return Err(ContractError::ChainNotSupported {
            chain_id: dest_chain_id,
        });
    }

    // Check token
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

    // Verify token type is LockUnlock
    if !matches!(token_config.token_type, TokenType::LockUnlock) {
        return Err(ContractError::InvalidTokenType {
            expected: "lock_unlock".to_string(),
            actual: token_config.token_type.as_str().to_string(),
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

    // Calculate fee using V2 fee manager
    let fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));
    let fee_amount = calculate_fee(deps.as_ref(), &fee_config, &sender, amount)?;
    let net_amount = amount - fee_amount;
    let fee_type = get_fee_type(deps.as_ref(), &fee_config, &sender)?;

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
    let dest_token_address = hex_to_bytes32(&token_config.evm_token_address).map_err(|e| {
        ContractError::InvalidAddress {
            reason: e.to_string(),
        }
    })?;
    let dest_account = hex_to_bytes32(&recipient).map_err(|e| ContractError::InvalidAddress {
        reason: e.to_string(),
    })?;

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
    let fee_recipient = fee_config.fee_recipient.to_string();
    let mut messages: Vec<CosmosMsg> = vec![];
    if !fee_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: fee_recipient.clone(),
                amount: fee_amount,
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "deposit_cw20_lock")
        .add_attribute("nonce", nonce.to_string())
        .add_attribute("sender", sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token", token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("fee_type", fee_type.as_str())
        .add_attribute("dest_chain_id", dest_chain_id.to_string())
        .add_attribute("deposit_hash", bytes32_to_hex(&deposit_hash)))
}

/// Internal handler for burning CW20 mintable tokens (MintBurn mode)
fn execute_cw20_burn(
    deps: DepsMut,
    env: Env,
    config: crate::state::Config,
    token: String,
    amount: Uint128,
    sender: cosmwasm_std::Addr,
    dest_chain_id: u64,
    recipient: String,
) -> Result<Response, ContractError> {
    // Check destination chain
    let chain_key = dest_chain_id.to_string();
    let chain = CHAINS.may_load(deps.storage, chain_key.clone())?.ok_or(
        ContractError::ChainNotSupported {
            chain_id: dest_chain_id,
        },
    )?;

    if !chain.enabled {
        return Err(ContractError::ChainNotSupported {
            chain_id: dest_chain_id,
        });
    }

    // Check token
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

    // Verify token type is MintBurn
    if !matches!(token_config.token_type, TokenType::MintBurn) {
        return Err(ContractError::InvalidTokenType {
            expected: "mint_burn".to_string(),
            actual: token_config.token_type.as_str().to_string(),
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

    // Calculate fee using V2 fee manager
    let fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));
    let fee_amount = calculate_fee(deps.as_ref(), &fee_config, &sender, amount)?;
    let net_amount = amount - fee_amount;
    let fee_type = get_fee_type(deps.as_ref(), &fee_config, &sender)?;

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
    let dest_token_address = hex_to_bytes32(&token_config.evm_token_address).map_err(|e| {
        ContractError::InvalidAddress {
            reason: e.to_string(),
        }
    })?;
    let dest_account = hex_to_bytes32(&recipient).map_err(|e| ContractError::InvalidAddress {
        reason: e.to_string(),
    })?;

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

    // Burn net_amount, send fee to collector
    let fee_recipient = fee_config.fee_recipient.to_string();
    let mut messages: Vec<CosmosMsg> = vec![];

    // Burn the net amount (tokens were already sent to this contract via CW20 send)
    messages.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
        contract_addr: token.clone(),
        msg: to_json_binary(&Cw20ExecuteMsg::Burn { amount: net_amount })?,
        funds: vec![],
    }));

    // Send fee to collector
    if !fee_amount.is_zero() {
        messages.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: fee_recipient.clone(),
                amount: fee_amount,
            })?,
            funds: vec![],
        }));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "deposit_cw20_mintable_burn")
        .add_attribute("nonce", nonce.to_string())
        .add_attribute("sender", sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token", token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("fee_type", fee_type.as_str())
        .add_attribute("dest_chain_id", dest_chain_id.to_string())
        .add_attribute("deposit_hash", bytes32_to_hex(&deposit_hash)))
}
