use eyre::Result;

pub mod evm;
pub mod terra;


/// Configuration for the confirmation tracker
#[derive(Debug, Clone)]
pub struct ConfirmationConfig {
    /// How often to check for submitted transactions (in milliseconds)
    pub poll_interval_ms: u64,
    /// Number of EVM block confirmations required
    pub evm_confirmations: u32,
    /// Number of Terra block confirmations required
    pub terra_confirmations: u32,
}

impl Default for ConfirmationConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 10_000,
            evm_confirmations: 12,
            terra_confirmations: 6,
        }
    }
}

/// Tracks and confirms submitted transactions on multiple chains
pub struct ConfirmationTracker {
    config: ConfirmationConfig,
    db: sqlx::PgPool,
    evm_checker: evm::EvmConfirmation,
    terra_checker: terra::TerraConfirmation,
}

impl ConfirmationTracker {
    /// Create a new confirmation tracker from relayer config
    pub async fn new(relayer_config: &crate::config::Config, db: sqlx::PgPool) -> Result<Self> {
        let config = ConfirmationConfig {
            poll_interval_ms: relayer_config.relayer.poll_interval_ms,
            evm_confirmations: relayer_config.evm.finality_blocks as u32,
            terra_confirmations: 6,
        };
        let evm_checker = evm::EvmConfirmation::new(
            db.clone(),
            config.evm_confirmations,
            relayer_config.evm.rpc_url.clone(),
        )?;
        let terra_checker = terra::TerraConfirmation::new(
            db.clone(),
            config.terra_confirmations,
            relayer_config.terra.lcd_url.clone(),
        )?;

        Ok(Self {
            config,
            db,
            evm_checker,
            terra_checker,
        })
    }

    /// Run the confirmation loop
    pub async fn run(&mut self, mut shutdown: tokio::sync::mpsc::Receiver<()>) -> Result<()> {
        tracing::info!(
            poll_interval_ms = self.config.poll_interval_ms,
            evm_confirmations = self.config.evm_confirmations,
            terra_confirmations = self.config.terra_confirmations,
            "Starting confirmation tracker"
        );

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    tracing::info!("Shutdown signal received, stopping confirmation tracker");
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(self.config.poll_interval_ms)) => {
                    if let Err(err) = self.process_pending().await {
                        tracing::error!(error = %err, "Error processing pending transactions");
                    }
                }
            }
        }

        Ok(())
    }

    /// Process all submitted transactions and check their confirmations
    pub async fn process_pending(&mut self) -> Result<()> {
        // Process approvals
        if let Err(err) = self.process_approvals().await {
            tracing::error!(error = %err, "Error processing approvals");
        }

        // Process releases
        if let Err(err) = self.process_releases().await {
            tracing::error!(error = %err, "Error processing releases");
        }

        Ok(())
    }

    /// Process all submitted approvals
    async fn process_approvals(&mut self) -> Result<()> {
        let approvals = crate::db::get_submitted_approvals(&self.db).await?;

        for approval in approvals {
            let id = approval.id;
            let tx_hash = approval.tx_hash.clone().unwrap_or_default();

            tracing::debug!(
                approval_id = id,
                tx_hash = %tx_hash,
                "Checking approval confirmation"
            );

            match self.evm_checker.check_approval_confirmation(&approval).await {
                Ok(result) => {
                    match result {
                        evm::ConfirmationResult::Confirmed => {
                            crate::db::update_approval_confirmed(&self.db, id).await?;
                            tracing::info!(approval_id = id, tx_hash = %tx_hash, "Approval confirmed");
                        }
                        evm::ConfirmationResult::Failed => {
                            crate::db::update_approval_failed(&self.db, id, "Transaction failed on-chain").await?;
                            tracing::warn!(approval_id = id, tx_hash = %tx_hash, "Approval failed");
                        }
                        _ => {
                            // Still pending or waiting for confirmations
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        approval_id = id,
                        tx_hash = %tx_hash,
                        error = %err,
                        "Error checking approval confirmation"
                    );
                }
            }
        }

        Ok(())
    }

    /// Process all submitted releases
    async fn process_releases(&mut self) -> Result<()> {
        let releases = crate::db::get_submitted_releases(&self.db).await?;

        for release in releases {
            let id = release.id;
            let tx_hash = release.tx_hash.clone().unwrap_or_default();

            tracing::debug!(
                release_id = id,
                tx_hash = %tx_hash,
                "Checking release confirmation"
            );

            match self.terra_checker.check_release_confirmation(&release).await {
                Ok(result) => {
                    match result {
                        terra::ConfirmationResult::Confirmed => {
                            crate::db::update_release_confirmed(&self.db, id).await?;
                            tracing::info!(release_id = id, tx_hash = %tx_hash, "Release confirmed");
                        }
                        terra::ConfirmationResult::Failed => {
                            crate::db::update_release_failed(&self.db, id, "Transaction failed on-chain").await?;
                            tracing::warn!(release_id = id, tx_hash = %tx_hash, "Release failed");
                        }
                        _ => {
                            // Still pending or waiting for confirmations
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        release_id = id,
                        tx_hash = %tx_hash,
                        error = %err,
                        "Error checking release confirmation"
                    );
                }
            }
        }

        Ok(())
    }
}