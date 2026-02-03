//! Terra Classic LCD Client for transaction signing and broadcasting
//!
//! Implements transaction signing using cosmrs and broadcasting via LCD REST API.
//! Falls back to raw HTTP if cosmrs has compatibility issues.

#![allow(dead_code)]

use std::time::Duration;

use bip39::Mnemonic;
use cosmrs::{
    bip32::DerivationPath,
    crypto::secp256k1::SigningKey,
    tx::{self, Fee, Msg, SignDoc, SignerInfo},
    AccountId, Coin,
};
use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Terra Classic LCD endpoints for fallback
pub const MAINNET_LCD_ENDPOINTS: &[&str] = &[
    "https://terra-classic-lcd.publicnode.com",
    "https://api-lunc-lcd.binodes.com",
    "https://lcd.terra-classic.hexxagon.io",
];

pub const TESTNET_LCD_ENDPOINTS: &[&str] = &[
    "https://lcd.luncblaze.com",
    "https://lcd.terra-classic.hexxagon.dev",
];

/// Terra Classic FCD for gas prices
pub const MAINNET_FCD_URL: &str = "https://terra-classic-fcd.publicnode.com";
pub const TESTNET_FCD_URL: &str = "https://fcd.luncblaze.com";

/// Terra derivation path (same as Cosmos)
const TERRA_DERIVATION_PATH: &str = "m/44'/330'/0'/0/0";

/// Terra client for signing and broadcasting transactions
pub struct TerraClient {
    /// Primary LCD URL
    lcd_url: String,
    /// Fallback LCD URLs
    fallback_urls: Vec<String>,
    /// Chain ID
    chain_id: String,
    /// Signing key derived from mnemonic
    signing_key: SigningKey,
    /// Account address
    pub address: AccountId,
    /// HTTP client
    client: Client,
}

/// Account info from LCD
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub sequence: u64,
    pub account_number: u64,
}

/// Transaction broadcast response
#[derive(Debug, Clone, Deserialize)]
pub struct BroadcastResponse {
    pub txhash: String,
    pub code: Option<u32>,
    pub raw_log: Option<String>,
}

/// Gas prices response from FCD
#[derive(Debug, Clone, Deserialize)]
pub struct GasPrices {
    pub uluna: String,
    #[serde(default)]
    pub uusd: Option<String>,
}

impl TerraClient {
    /// Create a new Terra client from mnemonic
    pub fn new(lcd_url: &str, chain_id: &str, mnemonic: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .wrap_err("Failed to create HTTP client")?;

        // Parse mnemonic and derive signing key
        let mnemonic = Mnemonic::parse(mnemonic).map_err(|e| eyre!("Invalid mnemonic: {}", e))?;

        let seed = mnemonic.to_seed("");
        let path: DerivationPath = TERRA_DERIVATION_PATH
            .parse()
            .map_err(|e| eyre!("Invalid derivation path: {:?}", e))?;

        let signing_key = SigningKey::derive_from_path(seed, &path)
            .map_err(|e| eyre!("Failed to derive signing key: {}", e))?;

        // Get account address
        let public_key = signing_key.public_key();
        let address = public_key
            .account_id("terra")
            .map_err(|e| eyre!("Failed to get account ID: {}", e))?;

        // Determine fallback URLs based on chain ID
        let fallback_urls = if chain_id == "columbus-5" {
            MAINNET_LCD_ENDPOINTS
                .iter()
                .filter(|u| **u != lcd_url)
                .map(|s| s.to_string())
                .collect()
        } else if chain_id == "rebel-2" {
            TESTNET_LCD_ENDPOINTS
                .iter()
                .filter(|u| **u != lcd_url)
                .map(|s| s.to_string())
                .collect()
        } else {
            // LocalTerra - no fallbacks
            vec![]
        };

        info!(
            address = %address,
            chain_id = chain_id,
            "Terra client initialized"
        );

        Ok(Self {
            lcd_url: lcd_url.to_string(),
            fallback_urls,
            chain_id: chain_id.to_string(),
            signing_key,
            address,
            client,
        })
    }

    /// Get account info (sequence and account number)
    pub async fn get_account_info(&self) -> Result<AccountInfo> {
        let url = format!(
            "{}/cosmos/auth/v1beta1/accounts/{}",
            self.lcd_url, self.address
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .wrap_err("Failed to query account info")?;

        if !response.status().is_success() {
            return Err(eyre!(
                "Account query failed: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let data: serde_json::Value = response.json().await?;

        // Handle different account response formats
        let account = data
            .get("account")
            .ok_or_else(|| eyre!("Missing 'account' field in response"))?;

        // Try to get sequence and account_number
        let sequence = account
            .get("sequence")
            .or_else(|| account.get("base_account").and_then(|b| b.get("sequence")))
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        let account_number = account
            .get("account_number")
            .or_else(|| {
                account
                    .get("base_account")
                    .and_then(|b| b.get("account_number"))
            })
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        Ok(AccountInfo {
            sequence,
            account_number,
        })
    }

    /// Get current gas prices from FCD
    pub async fn get_gas_prices(&self) -> Result<GasPrices> {
        let fcd_url = if self.chain_id == "columbus-5" {
            MAINNET_FCD_URL
        } else {
            TESTNET_FCD_URL
        };

        let url = format!("{}/v1/txs/gas_prices", fcd_url);

        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await?),
            _ => {
                // Default gas prices if FCD is unavailable
                warn!("Could not fetch gas prices, using defaults");
                Ok(GasPrices {
                    uluna: "0.015".to_string(),
                    uusd: Some("0.15".to_string()),
                })
            }
        }
    }

    /// Sign and broadcast a CosmWasm execute message
    pub async fn execute_contract(
        &self,
        contract_address: &str,
        msg: &impl Serialize,
        funds: Vec<(String, u128)>,
    ) -> Result<String> {
        // Get account info for signing
        let account_info = self.get_account_info().await?;

        // Get gas prices
        let gas_prices = self.get_gas_prices().await?;
        let gas_price: f64 = gas_prices.uluna.parse().unwrap_or(0.015);

        // Estimate gas (we'll use a reasonable default and simulate if needed)
        let gas_limit: u64 = 500_000;
        let fee_amount = ((gas_limit as f64) * gas_price).ceil() as u128;

        // Build the message
        let msg_json = serde_json::to_vec(msg)?;

        // Convert funds to coins
        let coins: Vec<Coin> = funds
            .iter()
            .map(|(denom, amount)| Coin {
                denom: denom.parse().unwrap(),
                amount: *amount,
            })
            .collect();

        // Create the MsgExecuteContract
        let execute_msg = cosmrs::cosmwasm::MsgExecuteContract {
            sender: self.address.clone(),
            contract: contract_address
                .parse()
                .map_err(|e| eyre!("Invalid contract address: {:?}", e))?,
            msg: msg_json,
            funds: coins,
        };

        // Build transaction body
        let body = tx::Body::new(
            vec![execute_msg
                .to_any()
                .map_err(|e| eyre!("Failed to convert message: {}", e))?],
            "",
            0u32,
        );

        // Build auth info
        let public_key = self.signing_key.public_key();
        let signer_info = SignerInfo::single_direct(Some(public_key), account_info.sequence);

        let fee = Fee::from_amount_and_gas(
            Coin {
                denom: "uluna".parse().unwrap(),
                amount: fee_amount,
            },
            gas_limit,
        );

        let auth_info = signer_info.auth_info(fee);

        // Create sign doc
        let chain_id = self
            .chain_id
            .parse()
            .map_err(|_| eyre!("Invalid chain ID"))?;

        let sign_doc = SignDoc::new(&body, &auth_info, &chain_id, account_info.account_number)
            .map_err(|e| eyre!("Failed to create sign doc: {}", e))?;

        // Sign the transaction
        let tx_raw = sign_doc
            .sign(&self.signing_key)
            .map_err(|e| eyre!("Failed to sign transaction: {}", e))?;

        // Serialize and broadcast
        let tx_bytes = tx_raw
            .to_bytes()
            .map_err(|e| eyre!("Failed to serialize transaction: {}", e))?;

        self.broadcast_tx(&tx_bytes).await
    }

    /// Broadcast a signed transaction
    async fn broadcast_tx(&self, tx_bytes: &[u8]) -> Result<String> {
        // Try primary URL first, then fallbacks
        let urls: Vec<&str> = std::iter::once(self.lcd_url.as_str())
            .chain(self.fallback_urls.iter().map(|s| s.as_str()))
            .collect();

        let tx_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, tx_bytes);

        let broadcast_request = serde_json::json!({
            "tx_bytes": tx_b64,
            "mode": "BROADCAST_MODE_SYNC"
        });

        let mut last_error = None;

        for url in urls {
            let broadcast_url = format!("{}/cosmos/tx/v1beta1/txs", url);

            debug!(url = %broadcast_url, "Broadcasting transaction");

            match self
                .client
                .post(&broadcast_url)
                .json(&broadcast_request)
                .send()
                .await
            {
                Ok(response) => {
                    let status = response.status();
                    let body: serde_json::Value = response.json().await.unwrap_or_else(
                        |_| serde_json::json!({"error": "Failed to parse response"}),
                    );

                    if status.is_success() {
                        // Check for tx response
                        if let Some(tx_response) = body.get("tx_response") {
                            let code = tx_response
                                .get("code")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);

                            if code == 0 {
                                let txhash = tx_response
                                    .get("txhash")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                info!(txhash = %txhash, "Transaction broadcast successful");
                                return Ok(txhash);
                            } else {
                                let raw_log = tx_response
                                    .get("raw_log")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Unknown error");

                                last_error =
                                    Some(eyre!("Transaction failed (code {}): {}", code, raw_log));
                                continue;
                            }
                        }
                    }

                    last_error = Some(eyre!("Broadcast failed: {}", body));
                }
                Err(e) => {
                    warn!(url = %url, error = %e, "Failed to broadcast to endpoint");
                    last_error = Some(eyre!("Network error: {}", e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| eyre!("All broadcast attempts failed")))
    }

    /// Execute contract using raw HTTP (fallback if cosmrs has issues)
    pub async fn execute_contract_raw_http(
        &self,
        contract_address: &str,
        msg: &impl Serialize,
        _funds: Vec<(String, u128)>,
    ) -> Result<String> {
        // This is a fallback implementation using the LCD's amino/JSON signing
        // For Terra Classic, we may need to use this if cosmrs has compatibility issues

        warn!("Using raw HTTP fallback for transaction signing");

        // Get account info
        let account_info = self.get_account_info().await?;

        // For raw HTTP, we need to construct the transaction manually
        // This requires the legacy amino signing which is more complex
        // For now, return an error directing to use simulate/estimate approach

        Err(eyre!(
            "Raw HTTP signing not yet implemented. Account: {:?}, sequence: {}",
            self.address,
            account_info.sequence
        ))
    }
}

/// Helper to build a CancelWithdrawApproval message
#[derive(Debug, Clone, Serialize)]
pub struct CancelWithdrawApprovalMsg {
    pub cancel_withdraw_approval: CancelWithdrawApprovalInner,
}

#[derive(Debug, Clone, Serialize)]
pub struct CancelWithdrawApprovalInner {
    pub withdraw_hash: String,
}

impl CancelWithdrawApprovalMsg {
    pub fn new(withdraw_hash: [u8; 32]) -> Self {
        use base64::Engine;
        Self {
            cancel_withdraw_approval: CancelWithdrawApprovalInner {
                withdraw_hash: base64::engine::general_purpose::STANDARD.encode(withdraw_hash),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derivation_path() {
        // Test that derivation path is valid
        let path: Result<DerivationPath, _> = TERRA_DERIVATION_PATH.parse();
        assert!(path.is_ok());
    }

    #[test]
    fn test_mnemonic_parsing() {
        // Test with a valid 12-word mnemonic
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let parsed = Mnemonic::parse(mnemonic);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_cancel_msg_serialization() {
        let msg = CancelWithdrawApprovalMsg::new([0u8; 32]);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("cancel_withdraw_approval"));
        assert!(json.contains("withdraw_hash"));
    }
}
