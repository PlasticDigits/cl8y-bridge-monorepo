//! Terra Transaction Signing Module
//!
//! Provides a dedicated signing interface for Terra Classic transactions,
//! extracted from the client module for clean separation of concerns.
//!
//! ## Features
//!
//! - Mnemonic-based key derivation (BIP44 coin type 330)
//! - Transaction signing with cosmrs
//! - Sequence (nonce) management with automatic refresh
//! - Gas estimation with FCD integration
//! - Retry logic for sequence mismatch errors

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
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Terra derivation path (BIP44 coin type 330)
pub const TERRA_DERIVATION_PATH: &str = "m/44'/330'/0'/0/0";

/// Default gas limit for contract execution
pub const DEFAULT_GAS_LIMIT: u64 = 500_000;

/// Default gas price in uluna
pub const DEFAULT_GAS_PRICE: f64 = 0.015;

/// Configuration for the Terra signer
#[derive(Clone)]
pub struct TerraSignerConfig {
    /// LCD URL for broadcasting
    pub lcd_url: String,
    /// Chain ID
    pub chain_id: String,
    /// Mnemonic phrase
    pub mnemonic: String,
    /// Custom gas limit (defaults to DEFAULT_GAS_LIMIT)
    pub gas_limit: Option<u64>,
    /// Custom derivation path (defaults to TERRA_DERIVATION_PATH)
    pub derivation_path: Option<String>,
}

impl fmt::Debug for TerraSignerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerraSignerConfig")
            .field("lcd_url", &self.lcd_url)
            .field("chain_id", &self.chain_id)
            .field("mnemonic", &"<redacted>")
            .field("gas_limit", &self.gas_limit)
            .field("derivation_path", &self.derivation_path)
            .finish()
    }
}

/// Terra transaction signer with sequence management
pub struct TerraSigner {
    /// Signing key derived from mnemonic
    signing_key: SigningKey,
    /// Account address
    address: AccountId,
    /// LCD URL
    lcd_url: String,
    /// Chain ID
    chain_id: String,
    /// HTTP client
    client: Client,
    /// Gas limit for transactions
    gas_limit: u64,
}

/// Account info from LCD (sequence = nonce)
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    /// Transaction sequence number (nonce)
    pub sequence: u64,
    /// Account number on chain
    pub account_number: u64,
}

/// Gas estimation result
#[derive(Debug, Clone)]
pub struct GasEstimate {
    /// Gas limit to use
    pub gas_limit: u64,
    /// Gas price in uluna
    pub gas_price: f64,
    /// Total fee in uluna
    pub fee_amount: u128,
}

/// Gas prices from FCD
#[derive(Debug, Clone, Deserialize)]
pub struct GasPrices {
    pub uluna: String,
    #[serde(default)]
    pub uusd: Option<String>,
}

/// Transaction result after broadcast and confirmation
#[derive(Debug, Clone)]
pub struct TerraTxResult {
    /// Transaction hash
    pub tx_hash: String,
    /// Block height (if confirmed)
    pub height: Option<u64>,
    /// Whether the transaction was successful.
    /// `false` when the chain returned a non-zero code **or** when
    /// confirmation timed out (see `confirmed` field).
    pub success: bool,
    /// Whether the transaction was confirmed in a block.
    /// `false` means the broadcast succeeded but we could not verify
    /// inclusion within the timeout window. Callers should treat
    /// unconfirmed transactions as pending, not as successful.
    pub confirmed: bool,
    /// Raw log from the chain
    pub raw_log: Option<String>,
}

/// FCD endpoints for gas price queries
pub const MAINNET_FCD_URL: &str = "https://terra-classic-fcd.publicnode.com";
pub const TESTNET_FCD_URL: &str = "https://fcd.luncblaze.com";

impl TerraSigner {
    /// Create a new Terra signer from configuration
    pub fn new(config: TerraSignerConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .wrap_err("Failed to create HTTP client")?;

        let derivation_path = config
            .derivation_path
            .as_deref()
            .unwrap_or(TERRA_DERIVATION_PATH);

        // Parse mnemonic and derive signing key
        let mnemonic =
            Mnemonic::parse(&config.mnemonic).map_err(|e| eyre!("Invalid mnemonic: {}", e))?;

        let seed = mnemonic.to_seed("");
        let path: DerivationPath = derivation_path
            .parse()
            .map_err(|e| eyre!("Invalid derivation path: {:?}", e))?;

        let signing_key = SigningKey::derive_from_path(seed, &path)
            .map_err(|e| eyre!("Failed to derive signing key: {}", e))?;

        let public_key = signing_key.public_key();
        let address = public_key
            .account_id("terra")
            .map_err(|e| eyre!("Failed to get account ID: {}", e))?;

        let gas_limit = config.gas_limit.unwrap_or(DEFAULT_GAS_LIMIT);

        info!(
            address = %address,
            chain_id = %config.chain_id,
            gas_limit = gas_limit,
            "Terra signer initialized"
        );

        Ok(Self {
            signing_key,
            address,
            lcd_url: config.lcd_url.trim_end_matches('/').to_string(),
            chain_id: config.chain_id,
            client,
            gas_limit,
        })
    }

    /// Create from mnemonic string
    pub fn from_mnemonic(lcd_url: &str, chain_id: &str, mnemonic: &str) -> Result<Self> {
        Self::new(TerraSignerConfig {
            lcd_url: lcd_url.to_string(),
            chain_id: chain_id.to_string(),
            mnemonic: mnemonic.to_string(),
            gas_limit: None,
            derivation_path: None,
        })
    }

    /// Get the signer's address
    pub fn address(&self) -> &AccountId {
        &self.address
    }

    /// Get the address as a string
    pub fn address_str(&self) -> String {
        self.address.to_string()
    }

    /// Get the chain ID
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }

    /// Get the LCD URL
    pub fn lcd_url(&self) -> &str {
        &self.lcd_url
    }

    /// Get the public key
    pub fn public_key(&self) -> cosmrs::crypto::PublicKey {
        self.signing_key.public_key()
    }

    // =========================================================================
    // Sequence (Nonce) Management
    // =========================================================================

    /// Get current account info (sequence and account number)
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
        let account = data
            .get("account")
            .ok_or_else(|| eyre!("Missing 'account' field in response"))?;

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

    /// Get just the current sequence number
    pub async fn get_sequence(&self) -> Result<u64> {
        let info = self.get_account_info().await?;
        Ok(info.sequence)
    }

    // =========================================================================
    // Gas Estimation
    // =========================================================================

    /// Get current gas prices from FCD
    pub async fn get_gas_prices(&self) -> Result<GasPrices> {
        let fcd_url = match self.chain_id.as_str() {
            "columbus-5" => MAINNET_FCD_URL,
            _ => TESTNET_FCD_URL,
        };

        let url = format!("{}/v1/txs/gas_prices", fcd_url);

        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await?),
            _ => {
                warn!("Could not fetch gas prices, using defaults");
                Ok(GasPrices {
                    uluna: DEFAULT_GAS_PRICE.to_string(),
                    uusd: Some("0.15".to_string()),
                })
            }
        }
    }

    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self) -> Result<GasEstimate> {
        let gas_prices = self.get_gas_prices().await?;
        let gas_price: f64 = gas_prices.uluna.parse().unwrap_or_else(|_| {
            warn!(
                raw_value = %gas_prices.uluna,
                default = DEFAULT_GAS_PRICE,
                "Failed to parse uluna gas price, using default"
            );
            DEFAULT_GAS_PRICE
        });
        let fee_amount = ((self.gas_limit as f64) * gas_price).ceil() as u128;

        Ok(GasEstimate {
            gas_limit: self.gas_limit,
            gas_price,
            fee_amount,
        })
    }

    /// Estimate gas with a custom gas limit
    pub async fn estimate_gas_with_limit(&self, gas_limit: u64) -> Result<GasEstimate> {
        let gas_prices = self.get_gas_prices().await?;
        let gas_price: f64 = gas_prices.uluna.parse().unwrap_or_else(|_| {
            warn!(
                raw_value = %gas_prices.uluna,
                default = DEFAULT_GAS_PRICE,
                "Failed to parse uluna gas price, using default"
            );
            DEFAULT_GAS_PRICE
        });
        let fee_amount = ((gas_limit as f64) * gas_price).ceil() as u128;

        Ok(GasEstimate {
            gas_limit,
            gas_price,
            fee_amount,
        })
    }

    /// Calculate bumped gas price for retries
    pub fn calculate_bumped_gas_price(&self, base_price: f64, bump_percent: u32) -> f64 {
        let multiplier = 1.0 + (bump_percent as f64 / 100.0);
        base_price * multiplier
    }

    // =========================================================================
    // Transaction Signing
    // =========================================================================

    /// Sign and broadcast a CosmWasm execute message
    pub async fn sign_and_broadcast_execute(
        &self,
        contract_address: &str,
        msg: &impl Serialize,
        funds: Vec<(String, u128)>,
    ) -> Result<TerraTxResult> {
        let account_info = self.get_account_info().await?;
        let gas_estimate = self.estimate_gas().await?;

        debug!(
            sequence = account_info.sequence,
            account_number = account_info.account_number,
            gas_limit = gas_estimate.gas_limit,
            fee = gas_estimate.fee_amount,
            "Signing Terra transaction"
        );

        self.sign_and_broadcast_execute_inner(
            contract_address,
            msg,
            &funds,
            &account_info,
            &gas_estimate,
        )
        .await
    }

    /// Sign and broadcast with retry on sequence mismatch
    pub async fn sign_and_broadcast_with_retry(
        &self,
        contract_address: &str,
        msg: &impl Serialize,
        funds: Vec<(String, u128)>,
        retry_config: &TerraRetryConfig,
    ) -> Result<TerraTxResult> {
        let mut last_error = None;

        for attempt in 0..retry_config.max_retries {
            match self
                .sign_and_broadcast_execute(contract_address, msg, funds.clone())
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let error_str = e.to_string();

                    if error_str.contains("account sequence mismatch")
                        || error_str.contains("code 32")
                        || error_str.contains("incorrect account sequence")
                    {
                        warn!(
                            attempt = attempt + 1,
                            max_retries = retry_config.max_retries,
                            error = %e,
                            "Sequence mismatch, refreshing and retrying"
                        );

                        let delay = retry_config.backoff_for_attempt(attempt);
                        tokio::time::sleep(delay).await;

                        last_error = Some(e);
                        continue;
                    }

                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            eyre!(
                "Transaction failed after {} retries",
                retry_config.max_retries
            )
        }))
    }

    /// Inner signing and broadcasting logic
    async fn sign_and_broadcast_execute_inner(
        &self,
        contract_address: &str,
        msg: &impl Serialize,
        funds: &[(String, u128)],
        account_info: &AccountInfo,
        gas_estimate: &GasEstimate,
    ) -> Result<TerraTxResult> {
        let msg_json = serde_json::to_vec(msg)?;

        let coins: Vec<Coin> = funds
            .iter()
            .map(|(denom, amount)| {
                let denom_parsed = denom
                    .parse()
                    .map_err(|e| eyre!("Invalid coin denom '{}': {}", denom, e))?;
                Ok::<_, eyre::Report>(Coin {
                    denom: denom_parsed,
                    amount: *amount,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let execute_msg = cosmrs::cosmwasm::MsgExecuteContract {
            sender: self.address.clone(),
            contract: contract_address
                .parse()
                .map_err(|e| eyre!("Invalid contract address: {:?}", e))?,
            msg: msg_json,
            funds: coins,
        };

        let body = tx::Body::new(
            vec![execute_msg
                .to_any()
                .map_err(|e| eyre!("Failed to convert message: {}", e))?],
            "",
            0u32,
        );

        let public_key = self.signing_key.public_key();
        let signer_info = SignerInfo::single_direct(Some(public_key), account_info.sequence);

        let fee = Fee::from_amount_and_gas(
            Coin {
                denom: "uluna"
                    .parse()
                    .expect("uluna is a valid constant Terra denom"),
                amount: gas_estimate.fee_amount,
            },
            gas_estimate.gas_limit,
        );

        let auth_info = signer_info.auth_info(fee);

        let chain_id = self
            .chain_id
            .parse()
            .map_err(|_| eyre!("Invalid chain ID"))?;

        let sign_doc = SignDoc::new(&body, &auth_info, &chain_id, account_info.account_number)
            .map_err(|e| eyre!("Failed to create sign doc: {}", e))?;

        let tx_raw = sign_doc
            .sign(&self.signing_key)
            .map_err(|e| eyre!("Failed to sign transaction: {}", e))?;

        let tx_bytes = tx_raw
            .to_bytes()
            .map_err(|e| eyre!("Failed to serialize transaction: {}", e))?;

        self.broadcast_and_confirm(&tx_bytes).await
    }

    // =========================================================================
    // Broadcasting
    // =========================================================================

    /// Broadcast a signed transaction and wait for confirmation
    async fn broadcast_and_confirm(&self, tx_bytes: &[u8]) -> Result<TerraTxResult> {
        let tx_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, tx_bytes);

        let broadcast_request = serde_json::json!({
            "tx_bytes": tx_b64,
            "mode": "BROADCAST_MODE_SYNC"
        });

        let broadcast_url = format!("{}/cosmos/tx/v1beta1/txs", self.lcd_url);

        info!(url = %broadcast_url, "Broadcasting Terra transaction");

        let response = self
            .client
            .post(&broadcast_url)
            .json(&broadcast_request)
            .send()
            .await
            .wrap_err("Failed to broadcast transaction")?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse response"}));

        if !status.is_success() {
            return Err(eyre!("Broadcast failed (HTTP {}): {}", status, body));
        }

        let tx_response = body
            .get("tx_response")
            .ok_or_else(|| eyre!("Missing tx_response in broadcast result: {}", body))?;

        let code = tx_response
            .get("code")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if code != 0 {
            let raw_log = tx_response
                .get("raw_log")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");

            return Err(eyre!("Transaction failed (code {}): {}", code, raw_log));
        }

        let txhash = tx_response
            .get("txhash")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        info!(txhash = %txhash, "Transaction broadcast successful, awaiting confirmation");

        // Wait for confirmation
        match self.wait_for_tx_confirmation(&txhash).await {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!(
                    txhash = %txhash,
                    error = %e,
                    "Broadcast succeeded but confirmation timed out — treating as unconfirmed"
                );
                Ok(TerraTxResult {
                    tx_hash: txhash,
                    height: None,
                    success: false,
                    confirmed: false,
                    raw_log: Some(format!(
                        "Broadcast succeeded, confirmation timed out: {}",
                        e
                    )),
                })
            }
        }
    }

    // =========================================================================
    // Confirmation
    // =========================================================================

    /// Wait for a transaction to be confirmed in a block
    pub async fn wait_for_tx_confirmation(&self, txhash: &str) -> Result<TerraTxResult> {
        let timeout = Duration::from_secs(30);
        let initial_delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(3);

        let start = Instant::now();
        let mut delay = initial_delay;

        let tx_url = format!("{}/cosmos/tx/v1beta1/txs/{}", self.lcd_url, txhash);

        while start.elapsed() < timeout {
            tokio::time::sleep(delay).await;

            match self.client.get(&tx_url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        let body: serde_json::Value = response.json().await.unwrap_or_default();

                        if let Some(tx_response) = body.get("tx_response") {
                            let code = tx_response
                                .get("code")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);

                            let height = tx_response
                                .get("height")
                                .and_then(|v| v.as_str())
                                .and_then(|h| h.parse().ok());

                            let raw_log = tx_response
                                .get("raw_log")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            if code == 0 {
                                debug!(txhash = %txhash, height = ?height, "Transaction confirmed");
                                return Ok(TerraTxResult {
                                    tx_hash: txhash.to_string(),
                                    height,
                                    success: true,
                                    confirmed: true,
                                    raw_log,
                                });
                            } else {
                                return Ok(TerraTxResult {
                                    tx_hash: txhash.to_string(),
                                    height,
                                    success: false,
                                    confirmed: true,
                                    raw_log,
                                });
                            }
                        }
                    } else if response.status().as_u16() == 404 {
                        debug!(txhash = %txhash, "Transaction not yet in block, waiting...");
                    }
                }
                Err(e) => {
                    warn!(txhash = %txhash, error = %e, "Error querying transaction status");
                }
            }

            delay = std::cmp::min(delay * 2, max_delay);
        }

        Err(eyre!(
            "Timeout waiting for transaction {} to be confirmed",
            txhash
        ))
    }

    /// Check if a transaction was successful
    pub async fn check_tx_success(&self, txhash: &str) -> Result<Option<bool>> {
        let tx_url = format!("{}/cosmos/tx/v1beta1/txs/{}", self.lcd_url, txhash);

        match self.client.get(&tx_url).send().await {
            Ok(response) if response.status().is_success() => {
                let body: serde_json::Value = response.json().await.unwrap_or_default();
                if let Some(tx_response) = body.get("tx_response") {
                    let code = tx_response
                        .get("code")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    Ok(Some(code == 0))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}

/// Retry configuration for Terra transactions
#[derive(Debug, Clone)]
pub struct TerraRetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Gas price bump percentage per retry
    pub gas_bump_percent: u32,
}

impl Default for TerraRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(10),
            gas_bump_percent: 20,
        }
    }
}

impl TerraRetryConfig {
    /// Calculate backoff duration for a given attempt
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let backoff_ms = self.initial_backoff.as_millis() * 2u128.pow(attempt);
        Duration::from_millis(backoff_ms.min(self.max_backoff.as_millis()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terra_derivation_path() {
        let path: Result<DerivationPath, _> = TERRA_DERIVATION_PATH.parse();
        assert!(path.is_ok());
    }

    #[test]
    fn test_signer_from_mnemonic() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let config = TerraSignerConfig {
            lcd_url: "http://localhost:1317".to_string(),
            chain_id: "localterra".to_string(),
            mnemonic: mnemonic.to_string(),
            gas_limit: None,
            derivation_path: None,
        };

        let signer = TerraSigner::new(config).unwrap();
        assert!(signer.address_str().starts_with("terra"));
    }

    #[test]
    fn test_retry_config_backoff() {
        let config = TerraRetryConfig::default();

        let backoff0 = config.backoff_for_attempt(0);
        let backoff1 = config.backoff_for_attempt(1);
        let backoff2 = config.backoff_for_attempt(2);

        assert_eq!(backoff0, Duration::from_millis(500));
        assert_eq!(backoff1, Duration::from_secs(1));
        assert_eq!(backoff2, Duration::from_secs(2));
    }

    #[test]
    fn test_gas_bump_calculation() {
        let config = TerraSignerConfig {
            lcd_url: "http://localhost:1317".to_string(),
            chain_id: "localterra".to_string(),
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            gas_limit: None,
            derivation_path: None,
        };

        let signer = TerraSigner::new(config).unwrap();

        let base = 0.015_f64;
        let bumped = signer.calculate_bumped_gas_price(base, 20);
        assert!((bumped - 0.018).abs() < 0.0001);
    }

    #[test]
    fn test_gas_estimate_calculation() {
        // Manual calculation
        let gas_limit = 500_000u64;
        let gas_price = 0.015f64;
        let fee = ((gas_limit as f64) * gas_price).ceil() as u128;
        assert_eq!(fee, 7500);
    }
}
