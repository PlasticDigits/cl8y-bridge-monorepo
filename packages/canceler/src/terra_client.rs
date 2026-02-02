//! Terra Client for canceler transactions
//!
//! Handles signing and submitting CancelWithdrawApproval transactions to Terra Classic.

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

use crate::hash::bytes32_to_hex;

/// Terra derivation path
const TERRA_DERIVATION_PATH: &str = "m/44'/330'/0'/0/0";

/// Cancel message for Terra contract
#[derive(Debug, Clone, Serialize)]
pub struct CancelWithdrawApprovalMsg {
    pub cancel_withdraw_approval: CancelWithdrawApprovalInner,
}

#[derive(Debug, Clone, Serialize)]
pub struct CancelWithdrawApprovalInner {
    pub withdraw_hash: String,
}

/// Account info from LCD
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub sequence: u64,
    pub account_number: u64,
}

/// Terra client for canceler transactions
pub struct TerraClient {
    lcd_url: String,
    chain_id: String,
    contract_address: String,
    signing_key: SigningKey,
    pub address: AccountId,
    client: Client,
}

impl TerraClient {
    /// Create a new Terra client
    pub fn new(lcd_url: &str, chain_id: &str, contract_address: &str, mnemonic: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .wrap_err("Failed to create HTTP client")?;

        // Parse mnemonic and derive signing key
        let mnemonic = Mnemonic::parse(mnemonic)
            .map_err(|e| eyre!("Invalid mnemonic: {}", e))?;
        
        let seed = mnemonic.to_seed("");
        let path: DerivationPath = TERRA_DERIVATION_PATH.parse()
            .map_err(|e| eyre!("Invalid derivation path: {:?}", e))?;
        
        let signing_key = SigningKey::derive_from_path(seed, &path)
            .map_err(|e| eyre!("Failed to derive signing key: {}", e))?;
        
        // Get account address
        let public_key = signing_key.public_key();
        let address = public_key.account_id("terra")
            .map_err(|e| eyre!("Failed to get account ID: {}", e))?;

        info!(
            canceler_address = %address,
            contract = contract_address,
            "Terra client initialized"
        );

        Ok(Self {
            lcd_url: lcd_url.to_string(),
            chain_id: chain_id.to_string(),
            contract_address: contract_address.to_string(),
            signing_key,
            address,
            client,
        })
    }

    /// Get account info (sequence and account number)
    async fn get_account_info(&self) -> Result<AccountInfo> {
        let url = format!(
            "{}/cosmos/auth/v1beta1/accounts/{}",
            self.lcd_url, self.address
        );

        let response = self.client.get(&url).send().await
            .wrap_err("Failed to query account info")?;
        
        if !response.status().is_success() {
            return Err(eyre!(
                "Account query failed: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let data: serde_json::Value = response.json().await?;
        
        let account = data.get("account")
            .ok_or_else(|| eyre!("Missing 'account' field in response"))?;
        
        let sequence = account.get("sequence")
            .or_else(|| account.get("base_account").and_then(|b| b.get("sequence")))
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        
        let account_number = account.get("account_number")
            .or_else(|| account.get("base_account").and_then(|b| b.get("account_number")))
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        Ok(AccountInfo {
            sequence,
            account_number,
        })
    }

    /// Cancel a withdraw approval on Terra
    pub async fn cancel_withdraw_approval(&self, withdraw_hash: [u8; 32]) -> Result<String> {
        // Build the cancel message
        let msg = CancelWithdrawApprovalMsg {
            cancel_withdraw_approval: CancelWithdrawApprovalInner {
                withdraw_hash: base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    withdraw_hash,
                ),
            },
        };

        debug!(
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            "Submitting CancelWithdrawApproval to Terra"
        );

        // Get account info
        let account_info = self.get_account_info().await?;

        // Estimate gas
        let gas_limit: u64 = 300_000;
        let gas_price: f64 = 0.015;
        let fee_amount = ((gas_limit as f64) * gas_price).ceil() as u128;

        // Build the message
        let msg_json = serde_json::to_vec(&msg)?;

        let execute_msg = cosmrs::cosmwasm::MsgExecuteContract {
            sender: self.address.clone(),
            contract: self.contract_address.parse()
                .map_err(|e| eyre!("Invalid contract address: {:?}", e))?,
            msg: msg_json,
            funds: vec![],
        };

        // Build transaction body
        let body = tx::Body::new(
            vec![execute_msg.to_any().map_err(|e| eyre!("Failed to convert message: {}", e))?],
            "",
            0u32,
        );

        // Build auth info
        let public_key = self.signing_key.public_key();
        let signer_info = SignerInfo::single_direct(
            Some(public_key),
            account_info.sequence,
        );

        let fee = Fee::from_amount_and_gas(
            Coin {
                denom: "uluna".parse().unwrap(),
                amount: fee_amount,
            },
            gas_limit,
        );

        let auth_info = signer_info.auth_info(fee);

        // Create sign doc
        let chain_id = self.chain_id.parse()
            .map_err(|_| eyre!("Invalid chain ID"))?;
        
        let sign_doc = SignDoc::new(
            &body,
            &auth_info,
            &chain_id,
            account_info.account_number,
        ).map_err(|e| eyre!("Failed to create sign doc: {}", e))?;

        // Sign the transaction
        let tx_raw = sign_doc.sign(&self.signing_key)
            .map_err(|e| eyre!("Failed to sign transaction: {}", e))?;

        // Serialize and broadcast
        let tx_bytes = tx_raw.to_bytes()
            .map_err(|e| eyre!("Failed to serialize transaction: {}", e))?;

        let tx_hash = self.broadcast_tx(&tx_bytes).await?;

        info!(
            tx_hash = %tx_hash,
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            "Approval successfully cancelled on Terra"
        );

        Ok(tx_hash)
    }

    /// Broadcast a signed transaction
    async fn broadcast_tx(&self, tx_bytes: &[u8]) -> Result<String> {
        let tx_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            tx_bytes,
        );

        let broadcast_request = serde_json::json!({
            "tx_bytes": tx_b64,
            "mode": "BROADCAST_MODE_SYNC"
        });

        let broadcast_url = format!("{}/cosmos/tx/v1beta1/txs", self.lcd_url);
        
        debug!(url = %broadcast_url, "Broadcasting transaction");

        let response = self.client
            .post(&broadcast_url)
            .json(&broadcast_request)
            .send()
            .await
            .map_err(|e| eyre!("Failed to broadcast: {}", e))?;

        let status = response.status();
        let body: serde_json::Value = response.json().await
            .unwrap_or_else(|_| serde_json::json!({"error": "Failed to parse response"}));

        if status.is_success() {
            if let Some(tx_response) = body.get("tx_response") {
                let code = tx_response.get("code")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                
                if code == 0 {
                    let txhash = tx_response.get("txhash")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    return Ok(txhash);
                } else {
                    let raw_log = tx_response.get("raw_log")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");
                    
                    return Err(eyre!("Transaction failed (code {}): {}", code, raw_log));
                }
            }
        }

        Err(eyre!("Broadcast failed: {}", body))
    }

    /// Check if an approval can be cancelled
    pub async fn can_cancel(&self, withdraw_hash: [u8; 32]) -> Result<bool> {
        let query = serde_json::json!({
            "withdraw_approval": {
                "withdraw_hash": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    withdraw_hash,
                )
            }
        });

        let query_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            serde_json::to_string(&query)?,
        );

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            self.lcd_url, self.contract_address, query_b64
        );

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let json: serde_json::Value = resp.json().await?;
                
                let exists = json["data"]["exists"].as_bool().unwrap_or(false);
                let cancelled = json["data"]["cancelled"].as_bool().unwrap_or(false);
                let executed = json["data"]["executed"].as_bool().unwrap_or(false);
                
                Ok(exists && !cancelled && !executed)
            }
            _ => {
                warn!("Could not query approval status");
                Ok(false)
            }
        }
    }
}
