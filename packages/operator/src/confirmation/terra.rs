#![allow(dead_code)]

use eyre::{eyre, Result};
use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::warn;

use crate::db::Release;

/// Result of checking a transaction receipt
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmationResult {
    /// Transaction is pending (not yet in a block)
    Pending,
    /// Transaction confirmed with enough blocks
    Confirmed,
    /// Waiting for more confirmations
    WaitingConfirmations(u32),
    /// Transaction failed on-chain (non-zero code)
    Failed,
    /// Transaction was reorged (no longer in chain)
    Reorged,
}

/// Terra transaction response from LCD
#[derive(Debug, Deserialize)]
struct TxResponse {
    tx_response: Option<TxResponseInner>,
}

#[derive(Debug, Deserialize)]
struct TxResponseInner {
    txhash: String,
    height: String,
    code: i32,
    codespace: Option<String>,
    raw_log: Option<String>,
}

/// Terra block response from LCD
#[derive(Debug, Deserialize)]
struct BlockResponse {
    block: BlockInfo,
}

#[derive(Debug, Deserialize)]
struct BlockInfo {
    header: BlockHeader,
}

#[derive(Debug, Deserialize)]
struct BlockHeader {
    height: String,
}

/// Terra confirmation checker
pub struct TerraConfirmation {
    db: PgPool,
    required_confirmations: u32,
    lcd_url: String,
    client: Client,
}

impl TerraConfirmation {
    /// Create a new Terra confirmation checker
    pub fn new(db: PgPool, required_confirmations: u32, lcd_url: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            db,
            required_confirmations,
            lcd_url,
            client,
        })
    }

    /// Check release transaction confirmation
    pub async fn check_release_confirmation(
        &self,
        release: &Release,
    ) -> Result<ConfirmationResult> {
        let tx_hash = release
            .tx_hash
            .as_ref()
            .ok_or_else(|| eyre!("Release has no tx_hash"))?;

        // 1. Query transaction from LCD
        let tx = self.get_transaction(tx_hash).await?;

        // 2. If not found, might still be pending or failed
        if tx.is_none() {
            return Ok(ConfirmationResult::Pending);
        }

        let tx = tx.unwrap();

        // 3. Check if transaction failed (non-zero code)
        if tx.code != 0 {
            warn!(
                tx_hash = %tx_hash,
                code = tx.code,
                codespace = ?tx.codespace,
                "Terra transaction failed"
            );
            return Ok(ConfirmationResult::Failed);
        }

        // 4. Get current block height
        let current_height = self.get_current_block_height().await?;

        // 5. Calculate confirmations
        let tx_height: u64 = tx.height.parse()?;
        let confirmations = current_height.saturating_sub(tx_height);

        // 6. Check if enough confirmations
        if confirmations >= self.required_confirmations as u64 {
            return Ok(ConfirmationResult::Confirmed);
        }

        Ok(ConfirmationResult::WaitingConfirmations(
            confirmations as u32,
        ))
    }

    /// Get transaction from LCD
    async fn get_transaction(&self, tx_hash: &str) -> Result<Option<TxResponseInner>> {
        let url = format!("{}/cosmos/tx/v1beta1/txs/{}", self.lcd_url, tx_hash);

        let response = self.client.get(&url).send().await;

        match response {
            Ok(resp) => {
                if resp.status() == 404 {
                    return Ok(None);
                }
                let tx_response: TxResponse = resp.json().await?;
                Ok(tx_response.tx_response)
            }
            Err(e) => {
                warn!(error = %e, "Failed to query Terra transaction");
                Err(e.into())
            }
        }
    }

    /// Get current block height from LCD
    async fn get_current_block_height(&self) -> Result<u64> {
        let url = format!(
            "{}/cosmos/base/tendermint/v1beta1/blocks/latest",
            self.lcd_url
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<BlockResponse>()
            .await?;

        let height: u64 = response.block.header.height.parse()?;

        Ok(height)
    }
}
