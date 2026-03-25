//! Decimal normalization matching EVM `Bridge._normalizeDecimals`.

use crate::error::BridgeError;
use anchor_lang::prelude::*;

/// Normalize `amount` from `src_decimals` to `dest_decimals` (same semantics as Solidity).
pub fn normalize_decimals(amount: u128, src_decimals: u8, dest_decimals: u8) -> Result<u128> {
    if src_decimals == dest_decimals {
        return Ok(amount);
    }
    if src_decimals > dest_decimals {
        let exp = (src_decimals - dest_decimals) as u32;
        let div = 10u128.pow(exp);
        amount.checked_div(div).ok_or(BridgeError::ArithmeticOverflow.into())
    } else {
        let exp = (dest_decimals - src_decimals) as u32;
        let mul = 10u128.pow(exp);
        amount.checked_mul(mul).ok_or(BridgeError::ArithmeticOverflow.into())
    }
}
