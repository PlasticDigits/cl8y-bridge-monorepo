#![allow(dead_code)]

use base64::Engine;
use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::tx::{Body, Fee, Msg, SignDoc, SignerInfo};
use cosmrs::Coin;
use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, error, warn};

use crate::config::TerraConfig;
use crate::contracts::terra_bridge::{ExecuteMsg, build_release_msg};
use crate::db::{self, EvmDeposit, NewRelease};
use crate::types::ChainKey;

/// Terra Classic transaction writer for submitting release transactions
pub struct TerraWriter {
    rpc_url: String,
    lcd_url: String,
    bridge_address: String,
    chain_id: String,
    mnemonic: String,
    http_client: Client,
    db: PgPool,
}

/// Account info response from LCD
#[derive(Debug, Deserialize)]
struct AccountResponse {
    account: AccountInfo,
}

#[derive(Debug, Deserialize)]
struct AccountInfo {
    account_number: String,
    sequence: String,
}

/// Broadcast response from LCD
#[derive(Debug, Deserialize)]
struct BroadcastResponse {
    tx_response: TxResponse,
}

#[derive(Debug, Deserialize)]
struct TxResponse {
    txhash: String,
    code: Option<u32>,
    raw_log: Option<String>,
}

impl TerraWriter {
    /// Create a new Terra writer
    pub async fn new(config: &TerraConfig, db: PgPool) -> Result<Self> {
        let http_client = Client::new();

        Ok(Self {
            rpc_url: config.rpc_url.clone(),
            lcd_url: config.lcd_url.clone(),
            bridge_address: config.bridge_address.clone(),
            chain_id: config.chain_id.clone(),
            mnemonic: config.mnemonic.clone(),
            http_client,
            db,
        })
    }

    /// Decode a Terra address from bytes32 encoding
    fn decode_terra_address(&self, bytes32: &[u8]) -> Result<String> {
        let trimmed: Vec<u8> = bytes32
            .iter()
            .skip_while(|&&b| b == 0)
            .copied()
            .collect();

        String::from_utf8(trimmed)
            .wrap_err("Invalid Terra address encoding")
    }

    /// Decode a token denom from bytes32 encoding
    fn decode_token(&self, bytes32: &[u8]) -> Result<String> {
        let trimmed: Vec<u8> = bytes32
            .iter()
            .skip_while(|&&b| b == 0)
            .copied()
            .collect();

        String::from_utf8(trimmed)
            .wrap_err("Invalid token encoding")
    }

    /// Get the sender address from the mnemonic
    fn get_sender_address(&self) -> Result<String> {
        let signing_key = self.derive_signing_key()?;
        let public_key = signing_key.public_key();
        let account_id = public_key.account_id("terra")
            .wrap_err("Failed to derive account ID")?;
        Ok(account_id.to_string())
    }

    /// Derive signing key from mnemonic
    fn derive_signing_key(&self) -> Result<SigningKey> {
        use bip39::Mnemonic;
        use cosmrs::bip32::DerivationPath;
        
        let mnemonic: Mnemonic = self.mnemonic.parse()
            .map_err(|e| eyre!("Invalid mnemonic: {:?}", e))?;
        
        let seed = mnemonic.to_seed("");
        
        // Terra Classic HD path: m/44'/330'/0'/0/0
        let path: DerivationPath = "m/44'/330'/0'/0/0".parse()
            .map_err(|e| eyre!("Invalid derivation path: {:?}", e))?;
        
        let child_key = cosmrs::bip32::XPrv::derive_from_path(seed, &path)
            .map_err(|e| eyre!("Failed to derive key from path: {:?}", e))?;
        
        let signing_key = SigningKey::from_slice(&child_key.private_key().to_bytes())
            .wrap_err("Failed to create signing key")?;
        
        Ok(signing_key)
    }

    /// Process all pending EVM deposits
    pub async fn process_pending(&self) -> Result<()> {
        let deposits = db::get_pending_evm_deposits(&self.db).await?;
        
        if !deposits.is_empty() {
            info!(count = deposits.len(), "Processing pending EVM deposits");
        }

        for deposit in deposits {
            if let Err(e) = self.process_deposit(&deposit).await {
                error!(error = %e, nonce = deposit.nonce, tx_hash = %deposit.tx_hash, "Failed to process deposit");
            }
        }

        Ok(())
    }

    /// Process a single EVM deposit - create and submit release
    async fn process_deposit(&self, deposit: &EvmDeposit) -> Result<()> {
        info!(
            nonce = deposit.nonce,
            tx_hash = %deposit.tx_hash,
            amount = %deposit.amount,
            "Processing EVM deposit for Terra release"
        );

        // Build the release record
        let release = self.build_release(deposit)?;

        // Check if release already exists
        let exists = db::release_exists(&self.db, &release.src_chain_key, release.nonce).await?;

        if exists {
            warn!(nonce = deposit.nonce, "Release already exists, skipping");
            return Ok(());
        }

        // Insert the release record
        let release_id = db::insert_release(&self.db, &release).await?;
        info!(release_id = release_id, "Created release record");

        // Build the execute message
        let msg = self.build_release_msg(&release)?;

        // Submit the transaction
        match self.broadcast_tx(msg).await {
            Ok(tx_hash) => {
                info!(tx_hash = %tx_hash, nonce = deposit.nonce, "Release submitted successfully");
                db::update_release_submitted(&self.db, release_id, &tx_hash).await?;
                db::update_evm_deposit_status(&self.db, deposit.id, "submitted").await?;
            }
            Err(e) => {
                error!(error = %e, nonce = deposit.nonce, "Failed to submit release");
                db::update_release_failed(&self.db, release_id, &e.to_string()).await?;
                return Err(e);
            }
        }

        Ok(())
    }

    /// Build a release record from an EVM deposit
    fn build_release(&self, deposit: &EvmDeposit) -> Result<NewRelease> {
        // Source chain key for EVM
        let src_chain_key = ChainKey::evm(deposit.chain_id as u64);

        // Decode recipient from bytes32
        let recipient = self.decode_terra_address(&deposit.dest_account)?;

        // Decode token from bytes32
        let token = self.decode_token(&deposit.dest_token_address)?;

        // Sender is the EVM token address (for tracking)
        let sender = deposit.token.clone();

        Ok(NewRelease {
            src_chain_key: src_chain_key.0.to_vec(),
            nonce: deposit.nonce,
            sender,
            recipient,
            token,
            amount: deposit.amount.clone(),
            source_chain_id: deposit.chain_id,
        })
    }

    /// Build the CosmWasm execute message for release
    fn build_release_msg(&self, release: &NewRelease) -> Result<ExecuteMsg> {
        // For single-relayer mode, we don't need signatures
        // The relayer is trusted and registered in the contract
        Ok(build_release_msg(
            release.nonce as u64,
            &release.sender,
            &release.recipient,
            &release.token,
            &release.amount,
            release.source_chain_id as u64,
            vec![], // Empty signatures for single-relayer mode
        ))
    }

    /// Get account info from LCD
    async fn get_account_info(&self, address: &str) -> Result<(u64, u64)> {
        let url = format!("{}/cosmos/auth/v1beta1/accounts/{}", self.lcd_url, address);
        
        let response = self.http_client.get(&url)
            .send()
            .await
            .wrap_err("Failed to query account info")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(eyre!("Account query failed with {}: {}", status, body));
        }

        let account_response: AccountResponse = response.json().await
            .wrap_err("Failed to parse account response")?;

        let account_number: u64 = account_response.account.account_number.parse()
            .wrap_err("Invalid account number")?;
        let sequence: u64 = account_response.account.sequence.parse()
            .wrap_err("Invalid sequence")?;

        Ok((account_number, sequence))
    }

    /// Broadcast a transaction via LCD
    async fn broadcast_tx(&self, msg: ExecuteMsg) -> Result<String> {
        let signing_key = self.derive_signing_key()?;
        let public_key = signing_key.public_key();
        let sender = public_key.account_id("terra")
            .wrap_err("Failed to derive account ID")?;
        
        info!(sender = %sender, "Broadcasting release transaction");

        // Get account info
        let (account_number, sequence) = self.get_account_info(sender.as_ref()).await?;
        info!(account_number = account_number, sequence = sequence, "Got account info");

        // Build MsgExecuteContract
        let msg_json = serde_json::to_vec(&msg)
            .wrap_err("Failed to serialize message")?;

        let execute_msg = cosmrs::cosmwasm::MsgExecuteContract {
            sender: sender.clone(),
            contract: self.bridge_address.parse()
                .wrap_err("Invalid bridge address")?,
            msg: msg_json,
            funds: vec![], // No funds attached for release
        };

        // Convert to Any using to_any method
        let any_msg = execute_msg.to_any()
            .wrap_err("Failed to encode message as Any")?;

        // Build transaction body
        let body = Body::new(
            vec![any_msg],
            "CL8Y Bridge Release",
            0u32, // timeout_height (0 = no timeout)
        );

        // Build fee (fixed for local testing)
        let fee = Fee::from_amount_and_gas(
            Coin::new(50000u128, "uluna").wrap_err("Invalid coin")?,
            200000u64,
        );

        // Build auth info
        let auth_info = SignerInfo::single_direct(Some(public_key), sequence)
            .auth_info(fee);

        // Create sign doc
        let sign_doc = SignDoc::new(
            &body,
            &auth_info,
            &self.chain_id.parse().wrap_err("Invalid chain ID")?,
            account_number,
        ).wrap_err("Failed to create sign doc")?;

        // Sign the transaction
        let tx_raw = sign_doc.sign(&signing_key)
            .wrap_err("Failed to sign transaction")?;

        // Encode to bytes
        let tx_bytes = tx_raw.to_bytes()
            .wrap_err("Failed to encode transaction")?;

        // Broadcast via LCD
        let broadcast_url = format!("{}/cosmos/tx/v1beta1/txs", self.lcd_url);
        let body = json!({
            "tx_bytes": base64::engine::general_purpose::STANDARD.encode(&tx_bytes),
            "mode": "BROADCAST_MODE_SYNC"
        });

        let response = self.http_client.post(&broadcast_url)
            .json(&body)
            .send()
            .await
            .wrap_err("Failed to broadcast transaction")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(eyre!("Broadcast failed with {}: {}", status, body));
        }

        let broadcast_response: BroadcastResponse = response.json().await
            .wrap_err("Failed to parse broadcast response")?;

        // Check for error code
        if let Some(code) = broadcast_response.tx_response.code {
            if code != 0 {
                let raw_log = broadcast_response.tx_response.raw_log.unwrap_or_default();
                return Err(eyre!("Transaction failed with code {}: {}", code, raw_log));
            }
        }

        Ok(broadcast_response.tx_response.txhash)
    }
}
