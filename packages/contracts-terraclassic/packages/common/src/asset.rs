//! Asset type definitions for handling both native and CW20 tokens

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};

/// Represents information about an asset (native or CW20)
#[cw_serde]
pub enum AssetInfo {
    /// Native token identified by denomination (e.g., "uusd", "uluna")
    Native { denom: String },
    /// CW20 token identified by contract address
    Cw20 { contract_addr: Addr },
}

/// Represents an asset with amount
#[cw_serde]
pub struct Asset {
    /// Asset type information
    pub info: AssetInfo,
    /// Amount of the asset
    pub amount: Uint128,
}

impl AssetInfo {
    /// Create a new native asset info
    pub fn native(denom: impl Into<String>) -> Self {
        AssetInfo::Native {
            denom: denom.into(),
        }
    }

    /// Create a new CW20 asset info
    pub fn cw20(contract_addr: Addr) -> Self {
        AssetInfo::Cw20 { contract_addr }
    }

    /// Check if this is a native token
    pub fn is_native(&self) -> bool {
        matches!(self, AssetInfo::Native { .. })
    }

    /// Check if this is a CW20 token
    pub fn is_cw20(&self) -> bool {
        matches!(self, AssetInfo::Cw20 { .. })
    }
}

impl Asset {
    /// Create a new asset
    pub fn new(info: AssetInfo, amount: impl Into<Uint128>) -> Self {
        Asset {
            info,
            amount: amount.into(),
        }
    }

    /// Create a new native asset
    pub fn native(denom: impl Into<String>, amount: impl Into<Uint128>) -> Self {
        Asset {
            info: AssetInfo::native(denom),
            amount: amount.into(),
        }
    }

    /// Create a new CW20 asset
    pub fn cw20(contract_addr: Addr, amount: impl Into<Uint128>) -> Self {
        Asset {
            info: AssetInfo::cw20(contract_addr),
            amount: amount.into(),
        }
    }
}
