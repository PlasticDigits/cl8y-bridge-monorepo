//! Approval verification logic
//!
//! Verifies that withdraw approvals on the destination chain
//! correspond to valid deposits on the source chain.

use eyre::{eyre, Result};
use reqwest::Client;
use tracing::{debug, info, warn};

use crate::hash::{bytes32_to_hex, compute_transfer_id};

/// Pending approval to verify
#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub withdraw_hash: [u8; 32],
    pub src_chain_key: [u8; 32],
    pub dest_chain_key: [u8; 32],
    pub dest_token_address: [u8; 32],
    pub dest_account: [u8; 32],
    pub amount: u128,
    pub nonce: u64,
    pub approved_at_timestamp: u64,
    pub delay_seconds: u64,
}

/// Verification result
#[derive(Debug, Clone)]
pub enum VerificationResult {
    /// Approval is valid - matches source deposit
    Valid,
    /// Approval is invalid - no matching deposit found
    Invalid { reason: String },
    /// Cannot verify yet - need more confirmations
    Pending,
}

/// Verifier for checking approvals against source chain
pub struct ApprovalVerifier {
    client: Client,
    evm_rpc_url: String,
    terra_lcd_url: String,
    terra_bridge_address: String,
}

impl ApprovalVerifier {
    pub fn new(evm_rpc_url: &str, terra_lcd_url: &str, terra_bridge_address: &str) -> Self {
        Self {
            client: Client::new(),
            evm_rpc_url: evm_rpc_url.to_string(),
            terra_lcd_url: terra_lcd_url.to_string(),
            terra_bridge_address: terra_bridge_address.to_string(),
        }
    }

    /// Verify an approval against the source chain
    pub async fn verify(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        // First, verify the hash is correctly computed
        let computed_hash = compute_transfer_id(
            &approval.src_chain_key,
            &approval.dest_chain_key,
            &approval.dest_token_address,
            &approval.dest_account,
            approval.amount,
            approval.nonce,
        );

        if computed_hash != approval.withdraw_hash {
            warn!(
                expected = %bytes32_to_hex(&computed_hash),
                got = %bytes32_to_hex(&approval.withdraw_hash),
                "Hash mismatch in approval"
            );
            return Ok(VerificationResult::Invalid {
                reason: "Hash does not match parameters".to_string(),
            });
        }

        // Determine source chain type from chain key
        // For MVP, we'll check if it's EVM or Terra based on key prefix
        // In full implementation, we'd have a chain registry

        // Check if source is EVM (deposit on EVM, withdraw to Terra)
        if self.is_evm_chain_key(&approval.src_chain_key) {
            return self.verify_evm_deposit(approval).await;
        }

        // Check if source is Terra (deposit on Terra, withdraw to EVM)
        if self.is_terra_chain_key(&approval.src_chain_key) {
            return self.verify_terra_deposit(approval).await;
        }

        Ok(VerificationResult::Pending)
    }

    /// Verify a deposit exists on EVM source chain
    async fn verify_evm_deposit(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        // TODO: Query EVM chain for deposit event matching this approval
        // For MVP, just log and return pending
        debug!(
            hash = %bytes32_to_hex(&approval.withdraw_hash),
            nonce = approval.nonce,
            "Verifying EVM deposit (MVP: not implemented)"
        );

        // MVP: Always return Valid (assume operator is honest)
        // Full implementation would query deposit events/storage
        Ok(VerificationResult::Valid)
    }

    /// Verify a deposit exists on Terra source chain
    async fn verify_terra_deposit(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        // Query Terra contract for deposit hash
        let query = serde_json::json!({
            "verify_deposit": {
                "deposit_hash": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &approval.withdraw_hash
                ),
                "dest_chain_key": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &approval.dest_chain_key
                ),
                "dest_token_address": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &approval.dest_token_address
                ),
                "dest_account": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &approval.dest_account
                ),
                "amount": approval.amount.to_string(),
                "nonce": approval.nonce
            }
        });

        let query_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            serde_json::to_string(&query)?,
        );

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            self.terra_lcd_url, self.terra_bridge_address, query_b64
        );

        match self.client.get(&url).send().await {
            Ok(resp) => {
                let json: serde_json::Value = resp.json().await?;

                let exists = json["data"]["exists"].as_bool().unwrap_or(false);
                let matches = json["data"]["matches"].as_bool().unwrap_or(false);

                if exists && matches {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "Deposit verified on Terra"
                    );
                    Ok(VerificationResult::Valid)
                } else if exists {
                    Ok(VerificationResult::Invalid {
                        reason: "Deposit exists but parameters don't match".to_string(),
                    })
                } else {
                    Ok(VerificationResult::Invalid {
                        reason: "No deposit found with this hash".to_string(),
                    })
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to query Terra deposit");
                Ok(VerificationResult::Pending)
            }
        }
    }

    /// Check if chain key is an EVM chain
    fn is_evm_chain_key(&self, _key: &[u8; 32]) -> bool {
        // MVP: simplified check - in full impl would have chain registry
        true
    }

    /// Check if chain key is Terra
    fn is_terra_chain_key(&self, _key: &[u8; 32]) -> bool {
        false
    }
}
