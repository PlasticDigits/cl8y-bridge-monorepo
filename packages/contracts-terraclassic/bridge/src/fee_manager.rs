//! Fee Manager Module
//!
//! This module provides fee calculation logic with CL8Y token holder discounts
//! and custom per-account fees. It matches the EVM FeeCalculatorLib.sol implementation.
//!
//! ## Fee Structure
//!
//! | Fee Type            | Rate      | Condition                    |
//! |---------------------|-----------|------------------------------|
//! | Standard Deposit    | 0.5% (50 bps)  | Default for all users  |
//! | CL8Y Holder Discount| 0.1% (10 bps)  | User holds ≥100 CL8Y   |
//! | Custom Account Fee  | 0-1% (0-100 bps) | Per-account override |
//!
//! ## Fee Priority (highest to lowest)
//!
//! 1. Custom account fee (if set) - capped at 1%
//! 2. CL8Y holder discount (if eligible)
//! 3. Standard fee

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Deps, QuerierWrapper, StdResult, Uint128};
use cw_storage_plus::{Item, Map};

// ============================================================================
// Constants
// ============================================================================

/// Maximum fee in basis points (1% = 100 bps)
pub const MAX_FEE_BPS: u64 = 100;

/// Default standard fee in basis points (0.5% = 50 bps)
pub const DEFAULT_STANDARD_FEE_BPS: u64 = 50;

/// Default discounted fee in basis points (0.1% = 10 bps)
pub const DEFAULT_DISCOUNTED_FEE_BPS: u64 = 10;

/// Default CL8Y threshold for discount (100 CL8Y with 6 decimals for Terra)
pub const DEFAULT_CL8Y_THRESHOLD: u128 = 100_000_000;

/// Basis points denominator (10000 = 100%)
pub const BPS_DENOMINATOR: u128 = 10000;

// ============================================================================
// Data Structures
// ============================================================================

/// Fee configuration parameters
#[cw_serde]
pub struct FeeConfig {
    /// Standard fee in basis points (default 50 = 0.5%)
    pub standard_fee_bps: u64,
    /// Discounted fee in basis points (default 10 = 0.1%)
    pub discounted_fee_bps: u64,
    /// Minimum CL8Y balance for discount
    pub cl8y_threshold: Uint128,
    /// CL8Y token contract address (None = discount disabled)
    pub cl8y_token: Option<Addr>,
    /// Address to receive collected fees
    pub fee_recipient: Addr,
}

impl FeeConfig {
    /// Create a default fee configuration
    pub fn default_with_recipient(fee_recipient: Addr) -> Self {
        Self {
            standard_fee_bps: DEFAULT_STANDARD_FEE_BPS,
            discounted_fee_bps: DEFAULT_DISCOUNTED_FEE_BPS,
            cl8y_threshold: Uint128::from(DEFAULT_CL8Y_THRESHOLD),
            cl8y_token: None,
            fee_recipient,
        }
    }

    /// Validate the fee configuration
    pub fn validate(&self) -> StdResult<()> {
        if self.standard_fee_bps > MAX_FEE_BPS {
            return Err(cosmwasm_std::StdError::generic_err(format!(
                "Standard fee {} exceeds max {}",
                self.standard_fee_bps, MAX_FEE_BPS
            )));
        }
        if self.discounted_fee_bps > MAX_FEE_BPS {
            return Err(cosmwasm_std::StdError::generic_err(format!(
                "Discounted fee {} exceeds max {}",
                self.discounted_fee_bps, MAX_FEE_BPS
            )));
        }
        Ok(())
    }
}

/// Fee type enum for reporting
#[cw_serde]
pub enum FeeType {
    Standard,
    Discounted,
    Custom,
}

impl FeeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeeType::Standard => "standard",
            FeeType::Discounted => "discounted",
            FeeType::Custom => "custom",
        }
    }
}

// ============================================================================
// Storage
// ============================================================================

/// Fee configuration storage
pub const FEE_CONFIG: Item<FeeConfig> = Item::new("fee_config_v2");

/// Custom per-account fees (account address -> fee in bps)
pub const CUSTOM_ACCOUNT_FEES: Map<&Addr, u64> = Map::new("custom_account_fees");

// ============================================================================
// Fee Calculation Functions
// ============================================================================

/// Calculate the fee for a given amount and depositor
///
/// Priority:
/// 1. Custom account fee (if set)
/// 2. CL8Y holder discount (if eligible)
/// 3. Standard fee
pub fn calculate_fee(
    deps: Deps,
    config: &FeeConfig,
    depositor: &Addr,
    amount: Uint128,
) -> StdResult<Uint128> {
    let fee_bps = get_effective_fee_bps(deps, config, depositor)?;
    Ok(calculate_fee_from_bps(amount, fee_bps))
}

/// Get the effective fee rate for an account
pub fn get_effective_fee_bps(deps: Deps, config: &FeeConfig, account: &Addr) -> StdResult<u64> {
    // Priority 1: Custom account fee
    if let Some(custom_fee) = CUSTOM_ACCOUNT_FEES.may_load(deps.storage, account)? {
        return Ok(custom_fee);
    }

    // Priority 2: CL8Y holder discount
    if let Some(cl8y_token) = &config.cl8y_token {
        if is_eligible_for_discount(&deps.querier, cl8y_token, account, config.cl8y_threshold)? {
            return Ok(config.discounted_fee_bps);
        }
    }

    // Priority 3: Standard fee
    Ok(config.standard_fee_bps)
}

/// Get the fee type for an account
pub fn get_fee_type(deps: Deps, config: &FeeConfig, account: &Addr) -> StdResult<FeeType> {
    // Priority 1: Custom account fee
    if CUSTOM_ACCOUNT_FEES
        .may_load(deps.storage, account)?
        .is_some()
    {
        return Ok(FeeType::Custom);
    }

    // Priority 2: CL8Y holder discount
    if let Some(cl8y_token) = &config.cl8y_token {
        if is_eligible_for_discount(&deps.querier, cl8y_token, account, config.cl8y_threshold)? {
            return Ok(FeeType::Discounted);
        }
    }

    // Priority 3: Standard fee
    Ok(FeeType::Standard)
}

/// Check if an account is eligible for CL8Y discount
pub fn is_eligible_for_discount(
    querier: &QuerierWrapper,
    cl8y_token: &Addr,
    account: &Addr,
    threshold: Uint128,
) -> StdResult<bool> {
    let balance = query_cw20_balance(querier, cl8y_token, account)?;
    Ok(balance >= threshold)
}

/// Query CW20 token balance
fn query_cw20_balance(
    querier: &QuerierWrapper,
    token: &Addr,
    account: &Addr,
) -> StdResult<Uint128> {
    let query_msg = cw20::Cw20QueryMsg::Balance {
        address: account.to_string(),
    };

    let response: cw20::BalanceResponse = querier.query_wasm_smart(token, &query_msg)?;
    Ok(response.balance)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate fee amount from amount and bps
pub fn calculate_fee_from_bps(amount: Uint128, fee_bps: u64) -> Uint128 {
    amount.multiply_ratio(fee_bps as u128, BPS_DENOMINATOR)
}

/// Calculate net amount after fee deduction
pub fn calculate_net_amount(amount: Uint128, fee_bps: u64) -> Uint128 {
    let fee = calculate_fee_from_bps(amount, fee_bps);
    amount.checked_sub(fee).unwrap_or(Uint128::zero())
}

/// Validate custom fee is within bounds
pub fn validate_custom_fee(fee_bps: u64) -> StdResult<()> {
    if fee_bps > MAX_FEE_BPS {
        return Err(cosmwasm_std::StdError::generic_err(format!(
            "Custom fee {} exceeds max {}",
            fee_bps, MAX_FEE_BPS
        )));
    }
    Ok(())
}

// ============================================================================
// Admin Functions (to be called from execute handlers)
// ============================================================================

/// Set custom fee for an account
pub fn set_custom_account_fee(
    storage: &mut dyn cosmwasm_std::Storage,
    account: &Addr,
    fee_bps: u64,
) -> StdResult<()> {
    validate_custom_fee(fee_bps)?;
    CUSTOM_ACCOUNT_FEES.save(storage, account, &fee_bps)
}

/// Remove custom fee for an account
pub fn remove_custom_account_fee(storage: &mut dyn cosmwasm_std::Storage, account: &Addr) {
    CUSTOM_ACCOUNT_FEES.remove(storage, account);
}

/// Check if an account has a custom fee
pub fn has_custom_fee(deps: Deps, account: &Addr) -> StdResult<bool> {
    Ok(CUSTOM_ACCOUNT_FEES
        .may_load(deps.storage, account)?
        .is_some())
}

/// Get custom fee for an account (if any)
pub fn get_custom_fee(deps: Deps, account: &Addr) -> StdResult<Option<u64>> {
    CUSTOM_ACCOUNT_FEES.may_load(deps.storage, account)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::mock_dependencies;

    #[test]
    fn test_default_config() {
        let recipient = Addr::unchecked("fee_recipient");
        let config = FeeConfig::default_with_recipient(recipient.clone());

        assert_eq!(config.standard_fee_bps, 50);
        assert_eq!(config.discounted_fee_bps, 10);
        assert_eq!(config.cl8y_threshold, Uint128::from(100_000_000u128));
        assert!(config.cl8y_token.is_none());
        assert_eq!(config.fee_recipient, recipient);
    }

    #[test]
    fn test_validate_config() {
        let recipient = Addr::unchecked("fee_recipient");

        // Valid config
        let config = FeeConfig::default_with_recipient(recipient.clone());
        assert!(config.validate().is_ok());

        // Invalid standard fee
        let config = FeeConfig {
            standard_fee_bps: 101,
            ..FeeConfig::default_with_recipient(recipient.clone())
        };
        assert!(config.validate().is_err());

        // Invalid discounted fee
        let config = FeeConfig {
            discounted_fee_bps: 101,
            ..FeeConfig::default_with_recipient(recipient)
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_calculate_fee_from_bps() {
        // 0.5% of 1000 = 5
        let fee = calculate_fee_from_bps(Uint128::from(1000u128), 50);
        assert_eq!(fee, Uint128::from(5u128));

        // 1% of 1000 = 10
        let fee = calculate_fee_from_bps(Uint128::from(1000u128), 100);
        assert_eq!(fee, Uint128::from(10u128));

        // 0% of 1000 = 0
        let fee = calculate_fee_from_bps(Uint128::from(1000u128), 0);
        assert_eq!(fee, Uint128::zero());
    }

    #[test]
    fn test_calculate_net_amount() {
        // 1000 - 0.5% = 995
        let net = calculate_net_amount(Uint128::from(1000u128), 50);
        assert_eq!(net, Uint128::from(995u128));

        // 1000 - 1% = 990
        let net = calculate_net_amount(Uint128::from(1000u128), 100);
        assert_eq!(net, Uint128::from(990u128));
    }

    #[test]
    fn test_validate_custom_fee() {
        // Valid fees
        assert!(validate_custom_fee(0).is_ok());
        assert!(validate_custom_fee(50).is_ok());
        assert!(validate_custom_fee(100).is_ok());

        // Invalid fee
        assert!(validate_custom_fee(101).is_err());
    }

    #[test]
    fn test_custom_account_fees() {
        let mut deps = mock_dependencies();
        let account = Addr::unchecked("user1");

        // No custom fee initially
        assert!(!has_custom_fee(deps.as_ref(), &account).unwrap());
        assert!(get_custom_fee(deps.as_ref(), &account).unwrap().is_none());

        // Set custom fee
        set_custom_account_fee(deps.as_mut().storage, &account, 25).unwrap();
        assert!(has_custom_fee(deps.as_ref(), &account).unwrap());
        assert_eq!(get_custom_fee(deps.as_ref(), &account).unwrap(), Some(25));

        // Remove custom fee
        remove_custom_account_fee(deps.as_mut().storage, &account);
        assert!(!has_custom_fee(deps.as_ref(), &account).unwrap());
        assert!(get_custom_fee(deps.as_ref(), &account).unwrap().is_none());
    }

    #[test]
    fn test_fee_type_as_str() {
        assert_eq!(FeeType::Standard.as_str(), "standard");
        assert_eq!(FeeType::Discounted.as_str(), "discounted");
        assert_eq!(FeeType::Custom.as_str(), "custom");
    }

    #[test]
    fn test_custom_fee_priority() {
        let mut deps = mock_dependencies();
        let account = Addr::unchecked("user1");
        let recipient = Addr::unchecked("fee_recipient");
        let config = FeeConfig::default_with_recipient(recipient);

        // Standard fee by default
        let fee_bps = get_effective_fee_bps(deps.as_ref(), &config, &account).unwrap();
        assert_eq!(fee_bps, 50);

        let fee_type = get_fee_type(deps.as_ref(), &config, &account).unwrap();
        assert_eq!(fee_type.as_str(), "standard");

        // Set custom fee
        set_custom_account_fee(deps.as_mut().storage, &account, 25).unwrap();
        let fee_bps = get_effective_fee_bps(deps.as_ref(), &config, &account).unwrap();
        assert_eq!(fee_bps, 25);

        let fee_type = get_fee_type(deps.as_ref(), &config, &account).unwrap();
        assert_eq!(fee_type.as_str(), "custom");
    }
}
