#![allow(dead_code)]

use eyre::{eyre, Result, WrapErr};
use sqlx::PgPool;
use tracing::{debug, info};

use crate::db::Approval;

/// EVM transaction confirmation checker
pub struct EvmConfirmation {
    db: PgPool,
    required_confirmations: u32,
}

impl EvmConfirmation {
    /// Create a new EVM confirmation checker
    pub fn new(db: PgPool, required_confirmations: u32) -> Result<Self> {
        Ok(Self {
            db,
            required_confirmations,
        })
    }

    /// Check if an approval transaction is confirmed
    pub async fn check_approval_confirmation(&self, approval: &Approval) -> Result<bool> {
        let tx_hash = approval.tx_hash.as_ref()
            .ok_or_else(|| eyre!("Approval has no tx_hash"))?;
        let chain_id = approval.dest_chain_id;

        debug!(
            tx_hash = %tx_hash,
            chain_id = chain_id,
            "Checking EVM approval confirmation"
        );

        // Get the current block number from our tracking table
        let current_block = self.get_current_block_number(chain_id).await?;

        if let Some(current_block) = current_block {
            // For now, consider confirmed after any successful submission
            // In a production system, we'd query the RPC for the tx receipt
            // and compare block numbers
            info!(
                tx_hash = %tx_hash,
                current_block = current_block,
                required_confirmations = self.required_confirmations,
                "Approval considered confirmed (simplified check)"
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Get the current block number for a chain
    async fn get_current_block_number(&self, chain_id: i64) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT last_processed_block 
            FROM evm_blocks 
            WHERE chain_id = $1
            "#,
        )
        .bind(chain_id)
        .fetch_optional(&self.db)
        .await
        .wrap_err("Failed to get current block number")?;

        Ok(row.map(|r| r.0))
    }
}