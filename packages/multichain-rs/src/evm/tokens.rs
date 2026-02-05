//! ERC20 Token Helpers
//!
//! Provides utilities for ERC20 token operations like approve, transfer, and balance checks.

use crate::evm::contracts::ERC20;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
};
use eyre::{eyre, Result};
use std::sync::Arc;

/// Get the ERC20 token balance of an address
pub async fn get_token_balance<P: Provider>(
    provider: Arc<P>,
    token_address: Address,
    account: Address,
) -> Result<U256> {
    let contract = ERC20::new(token_address, provider);
    let balance = contract
        .balanceOf(account)
        .call()
        .await
        .map_err(|e| eyre!("Failed to get balance: {}", e))?;
    Ok(balance._0)
}

/// Get the ERC20 token allowance
pub async fn get_token_allowance<P: Provider>(
    provider: Arc<P>,
    token_address: Address,
    owner: Address,
    spender: Address,
) -> Result<U256> {
    let contract = ERC20::new(token_address, provider);
    let allowance = contract
        .allowance(owner, spender)
        .call()
        .await
        .map_err(|e| eyre!("Failed to get allowance: {}", e))?;
    Ok(allowance._0)
}

/// Get token decimals
pub async fn get_token_decimals<P: Provider>(
    provider: Arc<P>,
    token_address: Address,
) -> Result<u8> {
    let contract = ERC20::new(token_address, provider);
    let decimals = contract
        .decimals()
        .call()
        .await
        .map_err(|e| eyre!("Failed to get decimals: {}", e))?;
    Ok(decimals._0)
}

/// Get token symbol
pub async fn get_token_symbol<P: Provider>(
    provider: Arc<P>,
    token_address: Address,
) -> Result<String> {
    let contract = ERC20::new(token_address, provider);
    let symbol = contract
        .symbol()
        .call()
        .await
        .map_err(|e| eyre!("Failed to get symbol: {}", e))?;
    Ok(symbol._0)
}

/// Get token name
pub async fn get_token_name<P: Provider>(
    provider: Arc<P>,
    token_address: Address,
) -> Result<String> {
    let contract = ERC20::new(token_address, provider);
    let name = contract
        .name()
        .call()
        .await
        .map_err(|e| eyre!("Failed to get name: {}", e))?;
    Ok(name._0)
}

/// Token info helper struct
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub address: Address,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

/// Get complete token info
pub async fn get_token_info<P: Provider>(
    provider: Arc<P>,
    token_address: Address,
) -> Result<TokenInfo> {
    let contract = ERC20::new(token_address, provider);

    let (name, symbol, decimals) = tokio::try_join!(
        async {
            contract
                .name()
                .call()
                .await
                .map(|r| r._0)
                .map_err(|e| eyre!("Failed to get name: {}", e))
        },
        async {
            contract
                .symbol()
                .call()
                .await
                .map(|r| r._0)
                .map_err(|e| eyre!("Failed to get symbol: {}", e))
        },
        async {
            contract
                .decimals()
                .call()
                .await
                .map(|r| r._0)
                .map_err(|e| eyre!("Failed to get decimals: {}", e))
        }
    )?;

    Ok(TokenInfo {
        address: token_address,
        name,
        symbol,
        decimals,
    })
}

/// Convert a human-readable amount to raw token units
pub fn to_token_units(amount: f64, decimals: u8) -> U256 {
    let multiplier = 10u64.pow(decimals as u32);
    let raw = (amount * multiplier as f64) as u128;
    U256::from(raw)
}

/// Convert raw token units to human-readable amount
pub fn from_token_units(raw: U256, decimals: u8) -> f64 {
    let divisor = 10u64.pow(decimals as u32);
    let raw_u128: u128 = raw.try_into().unwrap_or(u128::MAX);
    raw_u128 as f64 / divisor as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_token_units() {
        // 1.5 tokens with 18 decimals
        let result = to_token_units(1.5, 18);
        assert_eq!(result, U256::from(1_500_000_000_000_000_000u128));

        // 100 tokens with 6 decimals (like USDC)
        let result = to_token_units(100.0, 6);
        assert_eq!(result, U256::from(100_000_000u64));
    }

    #[test]
    fn test_from_token_units() {
        // 1.5 ETH in wei
        let result = from_token_units(U256::from(1_500_000_000_000_000_000u128), 18);
        assert!((result - 1.5).abs() < 0.0001);

        // 100 USDC in raw units
        let result = from_token_units(U256::from(100_000_000u64), 6);
        assert!((result - 100.0).abs() < 0.0001);
    }
}
