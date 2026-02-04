//! Approval verification logic
//!
//! Verifies that withdraw approvals on the destination chain
//! correspond to valid deposits on the source chain.
//!
//! # Verification Flow
//!
//! 1. Canceler observes WithdrawApproved event on destination chain
//! 2. Verifier queries source chain for matching deposit:
//!    - For EVM source: calls `getDepositFromHash(withdrawHash)` on EVM bridge
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

use crate::hash::{bytes32_to_hex, compute_transfer_id, cosmos_chain_key, evm_chain_key};

// EVM bridge contract interface for deposit verification
sol! {
    #[sol(rpc)]
    contract CL8YBridge {
        /// Deposit structure returned by getDepositFromHash
        struct Deposit {
            bytes32 destChainKey;
            bytes32 destTokenAddress;
            bytes32 destAccount;
            address from;
            uint256 amount;
            uint256 nonce;
        }

        /// Get deposit info by hash
        function getDepositFromHash(bytes32 depositHash) external view returns (Deposit memory deposit_);
    }
}

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
    /// Timestamp when approval was created (for delay tracking)
    #[allow(dead_code)]
    pub approved_at_timestamp: u64,
    /// Delay seconds required before execution (for time-based decisions)
    #[allow(dead_code)]
    pub delay_seconds: u64,
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
    pub rpc_url: String,
    pub bridge_address: String,
}

/// Verifier for checking approvals against source chain
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
    /// Precomputed Terra chain key for quick matching
    terra_chain_key: [u8; 32],
    /// Precomputed EVM chain key for the primary chain
    evm_chain_key: [u8; 32],
}

impl ApprovalVerifier {
    pub fn new(
        evm_rpc_url: &str,
        evm_bridge_address: &str,
        evm_chain_id: u64,
        terra_lcd_url: &str,
        terra_bridge_address: &str,
        terra_chain_id: &str,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        let terra_chain_key_computed = cosmos_chain_key(terra_chain_id);
        let evm_chain_key_computed = evm_chain_key(evm_chain_id);

        Self {
            client,
            evm_rpc_url: evm_rpc_url.to_string(),
            evm_bridge_address: evm_bridge_address.to_string(),
            terra_lcd_url: terra_lcd_url.to_string(),
            terra_bridge_address: terra_bridge_address.to_string(),
            terra_chain_key: terra_chain_key_computed,
            evm_chain_key: evm_chain_key_computed,
        }
    }

    /// Verify an approval against the source chain
    pub async fn verify(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        // First, verify the hash is correctly computed from the approval parameters
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

        // Determine source chain type from chain key
        // If src_chain_key matches our known EVM chain key, it's an EVM source
        // If src_chain_key matches Terra chain key, it's a Terra source

        if self.is_evm_chain_key(&approval.src_chain_key) {
            debug!(
                hash = %bytes32_to_hex(&approval.withdraw_hash),
                "Source is EVM chain, verifying deposit on EVM"
            );
            return self.verify_evm_deposit(approval).await;
        }

        if self.is_terra_chain_key(&approval.src_chain_key) {
            debug!(
                hash = %bytes32_to_hex(&approval.withdraw_hash),
                "Source is Terra chain, verifying deposit on Terra"
            );
            return self.verify_terra_deposit(approval).await;
        }

        // Unknown source chain - this is fraudulent!
        // An approval claiming to come from an unregistered/unknown chain
        // cannot have a valid deposit, so we should cancel it.
        warn!(
            src_chain_key = %bytes32_to_hex(&approval.src_chain_key),
            withdraw_hash = %bytes32_to_hex(&approval.withdraw_hash),
            "Unknown source chain key - marking as invalid (no deposit can exist)"
        );
        Ok(VerificationResult::Invalid {
            reason: format!(
                "Unknown source chain key {} - cannot verify deposit",
                bytes32_to_hex(&approval.src_chain_key)
            ),
        })
    }

    /// Verify a deposit exists on EVM source chain
    ///
    /// Queries the EVM bridge contract's `getDepositFromHash(withdrawHash)` function.
    /// The deposit hash on the source chain equals the withdraw hash on the destination chain
    /// because both are computed from the same canonical transfer ID.
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

        let contract = CL8YBridge::new(bridge_address, &provider);

        // Query the deposit by hash
        // The withdraw hash on destination equals the deposit hash on source
        let deposit_hash = FixedBytes::from(approval.withdraw_hash);

        match contract.getDepositFromHash(deposit_hash).call().await {
            Ok(deposit_result) => {
                let deposit = deposit_result.deposit_;

                // Check if deposit exists (amount > 0 indicates existence)
                let deposit_amount: u128 = deposit.amount.try_into().unwrap_or(0);
                let deposit_nonce: u64 = deposit.nonce.try_into().unwrap_or(0);

                if deposit_amount == 0 && deposit_nonce == 0 && deposit.from == Address::ZERO {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "No deposit found on EVM source chain"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: "No deposit found with this hash on source chain".to_string(),
                    });
                }

                // Verify the deposit parameters match the approval
                // The deposit's destChainKey should match the approval's dest_chain_key
                let deposit_dest_chain_key: [u8; 32] = deposit.destChainKey.0;
                if deposit_dest_chain_key != approval.dest_chain_key {
                    info!(
                        expected = %bytes32_to_hex(&approval.dest_chain_key),
                        got = %bytes32_to_hex(&deposit_dest_chain_key),
                        "Destination chain key mismatch"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: "Destination chain key mismatch".to_string(),
                    });
                }

                // Verify dest token address
                let deposit_dest_token: [u8; 32] = deposit.destTokenAddress.0;
                if deposit_dest_token != approval.dest_token_address {
                    info!(
                        expected = %bytes32_to_hex(&approval.dest_token_address),
                        got = %bytes32_to_hex(&deposit_dest_token),
                        "Destination token address mismatch"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: "Destination token address mismatch".to_string(),
                    });
                }

                // Verify dest account
                let deposit_dest_account: [u8; 32] = deposit.destAccount.0;
                if deposit_dest_account != approval.dest_account {
                    info!(
                        expected = %bytes32_to_hex(&approval.dest_account),
                        got = %bytes32_to_hex(&deposit_dest_account),
                        "Destination account mismatch"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: "Destination account mismatch".to_string(),
                    });
                }

                // Verify amount
                if deposit_amount != approval.amount {
                    info!(
                        expected = approval.amount,
                        got = deposit_amount,
                        "Amount mismatch"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: format!(
                            "Amount mismatch: expected {}, got {}",
                            approval.amount, deposit_amount
                        ),
                    });
                }

                // Verify nonce
                if deposit_nonce != approval.nonce {
                    info!(
                        expected = approval.nonce,
                        got = deposit_nonce,
                        "Nonce mismatch"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: format!(
                            "Nonce mismatch: expected {}, got {}",
                            approval.nonce, deposit_nonce
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

    /// Verify a deposit exists on Terra source chain
    ///
    /// Queries the Terra bridge contract's `VerifyDeposit` query.
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
                "dest_chain_key": base64::engine::general_purpose::STANDARD.encode(approval.dest_chain_key),
                "dest_token_address": base64::engine::general_purpose::STANDARD.encode(approval.dest_token_address),
                "dest_account": base64::engine::general_purpose::STANDARD.encode(approval.dest_account),
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

    /// Check if chain key matches the known EVM chain
    fn is_evm_chain_key(&self, key: &[u8; 32]) -> bool {
        // Check if key matches our configured EVM chain
        *key == self.evm_chain_key
    }

    /// Check if chain key matches the known Terra chain
    fn is_terra_chain_key(&self, key: &[u8; 32]) -> bool {
        // Check if key matches our configured Terra chain
        *key == self.terra_chain_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_key_matching() {
        let verifier = ApprovalVerifier::new(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            31337, // Anvil chain ID
            "http://localhost:1317",
            "terra1...",
            "localterra",
        );

        // Test EVM chain key matching
        let anvil_key = evm_chain_key(31337);
        assert!(verifier.is_evm_chain_key(&anvil_key));

        // BSC key should not match Anvil
        let bsc_key = evm_chain_key(56);
        assert!(!verifier.is_evm_chain_key(&bsc_key));

        // Test Terra chain key matching
        let localterra_key = cosmos_chain_key("localterra");
        assert!(verifier.is_terra_chain_key(&localterra_key));

        // Columbus-5 key should not match localterra
        let columbus_key = cosmos_chain_key("columbus-5");
        assert!(!verifier.is_terra_chain_key(&columbus_key));
    }
}
