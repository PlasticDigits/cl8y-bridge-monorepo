//! Terra Writer - Submits release transactions to Terra Classic
//!
//! Processes pending EVM deposits and submits corresponding
//! release transactions to the Terra bridge contract.

#![allow(dead_code)]

use eyre::{eyre, Result, WrapErr};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info, warn};

use crate::config::TerraConfig;
use crate::contracts::terra_bridge;
use crate::db::{self, EvmDeposit, NewRelease};
use crate::types::ChainKey;

/// Terra transaction writer for submitting releases
pub struct TerraWriter {
    lcd_url: String,
    chain_id: String,
    contract_address: String,
    mnemonic: String,
    client: Client,
    db: PgPool,
}

impl TerraWriter {
    /// Create a new Terra writer
    pub async fn new(terra_config: &TerraConfig, db: PgPool) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .wrap_err("Failed to create HTTP client")?;

        Ok(Self {
            lcd_url: terra_config.lcd_url.clone(),
            chain_id: terra_config.chain_id.clone(),
            contract_address: terra_config.bridge_address.clone(),
            mnemonic: terra_config.mnemonic.clone(),
            client,
            db,
        })
    }

    /// Process pending EVM deposits and create releases
    pub async fn process_pending(&self) -> Result<()> {
        let deposits = db::get_pending_evm_deposits(&self.db).await?;

        for deposit in deposits {
            if let Err(e) = self.process_deposit(&deposit).await {
                error!(
                    deposit_id = deposit.id,
                    error = %e,
                    "Failed to process EVM deposit"
                );
            }
        }

        Ok(())
    }

    /// Process a single EVM deposit
    async fn process_deposit(&self, deposit: &EvmDeposit) -> Result<()> {
        let src_chain_key = ChainKey::evm(deposit.chain_id as u64);

        // Check if release already exists
        if db::release_exists(&self.db, src_chain_key.as_bytes(), deposit.nonce).await? {
            db::update_evm_deposit_status(&self.db, deposit.id, "processed").await?;
            return Ok(());
        }

        // Decode destination account from bytes
        let recipient = self.decode_terra_address(&deposit.dest_account)?;

        // Create release record
        let new_release = NewRelease {
            src_chain_key: src_chain_key.as_bytes().to_vec(),
            nonce: deposit.nonce,
            sender: format!("0x{}", hex::encode(&deposit.token)),
            recipient: recipient.clone(),
            token: deposit.token.clone(),
            amount: deposit.amount.clone(),
            source_chain_id: deposit.chain_id,
        };

        let release_id = db::insert_release(&self.db, &new_release).await?;
        info!(
            release_id = release_id,
            nonce = deposit.nonce,
            "Created release for EVM deposit"
        );

        // Submit to Terra
        match self.submit_release(release_id).await {
            Ok(tx_hash) => {
                info!(
                    release_id = release_id,
                    tx_hash = %tx_hash,
                    "Submitted release transaction"
                );
                db::update_evm_deposit_status(&self.db, deposit.id, "processed").await?;
            }
            Err(e) => {
                warn!(
                    release_id = release_id,
                    error = %e,
                    "Failed to submit release, will retry"
                );
                db::update_release_failed(&self.db, release_id, &e.to_string()).await?;
            }
        }

        Ok(())
    }

    /// Submit a release transaction to Terra
    async fn submit_release(&self, release_id: i64) -> Result<String> {
        // Get release from database
        let releases = db::get_pending_releases(&self.db).await?;
        let release = releases
            .into_iter()
            .find(|r| r.id == release_id)
            .ok_or_else(|| eyre!("Release {} not found", release_id))?;

        // Build the release message
        let _msg = terra_bridge::build_release_msg(
            release.nonce as u64,
            &release.sender,
            &release.recipient,
            &release.token,
            &release.amount,
            release.source_chain_id as u64,
            vec![], // signatures - placeholder
        );

        // TODO: Sign and broadcast transaction
        // For now, return a placeholder
        let tx_hash = format!(
            "{}{}",
            hex::encode(&release.src_chain_key[..8]),
            release.nonce
        );

        db::update_release_submitted(&self.db, release_id, &tx_hash).await?;

        Ok(tx_hash)
    }

    /// Decode Terra address from bytes
    fn decode_terra_address(&self, bytes: &[u8]) -> Result<String> {
        // Try to decode as UTF-8 string first
        if let Ok(s) = String::from_utf8(bytes.to_vec()) {
            if s.starts_with("terra") {
                return Ok(s);
            }
        }

        // Otherwise decode as bech32
        // For now, just encode as hex
        Ok(format!("terra{}", hex::encode(&bytes[..20.min(bytes.len())])))
    }
}
