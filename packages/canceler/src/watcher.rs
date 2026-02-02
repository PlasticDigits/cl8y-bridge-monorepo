//! Watcher for monitoring approvals across chains

use std::collections::HashSet;
use std::time::Duration;

use eyre::Result;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::hash::bytes32_to_hex;
use crate::verifier::{ApprovalVerifier, PendingApproval, VerificationResult};

/// Main watcher that monitors all chains for approvals
pub struct CancelerWatcher {
    config: Config,
    verifier: ApprovalVerifier,
    /// Hashes we've already verified
    verified_hashes: HashSet<[u8; 32]>,
}

impl CancelerWatcher {
    pub async fn new(config: &Config) -> Result<Self> {
        let verifier = ApprovalVerifier::new(
            &config.evm_rpc_url,
            &config.terra_lcd_url,
            &config.terra_bridge_address,
        );

        Ok(Self {
            config: config.clone(),
            verifier,
            verified_hashes: HashSet::new(),
        })
    }

    /// Main run loop
    pub async fn run(&mut self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        info!("Canceler watcher starting...");

        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
                _ = tokio::time::sleep(poll_interval) => {
                    if let Err(e) = self.poll_approvals().await {
                        error!(error = %e, "Error polling approvals");
                    }
                }
            }
        }

        Ok(())
    }

    /// Poll for new approvals on all chains
    async fn poll_approvals(&mut self) -> Result<()> {
        debug!("Polling for new approvals...");

        // Poll EVM bridge for approvals
        self.poll_evm_approvals().await?;

        // Poll Terra bridge for approvals
        self.poll_terra_approvals().await?;

        Ok(())
    }

    /// Poll EVM bridge for pending approvals
    async fn poll_evm_approvals(&mut self) -> Result<()> {
        // TODO: Query EVM bridge for ApproveWithdraw events
        // For MVP, this is a placeholder
        debug!("Polling EVM approvals (MVP: stub)");
        Ok(())
    }

    /// Poll Terra bridge for pending approvals
    async fn poll_terra_approvals(&mut self) -> Result<()> {
        // TODO: Query Terra bridge for pending approvals
        // For MVP, this is a placeholder
        debug!("Polling Terra approvals (MVP: stub)");
        Ok(())
    }

    /// Verify an approval and potentially cancel it
    async fn verify_and_cancel(&mut self, approval: &PendingApproval) -> Result<()> {
        // Skip if already verified
        if self.verified_hashes.contains(&approval.withdraw_hash) {
            return Ok(());
        }

        let result = self.verifier.verify(approval).await?;

        match result {
            VerificationResult::Valid => {
                info!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    "Approval verified as VALID"
                );
                self.verified_hashes.insert(approval.withdraw_hash);
            }
            VerificationResult::Invalid { reason } => {
                warn!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    reason = %reason,
                    "Approval is INVALID - should cancel"
                );
                // TODO: Submit cancel transaction
                self.submit_cancel(&approval.withdraw_hash).await?;
                self.verified_hashes.insert(approval.withdraw_hash);
            }
            VerificationResult::Pending => {
                debug!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    "Verification pending - will retry"
                );
            }
        }

        Ok(())
    }

    /// Submit cancel transaction (MVP: just logs)
    async fn submit_cancel(&self, withdraw_hash: &[u8; 32]) -> Result<()> {
        // TODO: Sign and submit CancelWithdrawApproval transaction
        warn!(
            hash = %bytes32_to_hex(withdraw_hash),
            "CANCEL REQUIRED - MVP: not submitting transaction"
        );
        Ok(())
    }
}
