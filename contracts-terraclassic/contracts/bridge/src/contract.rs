//! CL8Y Bridge Contract Implementation

use cosmwasm_std::{
    entry_point, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Order, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::msg::{
    ChainResponse, ChainsResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, LockedBalanceResponse,
    MigrateMsg, NonceResponse, NonceUsedResponse, PendingAdminResponse, QueryMsg, ReceiveMsg,
    RelayersResponse, SimulationResponse, StatsResponse, StatusResponse, TokenResponse,
    TokensResponse, TransactionResponse,
};
use crate::state::{
    BridgeTransaction, ChainConfig, Config, PendingAdmin, Stats, TokenConfig, ADMIN_TIMELOCK_DURATION,
    CHAINS, CONFIG, CONTRACT_NAME, CONTRACT_VERSION, LOCKED_BALANCES, OUTGOING_NONCE,
    PENDING_ADMIN, RELAYERS, RELAYER_COUNT, STATS, TOKENS, TRANSACTIONS, USED_NONCES,
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

    // Validate relayers
    if msg.relayers.is_empty() {
        return Err(ContractError::InvalidAddress {
            reason: "At least one relayer required".to_string(),
        });
    }

    if msg.min_signatures == 0 || msg.min_signatures > msg.relayers.len() as u32 {
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

    // Initialize relayers
    let mut relayer_count = 0u32;
    for relayer_str in msg.relayers {
        let relayer = deps.api.addr_validate(&relayer_str)?;
        RELAYERS.save(deps.storage, &relayer, &true)?;
        relayer_count += 1;
    }
    RELAYER_COUNT.save(deps.storage, &relayer_count)?;

    // Initialize stats
    let stats = Stats {
        total_outgoing_txs: 0,
        total_incoming_txs: 0,
        total_fees_collected: Uint128::zero(),
    };
    STATS.save(deps.storage, &stats)?;

    // Initialize nonce
    OUTGOING_NONCE.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", config.admin)
        .add_attribute("relayer_count", relayer_count.to_string())
        .add_attribute("min_signatures", msg.min_signatures.to_string()))
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
        ExecuteMsg::Lock {
            dest_chain_id,
            recipient,
        } => execute_lock_native(deps, env, info, dest_chain_id, recipient),
        ExecuteMsg::Receive(cw20_msg) => execute_receive(deps, env, info, cw20_msg),
        ExecuteMsg::Release {
            nonce,
            sender,
            recipient,
            token,
            amount,
            source_chain_id,
            signatures,
        } => execute_release(
            deps,
            env,
            info,
            nonce,
            sender,
            recipient,
            token,
            amount,
            source_chain_id,
            signatures,
        ),
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
        ExecuteMsg::AddRelayer { relayer } => execute_add_relayer(deps, info, relayer),
        ExecuteMsg::RemoveRelayer { relayer } => execute_remove_relayer(deps, info, relayer),
        ExecuteMsg::UpdateMinSignatures { min_signatures } => {
            execute_update_min_signatures(deps, info, min_signatures)
        }
        ExecuteMsg::UpdateLimits {
            min_bridge_amount,
            max_bridge_amount,
        } => execute_update_limits(deps, info, min_bridge_amount, max_bridge_amount),
        ExecuteMsg::UpdateFees {
            fee_bps,
            fee_collector,
        } => execute_update_fees(deps, info, fee_bps, fee_collector),
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

fn execute_lock_native(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    dest_chain_id: u64,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Check if bridge is paused
    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    // Check if destination chain is supported
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

    // Validate funds sent
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

    // Check if token is supported
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

    // Increment nonce and save transaction
    let nonce = OUTGOING_NONCE.load(deps.storage)?;
    OUTGOING_NONCE.save(deps.storage, &(nonce + 1))?;

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
        .add_attribute("dest_chain_id", dest_chain_id.to_string()))
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

    let token = info.sender.to_string(); // CW20 contract address
    let amount = cw20_msg.amount;
    let sender = deps.api.addr_validate(&cw20_msg.sender)?;

    // Parse the receive message
    let receive_msg: ReceiveMsg = cosmwasm_std::from_json(&cw20_msg.msg)?;

    match receive_msg {
        ReceiveMsg::Lock {
            dest_chain_id,
            recipient,
        } => {
            // Check if destination chain is supported
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

            // Check if token is supported
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

            // Increment nonce and save transaction
            let nonce = OUTGOING_NONCE.load(deps.storage)?;
            OUTGOING_NONCE.save(deps.storage, &(nonce + 1))?;

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
                .add_attribute("dest_chain_id", dest_chain_id.to_string()))
        }
    }
}

fn execute_release(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: u64,
    sender: String,
    recipient: String,
    token: String,
    amount: Uint128,
    source_chain_id: u64,
    signatures: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.paused {
        return Err(ContractError::BridgePaused);
    }

    // Verify caller is a relayer
    let is_relayer = RELAYERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);
    if !is_relayer {
        return Err(ContractError::UnauthorizedRelayer);
    }

    // Check nonce hasn't been used
    let nonce_used = USED_NONCES.may_load(deps.storage, nonce)?.unwrap_or(false);
    if nonce_used {
        return Err(ContractError::NonceAlreadyUsed { nonce });
    }

    // Check source chain is supported
    let chain_key = source_chain_id.to_string();
    let chain = CHAINS
        .may_load(deps.storage, chain_key.clone())?
        .ok_or(ContractError::ChainNotSupported {
            chain_id: source_chain_id,
        })?;

    if !chain.enabled {
        return Err(ContractError::ChainNotSupported {
            chain_id: source_chain_id,
        });
    }

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

    // Verify signatures (simplified - in production would verify actual signatures)
    if signatures.len() < config.min_signatures as usize {
        return Err(ContractError::InsufficientSignatures {
            got: signatures.len() as u32,
            required: config.min_signatures,
        });
    }

    // Mark nonce as used
    USED_NONCES.save(deps.storage, nonce, &true)?;

    // Validate recipient
    let recipient_addr = deps.api.addr_validate(&recipient)?;

    // Check locked balance
    let locked = LOCKED_BALANCES
        .may_load(deps.storage, token.clone())?
        .unwrap_or(Uint128::zero());
    if locked < amount {
        return Err(ContractError::InsufficientLiquidity);
    }

    // Update locked balance
    LOCKED_BALANCES.save(deps.storage, token.clone(), &(locked - amount))?;

    // Save transaction
    let tx = BridgeTransaction {
        nonce,
        sender: sender.clone(),
        recipient: recipient.clone(),
        token: token.clone(),
        amount,
        dest_chain_id: 0, // TerraClassic
        timestamp: env.block.time,
        is_outgoing: false,
    };
    TRANSACTIONS.save(deps.storage, nonce, &tx)?;

    // Update stats
    let mut stats = STATS.load(deps.storage)?;
    stats.total_incoming_txs += 1;
    STATS.save(deps.storage, &stats)?;

    // Release tokens
    let messages: Vec<CosmosMsg> = if token_config.is_native {
        vec![CosmosMsg::Bank(BankMsg::Send {
            to_address: recipient_addr.to_string(),
            amount: vec![Coin {
                denom: token.clone(),
                amount,
            }],
        })]
    } else {
        vec![CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
            contract_addr: token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient_addr.to_string(),
                amount,
            })?,
            funds: vec![],
        })]
    };

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "release")
        .add_attribute("nonce", nonce.to_string())
        .add_attribute("sender", sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token", token)
        .add_attribute("amount", amount.to_string())
        .add_attribute("source_chain_id", source_chain_id.to_string()))
}

// Admin functions

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
        bridge_address: bridge_address.clone(),
        enabled: true,
    };
    CHAINS.save(deps.storage, chain_key.clone(), &chain)?;

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

    CHAINS.save(deps.storage, chain_key.clone(), &chain)?;

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
        evm_token_address: evm_token_address.clone(),
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

fn execute_add_relayer(
    deps: DepsMut,
    info: MessageInfo,
    relayer: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let relayer_addr = deps.api.addr_validate(&relayer)?;
    let existing = RELAYERS.may_load(deps.storage, &relayer_addr)?.unwrap_or(false);
    if existing {
        return Err(ContractError::RelayerAlreadyRegistered);
    }

    RELAYERS.save(deps.storage, &relayer_addr, &true)?;
    let count = RELAYER_COUNT.load(deps.storage)?;
    RELAYER_COUNT.save(deps.storage, &(count + 1))?;

    Ok(Response::new()
        .add_attribute("method", "add_relayer")
        .add_attribute("relayer", relayer))
}

fn execute_remove_relayer(
    deps: DepsMut,
    info: MessageInfo,
    relayer: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let relayer_addr = deps.api.addr_validate(&relayer)?;
    let existing = RELAYERS.may_load(deps.storage, &relayer_addr)?.unwrap_or(false);
    if !existing {
        return Err(ContractError::RelayerNotRegistered);
    }

    let count = RELAYER_COUNT.load(deps.storage)?;
    if count <= 1 {
        return Err(ContractError::CannotRemoveLastRelayer);
    }
    if count <= config.min_signatures {
        return Err(ContractError::InsufficientSignatures {
            got: count - 1,
            required: config.min_signatures,
        });
    }

    RELAYERS.remove(deps.storage, &relayer_addr);
    RELAYER_COUNT.save(deps.storage, &(count - 1))?;

    Ok(Response::new()
        .add_attribute("method", "remove_relayer")
        .add_attribute("relayer", relayer))
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

    let count = RELAYER_COUNT.load(deps.storage)?;
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
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
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
        QueryMsg::Relayers {} => to_json_binary(&query_relayers(deps)?),
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
    }
}

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
    let relayer_count = RELAYER_COUNT.load(deps.storage)?;

    // Count supported chains
    let chains: Vec<_> = CHAINS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    // Count supported tokens
    let tokens: Vec<_> = TOKENS
        .range(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    Ok(StatusResponse {
        paused: config.paused,
        active_relayers: relayer_count,
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
    let chain = CHAINS.load(deps.storage, chain_key.clone())?;
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
        .range(deps.storage, start, None, cosmwasm_std::Order::Ascending)
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
    let token_config = TOKENS.load(deps.storage, token.clone())?;
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
        .range(deps.storage, start, None, cosmwasm_std::Order::Ascending)
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

fn query_relayers(deps: Deps) -> StdResult<RelayersResponse> {
    let config = CONFIG.load(deps.storage)?;

    let relayers: Vec<Addr> = RELAYERS
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

    Ok(RelayersResponse {
        relayers,
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

    // Verify chain exists
    let chain_key = dest_chain_id.to_string();
    let _chain = CHAINS.load(deps.storage, chain_key)?;

    // Verify token exists
    let _token_config = TOKENS.load(deps.storage, token.clone())?;

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
// Migrate
// ============================================================================

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new().add_attribute("method", "migrate"))
}
