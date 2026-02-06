//! CW20 Token Helpers
//!
//! Provides utilities for CW20 token operations on Terra Classic.

use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// CW20 execute messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cw20ExecuteMsg {
    /// Transfer tokens to another address
    Transfer { recipient: String, amount: String },
    /// Send tokens to a contract (with message)
    Send {
        contract: String,
        amount: String,
        msg: String,
    },
    /// Increase allowance
    IncreaseAllowance {
        spender: String,
        amount: String,
        expires: Option<Expiration>,
    },
    /// Decrease allowance
    DecreaseAllowance {
        spender: String,
        amount: String,
        expires: Option<Expiration>,
    },
}

/// CW20 query messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cw20QueryMsg {
    /// Get token balance
    Balance { address: String },
    /// Get token info
    TokenInfo {},
    /// Get allowance
    Allowance { owner: String, spender: String },
}

/// Expiration type for allowances
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expiration {
    AtHeight(u64),
    AtTime(String),
    Never {},
}

/// CW20 balance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub balance: String,
}

/// CW20 token info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfoResponse {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: String,
}

/// CW20 allowance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowanceResponse {
    pub allowance: String,
    pub expires: Expiration,
}

/// Build a CW20 transfer message
pub fn build_cw20_transfer_msg(recipient: &str, amount: u128) -> Cw20ExecuteMsg {
    Cw20ExecuteMsg::Transfer {
        recipient: recipient.to_string(),
        amount: amount.to_string(),
    }
}

/// Build a CW20 send message (for sending to contracts)
pub fn build_cw20_send_msg(contract: &str, amount: u128, msg: &str) -> Cw20ExecuteMsg {
    use base64::Engine;
    Cw20ExecuteMsg::Send {
        contract: contract.to_string(),
        amount: amount.to_string(),
        msg: base64::engine::general_purpose::STANDARD.encode(msg),
    }
}

/// Build a CW20 increase allowance message
pub fn build_cw20_increase_allowance_msg(spender: &str, amount: u128) -> Cw20ExecuteMsg {
    Cw20ExecuteMsg::IncreaseAllowance {
        spender: spender.to_string(),
        amount: amount.to_string(),
        expires: None,
    }
}

/// Query CW20 token balance
pub async fn query_cw20_balance(lcd_url: &str, token_address: &str, account: &str) -> Result<u128> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .wrap_err("Failed to create HTTP client")?;

    let query = Cw20QueryMsg::Balance {
        address: account.to_string(),
    };
    let query_json = serde_json::to_string(&query)?;
    let query_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, query_json);

    let url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
        lcd_url.trim_end_matches('/'),
        token_address,
        query_b64
    );

    let response = client
        .get(&url)
        .send()
        .await
        .wrap_err("Failed to query balance")?;

    if !response.status().is_success() {
        return Err(eyre!(
            "Balance query failed: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let data: serde_json::Value = response.json().await?;
    let balance_response: BalanceResponse = serde_json::from_value(
        data.get("data")
            .ok_or_else(|| eyre!("Missing data field"))?
            .clone(),
    )?;

    balance_response
        .balance
        .parse()
        .map_err(|e| eyre!("Failed to parse balance: {}", e))
}

/// Query CW20 token info
pub async fn query_cw20_token_info(
    lcd_url: &str,
    token_address: &str,
) -> Result<TokenInfoResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .wrap_err("Failed to create HTTP client")?;

    let query = Cw20QueryMsg::TokenInfo {};
    let query_json = serde_json::to_string(&query)?;
    let query_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, query_json);

    let url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
        lcd_url.trim_end_matches('/'),
        token_address,
        query_b64
    );

    let response = client
        .get(&url)
        .send()
        .await
        .wrap_err("Failed to query token info")?;

    if !response.status().is_success() {
        return Err(eyre!(
            "Token info query failed: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let data: serde_json::Value = response.json().await?;
    serde_json::from_value(
        data.get("data")
            .ok_or_else(|| eyre!("Missing data field"))?
            .clone(),
    )
    .map_err(|e| eyre!("Failed to parse token info: {}", e))
}

/// Query CW20 allowance
pub async fn query_cw20_allowance(
    lcd_url: &str,
    token_address: &str,
    owner: &str,
    spender: &str,
) -> Result<u128> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .wrap_err("Failed to create HTTP client")?;

    let query = Cw20QueryMsg::Allowance {
        owner: owner.to_string(),
        spender: spender.to_string(),
    };
    let query_json = serde_json::to_string(&query)?;
    let query_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, query_json);

    let url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
        lcd_url.trim_end_matches('/'),
        token_address,
        query_b64
    );

    let response = client
        .get(&url)
        .send()
        .await
        .wrap_err("Failed to query allowance")?;

    if !response.status().is_success() {
        return Err(eyre!(
            "Allowance query failed: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let data: serde_json::Value = response.json().await?;
    let allowance_response: AllowanceResponse = serde_json::from_value(
        data.get("data")
            .ok_or_else(|| eyre!("Missing data field"))?
            .clone(),
    )?;

    allowance_response
        .allowance
        .parse()
        .map_err(|e| eyre!("Failed to parse allowance: {}", e))
}

/// Query native token balance (uluna, etc.)
pub async fn query_native_balance(lcd_url: &str, address: &str, denom: &str) -> Result<u128> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .wrap_err("Failed to create HTTP client")?;

    let url = format!(
        "{}/cosmos/bank/v1beta1/balances/{}/by_denom?denom={}",
        lcd_url.trim_end_matches('/'),
        address,
        denom
    );

    let response = client
        .get(&url)
        .send()
        .await
        .wrap_err("Failed to query native balance")?;

    if !response.status().is_success() {
        return Err(eyre!(
            "Balance query failed: {} - {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let data: serde_json::Value = response.json().await?;
    let amount = data
        .get("balance")
        .and_then(|b| b.get("amount"))
        .and_then(|a| a.as_str())
        .unwrap_or("0");

    amount
        .parse()
        .map_err(|e| eyre!("Failed to parse balance: {}", e))
}

/// Convert a human-readable amount to raw token units
pub fn to_token_units(amount: f64, decimals: u8) -> u128 {
    let multiplier = 10u64.pow(decimals as u32);
    (amount * multiplier as f64) as u128
}

/// Convert raw token units to human-readable amount
pub fn from_token_units(raw: u128, decimals: u8) -> f64 {
    let divisor = 10u64.pow(decimals as u32);
    raw as f64 / divisor as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_token_units() {
        // 1.5 tokens with 6 decimals (like LUNA)
        let result = to_token_units(1.5, 6);
        assert_eq!(result, 1_500_000);

        // 100 tokens with 6 decimals
        let result = to_token_units(100.0, 6);
        assert_eq!(result, 100_000_000);
    }

    #[test]
    fn test_from_token_units() {
        // 1.5 LUNA in uluna
        let result = from_token_units(1_500_000, 6);
        assert!((result - 1.5).abs() < 0.0001);

        // 100 tokens in raw units
        let result = from_token_units(100_000_000, 6);
        assert!((result - 100.0).abs() < 0.0001);
    }

    #[test]
    fn test_cw20_transfer_msg() {
        let msg = build_cw20_transfer_msg("terra1...", 1000000);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("transfer"));
        assert!(json.contains("1000000"));
    }

    #[test]
    fn test_cw20_send_msg() {
        let msg = build_cw20_send_msg("terra1...", 1000000, r#"{"deposit":{}}"#);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("send"));
        assert!(json.contains("contract"));
    }

    #[test]
    fn test_cw20_increase_allowance_msg() {
        let msg = build_cw20_increase_allowance_msg("terra1...", 1000000);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("increase_allowance"));
        assert!(json.contains("spender"));
    }
}
