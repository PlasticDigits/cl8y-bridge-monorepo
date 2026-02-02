//! Configuration management handlers.
//!
//! This module handles:
//! - Canceler management (add/remove)
//! - Withdraw delay configuration
//! - Rate limit configuration
//! - Chain management (add/update)
//! - Token management (add/update)
//! - Operator management (add/remove/update min signatures)
//! - Bridge limits and fees

use cosmwasm_std::{DepsMut, MessageInfo, Response, Uint128};

use crate::error::ContractError;
use crate::state::{
    ChainConfig, RateLimitConfig, TokenConfig, CANCELERS, CHAINS, CONFIG, OPERATORS,
    OPERATOR_COUNT, RATE_LIMITS, TOKENS, WITHDRAW_DELAY,
};

// ============================================================================
// Canceler Management
// ============================================================================

/// Add a new canceler address.
pub fn execute_add_canceler(
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

/// Remove a canceler address.
pub fn execute_remove_canceler(
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
// Withdraw Delay Configuration
// ============================================================================

/// Set the withdraw delay (watchtower pattern timer).
pub fn execute_set_withdraw_delay(
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

// ============================================================================
// Rate Limit Configuration
// ============================================================================

/// Set rate limits for a token.
pub fn execute_set_rate_limit(
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
// Chain Management
// ============================================================================

/// Add a new supported chain.
pub fn execute_add_chain(
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

/// Update an existing chain configuration.
pub fn execute_update_chain(
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

// ============================================================================
// Token Management
// ============================================================================

/// Add a new supported token.
pub fn execute_add_token(
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

/// Update an existing token configuration.
pub fn execute_update_token(
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

/// Add a new operator.
pub fn execute_add_operator(
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

/// Remove an operator.
pub fn execute_remove_operator(
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

/// Update the minimum required signatures.
pub fn execute_update_min_signatures(
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

/// Update bridge amount limits.
pub fn execute_update_limits(
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

/// Update fee configuration.
pub fn execute_update_fees(
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
