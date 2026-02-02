//! Outgoing transfer handlers (Lock and Receive).
//!
//! These handlers process tokens being locked on Terra for bridging to other chains.

use cosmwasm_std::{
    to_json_binary, BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, Uint128,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::error::ContractError;
use crate::hash::{
    bytes32_to_hex, compute_transfer_id, evm_chain_key, hex_to_bytes32, terra_chain_key,
};
use crate::msg::ReceiveMsg;
use crate::state::{
    BridgeTransaction, DepositInfo, CHAINS, CONFIG, DEPOSIT_BY_NONCE, DEPOSIT_HASHES,
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
        .map_err(|e| ContractError::InvalidAddress {
            reason: e.to_string(),
        })?;
    let dest_account =
        hex_to_bytes32(&recipient).map_err(|e| ContractError::InvalidAddress {
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

/// Execute handler for receiving CW20 tokens to lock
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
                .map_err(|e| ContractError::InvalidAddress {
                    reason: e.to_string(),
                })?;
            let dest_account =
                hex_to_bytes32(&recipient).map_err(|e| ContractError::InvalidAddress {
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
