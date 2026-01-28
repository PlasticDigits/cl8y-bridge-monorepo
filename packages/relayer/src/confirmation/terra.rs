#![allow(dead_code)]

use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use tracing::{debug, info};

use crate::db::Release;

/// Terra transaction confirmation checker
pub struct TerraConfirmation {
    db: PgPool,
    required_confirmations: u32,
}

impl TerraConfirmation {
    /// Create a new Terra confirmation checker
    pub fn new(db: PgPool, required_confirmations: u32) -> Result<Self> {
        Ok(Self {
            db,
            required_confirmations,
        })
    }

    /// Check if a release transaction is confirmed
    pub async fn check_release_confirmation(&self, release: &Release) -> Result<bool> {
        let tx_hash = release.tx_hash.as_ref()
            .ok_or_else(|| eyre!("Release has no tx_hash"))?;

        debug!(
            tx_hash = %tx_hash,
            "Checking Terra release confirmation"
        );

        // Get the current block height from our tracking table
        // We use a fixed chain ID for Terra Classic
        let current_height = self.get_current_block_height("localterra").await?;

        if let Some(current_height) = current_height {
            // For now, consider confirmed after any successful submission
            // In a production system, we'd query the LCD for the tx
            // and compare block heights
            info!(
                tx_hash = %tx_hash,
                current_height = current_height,
                required_confirmations = self.required_confirmations,
                "Release considered confirmed (simplified check)"
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Get the current block height for Terra
    async fn get_current_block_height(&self, chain_id: &str) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT last_processed_height 
            FROM terra_blocks 
            WHERE chain_id = $1
            "#,
        )
        .bind(chain_id)
        .fetch_optional(&self.db)
        .await
        .wrap_err("Failed to get current block height")?;

        Ok(row.map(|r| r.0))
    }
}