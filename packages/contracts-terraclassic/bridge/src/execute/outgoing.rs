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
use crate::hash::{bytes32_to_hex, compute_transfer_hash, encode_terra_address, hex_to_bytes32};
use crate::msg::ReceiveMsg;
use crate::state::{
    BridgeTransaction, DepositInfo, TokenType, CHAINS, CONFIG, DEPOSIT_BY_NONCE, DEPOSIT_HASHES,
    LOCKED_BALANCES, OUTGOING_NONCE, STATS, THIS_CHAIN_ID, TOKENS, TRANSACTIONS,
};

/// Parse a 4-byte chain ID from Binary input.
fn parse_chain_id(chain: &cosmwasm_std::Binary) -> Result<[u8; 4], ContractError> {
    chain
        .to_vec()
        .try_into()
        .map_err(|_| ContractError::InvalidHashLength { got: chain.len() })
}

/// Parse a 32-byte account from Binary input.
fn parse_bytes32(bin: &cosmwasm_std::Binary) -> Result<[u8; 32], ContractError> {
    if bin.len() != 32 {
        return Err(ContractError::InvalidAddress {
            reason: format!("Expected 32 bytes, got {}", bin.len()),
        });
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(bin);
    Ok(arr)
}

/// Execute handler for depositing native tokens (uluna, etc.) — locks them on Terra.
pub fn execute_deposit_native(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dest_chain: cosmwasm_std::Binary,
    dest_account_bin: cosmwasm_std::Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    // Check destination chain
    let dest_chain_bytes = parse_chain_id(&dest_chain)?;
    let chain =
        CHAINS
            .may_load(deps.storage, &dest_chain_bytes)?
            .ok_or(ContractError::InvalidAddress {
                reason: "Destination chain not registered".to_string(),
            })?;

    if !chain.enabled {
        return Err(ContractError::InvalidAddress {
            reason: "Destination chain is disabled".to_string(),
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

    // Native deposits lock tokens; must be LockUnlock type
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
    let dest_account = parse_bytes32(&dest_account_bin)?;

    let tx = BridgeTransaction {
        nonce,
        sender: info.sender.to_string(),
        recipient: format!("0x{}", hex::encode(dest_account)),
        token: token.clone(),
        amount: net_amount,
        dest_chain: dest_chain_bytes,
        timestamp: env.block.time,
        is_outgoing: true,
    };
    TRANSACTIONS.save(deps.storage, nonce, &tx)?;

    // Compute and store deposit hash for verification
    let src_chain = THIS_CHAIN_ID.load(deps.storage)?;
    let dest_chain = dest_chain_bytes;
    let src_account = encode_terra_address(deps.as_ref(), &info.sender)?;
    let dest_token_address = hex_to_bytes32(&token_config.evm_token_address).map_err(|e| {
        ContractError::InvalidAddress {
            reason: e.to_string(),
        }
    })?;

    let deposit_info = DepositInfo {
        src_chain,
        dest_chain,
        src_account,
        dest_account,
        dest_token_address,
        amount: net_amount,
        nonce,
        deposited_at: env.block.time,
        dest_chain_key: [0u8; 32],
    };

    let deposit_hash = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &dest_token_address,
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
        .add_attribute("dest_account", format!("0x{}", hex::encode(dest_account)))
        .add_attribute("token", token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("fee_type", fee_type.as_str())
        .add_attribute("dest_chain", format!("0x{}", hex::encode(dest_chain_bytes)))
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
        ReceiveMsg::DepositCw20Lock {
            dest_chain,
            dest_account,
        } => execute_deposit_cw20_lock(
            deps,
            env,
            config,
            token,
            amount,
            sender,
            dest_chain,
            dest_account,
        ),
        ReceiveMsg::DepositCw20MintableBurn {
            dest_chain,
            dest_account,
        } => execute_deposit_cw20_burn(
            deps,
            env,
            config,
            token,
            amount,
            sender,
            dest_chain,
            dest_account,
        ),
    }
}

/// Internal handler for locking CW20 tokens (LockUnlock mode)
#[allow(clippy::too_many_arguments)]
fn execute_deposit_cw20_lock(
    deps: DepsMut,
    env: Env,
    config: crate::state::Config,
    token: String,
    amount: Uint128,
    sender: cosmwasm_std::Addr,
    dest_chain: cosmwasm_std::Binary,
    dest_account_bin: cosmwasm_std::Binary,
) -> Result<Response, ContractError> {
    // Check destination chain
    let dest_chain_bytes = parse_chain_id(&dest_chain)?;
    let chain =
        CHAINS
            .may_load(deps.storage, &dest_chain_bytes)?
            .ok_or(ContractError::InvalidAddress {
                reason: "Destination chain not registered".to_string(),
            })?;

    if !chain.enabled {
        return Err(ContractError::InvalidAddress {
            reason: "Destination chain is disabled".to_string(),
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

    let dest_account = parse_bytes32(&dest_account_bin)?;

    // Store transaction
    let tx = BridgeTransaction {
        nonce,
        sender: sender.to_string(),
        recipient: format!("0x{}", hex::encode(dest_account)),
        token: token.clone(),
        amount: net_amount,
        dest_chain: dest_chain_bytes,
        timestamp: env.block.time,
        is_outgoing: true,
    };
    TRANSACTIONS.save(deps.storage, nonce, &tx)?;

    // Compute and store deposit hash
    let src_chain = THIS_CHAIN_ID.load(deps.storage)?;
    let dest_chain = dest_chain_bytes;
    let src_account = encode_terra_address(deps.as_ref(), &sender)?;
    let dest_token_address = hex_to_bytes32(&token_config.evm_token_address).map_err(|e| {
        ContractError::InvalidAddress {
            reason: e.to_string(),
        }
    })?;

    let deposit_info = DepositInfo {
        src_chain,
        dest_chain,
        src_account,
        dest_account,
        dest_token_address,
        amount: net_amount,
        nonce,
        deposited_at: env.block.time,
        dest_chain_key: [0u8; 32],
    };

    let deposit_hash = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &dest_token_address,
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
        .add_attribute("dest_account", format!("0x{}", hex::encode(dest_account)))
        .add_attribute("token", token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("fee_type", fee_type.as_str())
        .add_attribute("dest_chain", format!("0x{}", hex::encode(dest_chain_bytes)))
        .add_attribute("deposit_hash", bytes32_to_hex(&deposit_hash)))
}

/// Internal handler for burning CW20 mintable tokens (MintBurn mode)
#[allow(clippy::too_many_arguments)]
fn execute_deposit_cw20_burn(
    deps: DepsMut,
    env: Env,
    config: crate::state::Config,
    token: String,
    amount: Uint128,
    sender: cosmwasm_std::Addr,
    dest_chain: cosmwasm_std::Binary,
    dest_account_bin: cosmwasm_std::Binary,
) -> Result<Response, ContractError> {
    // Check destination chain
    let dest_chain_bytes = parse_chain_id(&dest_chain)?;
    let chain =
        CHAINS
            .may_load(deps.storage, &dest_chain_bytes)?
            .ok_or(ContractError::InvalidAddress {
                reason: "Destination chain not registered".to_string(),
            })?;

    if !chain.enabled {
        return Err(ContractError::InvalidAddress {
            reason: "Destination chain is disabled".to_string(),
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

    let dest_account = parse_bytes32(&dest_account_bin)?;

    // Store transaction
    let tx = BridgeTransaction {
        nonce,
        sender: sender.to_string(),
        recipient: format!("0x{}", hex::encode(dest_account)),
        token: token.clone(),
        amount: net_amount,
        dest_chain: dest_chain_bytes,
        timestamp: env.block.time,
        is_outgoing: true,
    };
    TRANSACTIONS.save(deps.storage, nonce, &tx)?;

    // Compute and store deposit hash
    let src_chain = THIS_CHAIN_ID.load(deps.storage)?;
    let dest_chain = dest_chain_bytes;
    let src_account = encode_terra_address(deps.as_ref(), &sender)?;
    let dest_token_address = hex_to_bytes32(&token_config.evm_token_address).map_err(|e| {
        ContractError::InvalidAddress {
            reason: e.to_string(),
        }
    })?;

    let deposit_info = DepositInfo {
        src_chain,
        dest_chain,
        src_account,
        dest_account,
        dest_token_address,
        amount: net_amount,
        nonce,
        deposited_at: env.block.time,
        dest_chain_key: [0u8; 32],
    };

    let deposit_hash = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &dest_token_address,
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
        .add_attribute("dest_account", format!("0x{}", hex::encode(dest_account)))
        .add_attribute("token", token)
        .add_attribute("amount", net_amount.to_string())
        .add_attribute("fee", fee_amount.to_string())
        .add_attribute("fee_type", fee_type.as_str())
        .add_attribute("dest_chain", format!("0x{}", hex::encode(dest_chain_bytes)))
        .add_attribute("deposit_hash", bytes32_to_hex(&deposit_hash)))
}
