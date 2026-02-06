//! Configuration management handlers.
//!
//! This module handles:
//! - Canceler management (add/remove)
//! - Withdraw delay configuration
//! - Rate limit configuration
//! - Chain management (add/update)
//! - Token management (add/update)
//! - Token destination mappings
//! - Operator management (add/remove/update min signatures)
//! - Bridge limits and fees
//! - V2 Fee configuration (CL8Y discount, custom account fees)

use cosmwasm_std::{DepsMut, MessageInfo, Response, Uint128};

use crate::error::ContractError;
use crate::fee_manager::{
    remove_custom_account_fee, set_custom_account_fee, validate_custom_fee, FeeConfig, FEE_CONFIG,
    MAX_FEE_BPS,
};
use cosmwasm_std::Binary;

use crate::hash::{hex_to_bytes32, keccak256};
use crate::state::{
    ChainConfig, RateLimitConfig, TokenConfig, TokenDestMapping, TokenType, CANCELERS, CHAINS,
    CHAIN_BY_IDENTIFIER, CONFIG, OPERATORS, OPERATOR_COUNT, RATE_LIMITS, TOKENS,
    TOKEN_DEST_MAPPINGS, WITHDRAW_DELAY,
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
    if !(60..=86400).contains(&delay_seconds) {
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

/// Register a new chain with a predetermined 4-byte chain ID.
pub fn execute_register_chain(
    deps: DepsMut,
    info: MessageInfo,
    identifier: String,
    chain_id_bin: cosmwasm_std::Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    // Parse and validate chain ID (must be exactly 4 bytes)
    if chain_id_bin.len() != 4 {
        return Err(ContractError::InvalidAddress {
            reason: format!(
                "chain_id must be exactly 4 bytes, got {}",
                chain_id_bin.len()
            ),
        });
    }
    let chain_id: [u8; 4] = [
        chain_id_bin[0],
        chain_id_bin[1],
        chain_id_bin[2],
        chain_id_bin[3],
    ];

    // Chain ID 0x00000000 is reserved/invalid
    if chain_id == [0u8; 4] {
        return Err(ContractError::InvalidAddress {
            reason: "chain_id 0x00000000 is reserved/invalid".to_string(),
        });
    }

    // Check identifier not already registered
    if CHAIN_BY_IDENTIFIER
        .may_load(deps.storage, &identifier)?
        .is_some()
    {
        return Err(ContractError::InvalidAddress {
            reason: format!("Chain identifier '{}' already registered", identifier),
        });
    }

    // Check chain ID not already in use
    if CHAINS.may_load(deps.storage, &chain_id)?.is_some() {
        return Err(ContractError::InvalidAddress {
            reason: format!("Chain ID 0x{} already in use", hex::encode(chain_id)),
        });
    }

    let identifier_hash = keccak256(identifier.as_bytes());

    let chain = ChainConfig {
        chain_id,
        identifier: identifier.clone(),
        identifier_hash,
        enabled: true,
    };
    CHAINS.save(deps.storage, &chain_id, &chain)?;
    CHAIN_BY_IDENTIFIER.save(deps.storage, &identifier, &chain_id)?;

    Ok(Response::new()
        .add_attribute("action", "register_chain")
        .add_attribute("chain_id", format!("0x{}", hex::encode(chain_id)))
        .add_attribute("identifier", identifier))
}

/// Unregister an existing chain by its 4-byte chain ID.
pub fn execute_unregister_chain(
    deps: DepsMut,
    info: MessageInfo,
    chain_id_bin: cosmwasm_std::Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    // Parse chain ID
    if chain_id_bin.len() != 4 {
        return Err(ContractError::InvalidAddress {
            reason: format!(
                "chain_id must be exactly 4 bytes, got {}",
                chain_id_bin.len()
            ),
        });
    }
    let chain_id: [u8; 4] = [
        chain_id_bin[0],
        chain_id_bin[1],
        chain_id_bin[2],
        chain_id_bin[3],
    ];

    // Load existing chain config (must exist)
    let chain =
        CHAINS
            .may_load(deps.storage, &chain_id)?
            .ok_or_else(|| ContractError::InvalidAddress {
                reason: format!("Chain ID 0x{} not registered", hex::encode(chain_id)),
            })?;

    // Remove from both maps
    CHAINS.remove(deps.storage, &chain_id);
    CHAIN_BY_IDENTIFIER.remove(deps.storage, &chain.identifier);

    Ok(Response::new()
        .add_attribute("action", "unregister_chain")
        .add_attribute("chain_id", format!("0x{}", hex::encode(chain_id)))
        .add_attribute("identifier", chain.identifier))
}

/// Update an existing chain configuration (enable/disable).
pub fn execute_update_chain(
    deps: DepsMut,
    info: MessageInfo,
    chain_id: Binary,
    enabled: Option<bool>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let chain_id_bytes: [u8; 4] =
        chain_id
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: chain_id.len(),
            })?;

    let mut chain =
        CHAINS
            .may_load(deps.storage, &chain_id_bytes)?
            .ok_or(ContractError::InvalidAddress {
                reason: "Chain not registered".to_string(),
            })?;

    if let Some(e) = enabled {
        chain.enabled = e;
    }

    CHAINS.save(deps.storage, &chain_id_bytes, &chain)?;

    Ok(Response::new()
        .add_attribute("action", "update_chain")
        .add_attribute("chain_id", format!("0x{}", hex::encode(chain_id_bytes))))
}

// ============================================================================
// Token Management
// ============================================================================

/// Parse token type from string.
fn parse_token_type(token_type_str: Option<String>) -> TokenType {
    match token_type_str.as_deref() {
        Some("mint_burn") => TokenType::MintBurn,
        _ => TokenType::LockUnlock, // Default
    }
}

/// Add a new supported token.
#[allow(clippy::too_many_arguments)]
pub fn execute_add_token(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    is_native: bool,
    token_type: Option<String>,
    evm_token_address: String,
    terra_decimals: u8,
    evm_decimals: u8,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let token_type_parsed = parse_token_type(token_type);

    let token_config = TokenConfig {
        token: token.clone(),
        is_native,
        token_type: token_type_parsed.clone(),
        evm_token_address,
        terra_decimals,
        evm_decimals,
        enabled: true,
    };
    TOKENS.save(deps.storage, token.clone(), &token_config)?;

    Ok(Response::new()
        .add_attribute("action", "add_token")
        .add_attribute("token", token)
        .add_attribute("is_native", is_native.to_string())
        .add_attribute("token_type", token_type_parsed.as_str()))
}

/// Update an existing token configuration.
pub fn execute_update_token(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    evm_token_address: Option<String>,
    enabled: Option<bool>,
    token_type: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let mut token_config =
        TOKENS
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
    if let Some(tt) = token_type {
        token_config.token_type = parse_token_type(Some(tt));
    }

    TOKENS.save(deps.storage, token.clone(), &token_config)?;

    Ok(Response::new()
        .add_attribute("action", "update_token")
        .add_attribute("token", token)
        .add_attribute("token_type", token_config.token_type.as_str()))
}

/// Set destination chain token mapping.
pub fn execute_set_token_destination(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    dest_chain: Binary,
    dest_token: String,
    dest_decimals: u8,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    // Verify token exists
    let _token_config =
        TOKENS
            .may_load(deps.storage, token.clone())?
            .ok_or(ContractError::TokenNotSupported {
                token: token.clone(),
            })?;

    // Parse and verify destination chain
    let dest_chain_bytes: [u8; 4] =
        dest_chain
            .to_vec()
            .try_into()
            .map_err(|_| ContractError::InvalidHashLength {
                got: dest_chain.len(),
            })?;
    let _chain =
        CHAINS
            .may_load(deps.storage, &dest_chain_bytes)?
            .ok_or(ContractError::InvalidAddress {
                reason: "Destination chain not registered".to_string(),
            })?;

    // Parse destination token address
    let dest_token_bytes =
        hex_to_bytes32(&dest_token).map_err(|e| ContractError::InvalidAddress {
            reason: e.to_string(),
        })?;

    let mapping = TokenDestMapping {
        dest_token: dest_token_bytes,
        dest_decimals,
    };

    let dest_chain_key = hex::encode(dest_chain_bytes);
    TOKEN_DEST_MAPPINGS.save(deps.storage, (&token, &dest_chain_key), &mapping)?;

    Ok(Response::new()
        .add_attribute("action", "set_token_destination")
        .add_attribute("token", token)
        .add_attribute("dest_chain", format!("0x{}", dest_chain_key))
        .add_attribute("dest_token", dest_token)
        .add_attribute("dest_decimals", dest_decimals.to_string()))
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

/// Update fee configuration (legacy, for backwards compatibility).
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
        .add_attribute("action", "update_fees")
        .add_attribute("fee_bps", config.fee_bps.to_string())
        .add_attribute("fee_collector", config.fee_collector.to_string()))
}

// ============================================================================
// V2 Fee Configuration
// ============================================================================

/// Set V2 fee parameters (CL8Y discount support).
pub fn execute_set_fee_params(
    deps: DepsMut,
    info: MessageInfo,
    standard_fee_bps: Option<u64>,
    discounted_fee_bps: Option<u64>,
    cl8y_threshold: Option<Uint128>,
    cl8y_token: Option<String>,
    fee_recipient: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized);
    }

    let mut fee_config = FEE_CONFIG
        .may_load(deps.storage)?
        .unwrap_or_else(|| FeeConfig::default_with_recipient(config.fee_collector.clone()));

    if let Some(bps) = standard_fee_bps {
        if bps > MAX_FEE_BPS {
            return Err(ContractError::FeeExceedsMax { fee_bps: bps });
        }
        fee_config.standard_fee_bps = bps;
    }

    if let Some(bps) = discounted_fee_bps {
        if bps > MAX_FEE_BPS {
            return Err(ContractError::FeeExceedsMax { fee_bps: bps });
        }
        fee_config.discounted_fee_bps = bps;
    }

    if let Some(threshold) = cl8y_threshold {
        fee_config.cl8y_threshold = threshold;
    }

    if let Some(token) = cl8y_token {
        fee_config.cl8y_token = Some(deps.api.addr_validate(&token)?);
    }

    if let Some(recipient) = fee_recipient {
        fee_config.fee_recipient = deps.api.addr_validate(&recipient)?;
    }

    FEE_CONFIG.save(deps.storage, &fee_config)?;

    Ok(Response::new()
        .add_attribute("action", "set_fee_params")
        .add_attribute("standard_fee_bps", fee_config.standard_fee_bps.to_string())
        .add_attribute(
            "discounted_fee_bps",
            fee_config.discounted_fee_bps.to_string(),
        )
        .add_attribute("cl8y_threshold", fee_config.cl8y_threshold.to_string())
        .add_attribute("fee_recipient", fee_config.fee_recipient.to_string()))
}

/// Set custom fee for a specific account.
pub fn execute_set_custom_account_fee(
    deps: DepsMut,
    info: MessageInfo,
    account: String,
    fee_bps: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // Allow both admin and operators to set custom fees
    let is_operator = crate::state::OPERATORS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);

    if info.sender != config.admin && !is_operator {
        return Err(ContractError::Unauthorized);
    }

    validate_custom_fee(fee_bps)?;

    let account_addr = deps.api.addr_validate(&account)?;
    set_custom_account_fee(deps.storage, &account_addr, fee_bps)?;

    Ok(Response::new()
        .add_attribute("action", "set_custom_account_fee")
        .add_attribute("account", account)
        .add_attribute("fee_bps", fee_bps.to_string()))
}

/// Remove custom fee for a specific account.
pub fn execute_remove_custom_account_fee(
    deps: DepsMut,
    info: MessageInfo,
    account: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // Allow both admin and operators to remove custom fees
    let is_operator = crate::state::OPERATORS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false);

    if info.sender != config.admin && !is_operator {
        return Err(ContractError::Unauthorized);
    }

    let account_addr = deps.api.addr_validate(&account)?;
    remove_custom_account_fee(deps.storage, &account_addr);

    Ok(Response::new()
        .add_attribute("action", "remove_custom_account_fee")
        .add_attribute("account", account))
}
