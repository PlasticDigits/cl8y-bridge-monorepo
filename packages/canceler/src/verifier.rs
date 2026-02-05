//! Approval verification logic (V2)
//!
//! Verifies that withdraw approvals on the destination chain
//! correspond to valid deposits on the source chain.
//!
//! # V2 Verification Flow
//!
//! 1. Canceler observes WithdrawApprove event on destination chain
//! 2. Verifier queries source chain for matching deposit:
//!    - For EVM source: calls `deposits(depositHash)` on EVM bridge
//!    - For Terra source: calls `VerifyDeposit` query on Terra bridge
//! 3. If deposit exists and parameters match → Valid
//! 4. If deposit missing or parameters mismatch → Invalid → Submit cancellation

use std::time::Duration;

use alloy::primitives::{Address, FixedBytes};
use alloy::providers::ProviderBuilder;
use alloy::sol;
use base64::Engine;
use eyre::Result;
use reqwest::Client;
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::hash::{bytes32_to_hex, compute_withdraw_hash};

// EVM bridge contract interface for deposit verification (V2)
sol! {
    #[sol(rpc)]
    contract Bridge {
        /// Get deposit info by hash
        function deposits(bytes32 depositHash) external view returns (
            bytes4 srcChain,
            bytes4 destChain,
            bytes32 token,
            bytes32 srcAccount,
            uint128 amount,
            uint64 nonce,
            bool exists
        );
    }
}

/// Pending approval to verify (V2 - uses 4-byte chain IDs)
#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub withdraw_hash: [u8; 32],
    /// Source chain ID (4 bytes)
    pub src_chain_id: [u8; 4],
    /// Destination chain ID (4 bytes)
    pub dest_chain_id: [u8; 4],
    pub dest_token: [u8; 32],
    pub dest_account: [u8; 32],
    pub amount: u128,
    pub nonce: u64,
    /// Timestamp when approval was created (for delay tracking)
    #[allow(dead_code)]
    pub approved_at_timestamp: u64,
    /// Cancel window seconds (for time-based decisions)
    #[allow(dead_code)]
    pub cancel_window: u64,
}

/// Verification result
#[derive(Debug, Clone)]
pub enum VerificationResult {
    /// Approval is valid - matches source deposit
    Valid,
    /// Approval is invalid - no matching deposit found
    Invalid { reason: String },
    /// Cannot verify yet - need more confirmations or source chain unreachable
    Pending,
}

/// Known EVM chain IDs and their RPC URLs (for multi-chain support)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EvmChainConfig {
    pub chain_id: u64,
    /// 4-byte chain ID (V2)
    pub this_chain_id: [u8; 4],
    pub rpc_url: String,
    pub bridge_address: String,
}

/// Verifier for checking approvals against source chain (V2)
pub struct ApprovalVerifier {
    client: Client,
    /// Primary EVM RPC URL (for the main monitored chain)
    evm_rpc_url: String,
    /// Primary EVM bridge address
    evm_bridge_address: String,
    /// Terra LCD URL
    terra_lcd_url: String,
    /// Terra bridge contract address
    terra_bridge_address: String,
    /// This EVM chain's 4-byte chain ID
    evm_chain_id: [u8; 4],
    /// Terra's 4-byte chain ID
    terra_chain_id: [u8; 4],
}

impl ApprovalVerifier {
    pub fn new(
        evm_rpc_url: &str,
        evm_bridge_address: &str,
        evm_chain_id: u64,
        terra_lcd_url: &str,
        terra_bridge_address: &str,
        _terra_chain_id: &str, // Legacy param, we use numeric IDs now
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Convert native EVM chain ID to 4-byte format
        let evm_chain_id_bytes = (evm_chain_id as u32).to_be_bytes();
        
        // Terra chain ID (default: 5 for localterra, 4 for columbus-5)
        let terra_chain_id_bytes = if _terra_chain_id == "localterra" {
            5u32.to_be_bytes()
        } else {
            4u32.to_be_bytes()
        };

        Self {
            client,
            evm_rpc_url: evm_rpc_url.to_string(),
            evm_bridge_address: evm_bridge_address.to_string(),
            terra_lcd_url: terra_lcd_url.to_string(),
            terra_bridge_address: terra_bridge_address.to_string(),
            evm_chain_id: evm_chain_id_bytes,
            terra_chain_id: terra_chain_id_bytes,
        }
    }

    /// Verify an approval against the source chain
    pub async fn verify(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        // First, verify the hash is correctly computed from the approval parameters
        let computed_hash = compute_withdraw_hash(
            &approval.src_chain_id,
            &approval.dest_chain_id,
            &approval.dest_token,
            &approval.dest_account,
            approval.amount,
            approval.nonce,
        );

        if computed_hash != approval.withdraw_hash {
            warn!(
                expected = %bytes32_to_hex(&computed_hash),
                got = %bytes32_to_hex(&approval.withdraw_hash),
                "Hash mismatch in approval - parameters don't match claimed hash"
            );
            return Ok(VerificationResult::Invalid {
                reason: format!(
                    "Hash does not match parameters. Expected {}, got {}",
                    bytes32_to_hex(&computed_hash),
                    bytes32_to_hex(&approval.withdraw_hash)
                ),
            });
        }

        // Determine source chain type from chain ID
        if self.is_evm_chain(&approval.src_chain_id) {
            debug!(
                hash = %bytes32_to_hex(&approval.withdraw_hash),
                "Source is EVM chain, verifying deposit on EVM"
            );
            return self.verify_evm_deposit(approval).await;
        }

        if self.is_terra_chain(&approval.src_chain_id) {
            debug!(
                hash = %bytes32_to_hex(&approval.withdraw_hash),
                "Source is Terra chain, verifying deposit on Terra"
            );
            return self.verify_terra_deposit(approval).await;
        }

        // Unknown source chain - this is suspicious
        warn!(
            src_chain_id = %hex::encode(approval.src_chain_id),
            withdraw_hash = %bytes32_to_hex(&approval.withdraw_hash),
            "Unknown source chain ID - marking as pending for further investigation"
        );
        Ok(VerificationResult::Pending)
    }

    /// Verify a deposit exists on EVM source chain (V2)
    async fn verify_evm_deposit(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        debug!(
            hash = %bytes32_to_hex(&approval.withdraw_hash),
            nonce = approval.nonce,
            amount = approval.amount,
            "Querying EVM source chain for deposit"
        );

        // Parse bridge address
        let bridge_address = match Address::from_str(&self.evm_bridge_address) {
            Ok(addr) => addr,
            Err(e) => {
                warn!(error = %e, "Invalid EVM bridge address");
                return Ok(VerificationResult::Pending);
            }
        };

        // Create provider
        let provider = match self.evm_rpc_url.parse() {
            Ok(url) => ProviderBuilder::new().on_http(url),
            Err(e) => {
                warn!(error = %e, "Invalid EVM RPC URL");
                return Ok(VerificationResult::Pending);
            }
        };

        let contract = Bridge::new(bridge_address, &provider);

        // Query the deposit by hash
        let deposit_hash = FixedBytes::from(approval.withdraw_hash);

        match contract.deposits(deposit_hash).call().await {
            Ok(deposit) => {
                // Check if deposit exists
                if !deposit.exists {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "No deposit found on EVM source chain"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: "No deposit found with this hash on source chain".to_string(),
                    });
                }

                // Verify amount
                if deposit.amount != approval.amount {
                    info!(
                        expected = approval.amount,
                        got = deposit.amount,
                        "Amount mismatch"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: format!(
                            "Amount mismatch: expected {}, got {}",
                            approval.amount, deposit.amount
                        ),
                    });
                }

                // Verify nonce
                if deposit.nonce != approval.nonce {
                    info!(
                        expected = approval.nonce,
                        got = deposit.nonce,
                        "Nonce mismatch"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: format!(
                            "Nonce mismatch: expected {}, got {}",
                            approval.nonce, deposit.nonce
                        ),
                    });
                }

                info!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    nonce = approval.nonce,
                    amount = approval.amount,
                    "Deposit verified on EVM source chain"
                );
                Ok(VerificationResult::Valid)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    "Failed to query EVM deposit - will retry"
                );
                Ok(VerificationResult::Pending)
            }
        }
    }

    /// Verify a deposit exists on Terra source chain (V2)
    async fn verify_terra_deposit(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        debug!(
            hash = %bytes32_to_hex(&approval.withdraw_hash),
            nonce = approval.nonce,
            "Querying Terra source chain for deposit"
        );

        // Query Terra contract for deposit verification
        let query = serde_json::json!({
            "verify_deposit": {
                "deposit_hash": base64::engine::general_purpose::STANDARD.encode(approval.withdraw_hash),
                "amount": approval.amount.to_string(),
                "nonce": approval.nonce
            }
        });

        let query_b64 =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&query)?);

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            self.terra_lcd_url, self.terra_bridge_address, query_b64
        );

        match self.client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warn!(
                        status = %status,
                        body = %body,
                        "Terra query returned error status"
                    );
                    return Ok(VerificationResult::Pending);
                }

                let json: serde_json::Value = resp.json().await?;

                let exists = json["data"]["exists"].as_bool().unwrap_or(false);
                let matches = json["data"]["matches"].as_bool().unwrap_or(false);

                if exists && matches {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        nonce = approval.nonce,
                        "Deposit verified on Terra source chain"
                    );
                    Ok(VerificationResult::Valid)
                } else if exists {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "Deposit exists on Terra but parameters don't match"
                    );
                    Ok(VerificationResult::Invalid {
                        reason: "Deposit exists but parameters don't match".to_string(),
                    })
                } else {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "No deposit found on Terra source chain"
                    );
                    Ok(VerificationResult::Invalid {
                        reason: "No deposit found with this hash on source chain".to_string(),
                    })
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to query Terra deposit - will retry");
                Ok(VerificationResult::Pending)
            }
        }
    }

    /// Check if chain ID matches the known EVM chain
    fn is_evm_chain(&self, id: &[u8; 4]) -> bool {
        *id == self.evm_chain_id
    }

    /// Check if chain ID matches the known Terra chain
    fn is_terra_chain(&self, id: &[u8; 4]) -> bool {
        *id == self.terra_chain_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_matching() {
        let verifier = ApprovalVerifier::new(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            31337, // Anvil chain ID
            "http://localhost:1317",
            "terra1...",
            "localterra",
        );

        // Test EVM chain ID matching
        let anvil_id = 31337u32.to_be_bytes();
        assert!(verifier.is_evm_chain(&anvil_id));

        // BSC ID should not match Anvil
        let bsc_id = 56u32.to_be_bytes();
        assert!(!verifier.is_evm_chain(&bsc_id));

        // Test Terra chain ID matching (localterra = 5)
        let localterra_id = 5u32.to_be_bytes();
        assert!(verifier.is_terra_chain(&localterra_id));

        // Columbus-5 ID should not match localterra
        let columbus_id = 4u32.to_be_bytes();
        assert!(!verifier.is_terra_chain(&columbus_id));
    }
}
