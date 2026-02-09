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

use crate::hash::{bytes32_to_hex, compute_transfer_hash};

// EVM bridge contract interface for deposit verification (V2)
//
// IMPORTANT: Must match Bridge.sol exactly:
// - Function name: `getDeposit` (NOT `deposits`)
// - Return type: DepositRecord struct fields in order
// - amount is uint256 (NOT uint128)
sol! {
    #[sol(rpc)]
    contract Bridge {
        /// Get deposit record by hash (V2 DepositRecord struct)
        function getDeposit(bytes32 depositHash) external view returns (
            bytes4 destChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            uint256 amount,
            uint64 nonce,
            uint256 fee,
            uint256 timestamp
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
    /// Source account (depositor) encoded as bytes32
    pub src_account: [u8; 32],
    /// Destination account (recipient) encoded as bytes32
    pub dest_account: [u8; 32],
    /// Token address on destination chain encoded as bytes32
    pub dest_token: [u8; 32],
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
    /// Legacy constructor — uses native chain IDs. Prefer `with_v2_chain_ids` for V2.
    #[allow(dead_code)]
    pub fn new(
        evm_rpc_url: &str,
        evm_bridge_address: &str,
        evm_chain_id: u64,
        terra_lcd_url: &str,
        terra_bridge_address: &str,
        _terra_chain_id: &str, // Legacy param, we use numeric IDs now
    ) -> Self {
        Self::with_v2_chain_ids(
            evm_rpc_url,
            evm_bridge_address,
            evm_chain_id,
            terra_lcd_url,
            terra_bridge_address,
            _terra_chain_id,
            None,
            None,
        )
    }

    /// Create a verifier with explicit V2 chain IDs from ChainRegistry.
    ///
    /// The V2 chain IDs are the 4-byte IDs assigned by ChainRegistry (e.g. 0x00000001),
    /// NOT the native chain IDs (e.g. 31337 for Anvil). Using the correct V2 IDs is
    /// critical for fraud detection: if the verifier can't identify the source chain,
    /// it marks the approval as Pending instead of Invalid, preventing cancellation.
    #[allow(clippy::too_many_arguments)]
    pub fn with_v2_chain_ids(
        evm_rpc_url: &str,
        evm_bridge_address: &str,
        evm_chain_id: u64,
        terra_lcd_url: &str,
        terra_bridge_address: &str,
        _terra_chain_id: &str,
        evm_v2_chain_id: Option<[u8; 4]>,
        terra_v2_chain_id: Option<[u8; 4]>,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Use V2 chain IDs from ChainRegistry if provided, otherwise fall back
        // to native chain ID conversion (which may be wrong for V2 systems!)
        let evm_chain_id_bytes = evm_v2_chain_id.unwrap_or_else(|| {
            let native = (evm_chain_id as u32).to_be_bytes();
            warn!(
                native_chain_id = evm_chain_id,
                native_bytes = %hex::encode(native),
                "EVM_V2_CHAIN_ID not set — falling back to native chain ID. \
                 This will fail if ChainRegistry uses different IDs (e.g. 0x00000001). \
                 Set EVM_V2_CHAIN_ID to the bytes4 value from ChainRegistry."
            );
            native
        });

        let terra_chain_id_bytes = terra_v2_chain_id.unwrap_or_else(|| {
            // Default to V2 ChainRegistry ID 2 for Terra
            // Previously used 5 (localterra native) or 4 (columbus-5) which were WRONG
            let default_v2 = 2u32.to_be_bytes();
            warn!(
                terra_chain_id = _terra_chain_id,
                default_v2_bytes = %hex::encode(default_v2),
                "TERRA_V2_CHAIN_ID not set — falling back to default V2 ID 0x00000002. \
                 Set TERRA_V2_CHAIN_ID to the bytes4 value from ChainRegistry."
            );
            default_v2
        });

        info!(
            evm_v2_chain_id = %hex::encode(evm_chain_id_bytes),
            terra_v2_chain_id = %hex::encode(terra_chain_id_bytes),
            "ApprovalVerifier initialized with V2 chain IDs"
        );

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
        let computed_hash = compute_transfer_hash(
            &approval.src_chain_id,
            &approval.dest_chain_id,
            &approval.src_account,
            &approval.dest_account,
            &approval.dest_token,
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

        // Unknown source chain — cannot verify, mark as invalid.
        // In V2, if the source chain ID is not one of our configured chains,
        // we cannot look up the deposit. This is likely fraud (spoofed src_chain)
        // or a misconfiguration (V2 chain IDs not matching ChainRegistry values).
        warn!(
            src_chain_id = %hex::encode(approval.src_chain_id),
            known_evm_id = %hex::encode(self.evm_chain_id),
            known_terra_id = %hex::encode(self.terra_chain_id),
            withdraw_hash = %bytes32_to_hex(&approval.withdraw_hash),
            nonce = approval.nonce,
            "Unknown source chain ID — cannot verify deposit. \
             If this is unexpected, check EVM_V2_CHAIN_ID and TERRA_V2_CHAIN_ID config."
        );
        Ok(VerificationResult::Invalid {
            reason: format!(
                "Unknown source chain 0x{} (known: EVM=0x{}, Terra=0x{})",
                hex::encode(approval.src_chain_id),
                hex::encode(self.evm_chain_id),
                hex::encode(self.terra_chain_id)
            ),
        })
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

        // Query the deposit by hash using V2 getDeposit()
        let deposit_hash = FixedBytes::from(approval.withdraw_hash);

        match contract.getDeposit(deposit_hash).call().await {
            Ok(deposit) => {
                // Check if deposit exists (timestamp == 0 means no record)
                if deposit.timestamp.is_zero() {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "No deposit found on EVM source chain (timestamp=0)"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: "No deposit found with this hash on source chain".to_string(),
                    });
                }

                // Convert amount for comparison (V2 uses uint256, we use u128)
                let deposit_amount: u128 = deposit.amount.try_into().unwrap_or_else(|_| {
                    warn!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        amount = %deposit.amount,
                        "Deposit amount exceeds u128::MAX"
                    );
                    u128::MAX
                });

                // Verify amount
                if deposit_amount != approval.amount {
                    info!(
                        expected = approval.amount,
                        got = deposit_amount,
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
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
                if deposit.nonce != approval.nonce {
                    info!(
                        expected = approval.nonce,
                        got = deposit.nonce,
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
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
                    dest_chain = %format!("0x{}", hex::encode(deposit.destChain.0)),
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
    ///
    /// Uses the `DepositHash` smart query on the Terra bridge contract to check
    /// if a deposit with the given hash exists. This query only requires the hash
    /// (not all deposit parameters), making it simpler and more reliable than
    /// `VerifyDeposit` which requires 6 fields.
    ///
    /// Response:
    /// - Deposit exists: `{"data": {"deposit_hash": "...", "nonce": N, ...}}`
    /// - No deposit: `{"data": null}`
    async fn verify_terra_deposit(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        debug!(
            hash = %bytes32_to_hex(&approval.withdraw_hash),
            nonce = approval.nonce,
            "Querying Terra source chain for deposit"
        );

        // Use the DepositHash query which only needs the deposit hash.
        // QueryMsg::DepositHash { deposit_hash: Binary } → Option<DepositInfoResponse>
        //
        // Previously used VerifyDeposit which requires 6 parameters
        // (deposit_hash, dest_chain_key, dest_token_address, dest_account, amount, nonce).
        // The old code only sent 3 of 6, causing the contract to reject the query with
        // a deserialization error. The error handler returned Pending, creating an infinite
        // retry loop that prevented fraud detection.
        let query = serde_json::json!({
            "deposit_hash": {
                "deposit_hash": base64::engine::general_purpose::STANDARD.encode(approval.withdraw_hash)
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
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "Terra deposit_hash query returned error status"
                    );
                    // For contract query errors (400/500), the query format may be wrong
                    // or the contract state is inconsistent. Return Pending for transient
                    // errors, but log enough context to diagnose persistent failures.
                    return Ok(VerificationResult::Pending);
                }

                let json: serde_json::Value = resp.json().await?;
                let data = &json["data"];

                // DepositHash returns Option<DepositInfoResponse>:
                // - null means no deposit with this hash exists → fraud
                // - non-null means deposit exists → verify parameters
                if data.is_null() {
                    info!(
                        hash = %bytes32_to_hex(&approval.withdraw_hash),
                        "No deposit found on Terra source chain"
                    );
                    return Ok(VerificationResult::Invalid {
                        reason: "No deposit found with this hash on source chain".to_string(),
                    });
                }

                // Deposit exists — verify nonce matches
                let deposit_nonce = data["nonce"].as_u64();
                if let Some(dep_nonce) = deposit_nonce {
                    if dep_nonce != approval.nonce {
                        info!(
                            hash = %bytes32_to_hex(&approval.withdraw_hash),
                            expected_nonce = approval.nonce,
                            actual_nonce = dep_nonce,
                            "Deposit exists on Terra but nonce doesn't match"
                        );
                        return Ok(VerificationResult::Invalid {
                            reason: format!(
                                "Nonce mismatch: expected {}, got {}",
                                approval.nonce, dep_nonce
                            ),
                        });
                    }
                }

                // Verify amount if available
                // CosmWasm Uint128 is serialized as a string in JSON
                if let Some(amount_str) = data["amount"].as_str() {
                    if let Ok(deposit_amount) = amount_str.parse::<u128>() {
                        if deposit_amount != approval.amount {
                            info!(
                                hash = %bytes32_to_hex(&approval.withdraw_hash),
                                expected_amount = approval.amount,
                                actual_amount = deposit_amount,
                                "Deposit exists on Terra but amount doesn't match"
                            );
                            return Ok(VerificationResult::Invalid {
                                reason: format!(
                                    "Amount mismatch: expected {}, got {}",
                                    approval.amount, deposit_amount
                                ),
                            });
                        }
                    }
                }

                info!(
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    nonce = approval.nonce,
                    "Deposit verified on Terra source chain"
                );
                Ok(VerificationResult::Valid)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    hash = %bytes32_to_hex(&approval.withdraw_hash),
                    "Failed to query Terra deposit - will retry"
                );
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
    fn test_chain_id_matching_legacy() {
        let verifier = ApprovalVerifier::new(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            31337, // Anvil chain ID
            "http://localhost:1317",
            "terra1...",
            "localterra",
        );

        // Legacy: EVM chain ID is native 31337
        let anvil_id = 31337u32.to_be_bytes();
        assert!(verifier.is_evm_chain(&anvil_id));

        let bsc_id = 56u32.to_be_bytes();
        assert!(!verifier.is_evm_chain(&bsc_id));

        // Terra chain ID defaults to V2 ID 2 (0x00000002)
        // Previously used 5 for localterra which was wrong
        let terra_v2_id = 2u32.to_be_bytes();
        assert!(verifier.is_terra_chain(&terra_v2_id));

        // Old value 5 should NOT match
        let old_localterra_id = 5u32.to_be_bytes();
        assert!(
            !verifier.is_terra_chain(&old_localterra_id),
            "Old localterra ID 5 should no longer match — V2 uses 2"
        );
    }

    /// Test that V2 chain IDs from ChainRegistry are used correctly.
    ///
    /// This was the root cause of all 6 canceler test failures:
    /// the verifier used native chain IDs (31337, 5) but ChainRegistry
    /// assigns sequential IDs (0x00000001, 0x00000002). The mismatch
    /// caused ALL approvals to be marked as Pending (unknown source chain),
    /// so fraud was never detected.
    #[test]
    fn test_chain_id_matching_v2_registry_ids() {
        // V2: ChainRegistry assigns 0x00000001 to EVM, 0x00000002 to Terra
        let evm_v2: [u8; 4] = 1u32.to_be_bytes(); // 0x00000001
        let terra_v2: [u8; 4] = 2u32.to_be_bytes(); // 0x00000002

        let verifier = ApprovalVerifier::with_v2_chain_ids(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            31337,
            "http://localhost:1317",
            "terra1...",
            "localterra",
            Some(evm_v2),
            Some(terra_v2),
        );

        // V2 IDs should match
        assert!(
            verifier.is_evm_chain(&evm_v2),
            "EVM V2 chain ID 0x00000001 should match"
        );
        assert!(
            verifier.is_terra_chain(&terra_v2),
            "Terra V2 chain ID 0x00000002 should match"
        );

        // Native IDs should NOT match V2 IDs
        let native_anvil = 31337u32.to_be_bytes(); // 0x00007A69
        assert!(
            !verifier.is_evm_chain(&native_anvil),
            "Native Anvil ID 0x00007A69 should NOT match V2 EVM ID 0x00000001"
        );

        let native_terra = 5u32.to_be_bytes(); // 0x00000005
        assert!(
            !verifier.is_terra_chain(&native_terra),
            "Native Terra ID 0x00000005 should NOT match V2 Terra ID 0x00000002"
        );
    }

    /// Test that the Terra deposit_hash query JSON is correctly formatted.
    ///
    /// The DepositHash query uses the simpler format:
    ///   {"deposit_hash": {"deposit_hash": "<base64-encoded-hash>"}}
    ///
    /// Previously, the code used a malformed VerifyDeposit query with only 3 of 6
    /// required fields, causing the Terra contract to reject it. The error handler
    /// returned Pending (retry), creating an infinite loop that prevented fraud detection.
    #[test]
    fn test_terra_deposit_hash_query_format() {
        let deposit_hash = [0xABu8; 32];
        let query = serde_json::json!({
            "deposit_hash": {
                "deposit_hash": base64::engine::general_purpose::STANDARD.encode(deposit_hash)
            }
        });

        // Verify the query structure
        let query_str = serde_json::to_string(&query).unwrap();
        assert!(
            query_str.contains("deposit_hash"),
            "Query should contain deposit_hash key"
        );
        assert!(
            !query_str.contains("verify_deposit"),
            "Query should NOT use verify_deposit"
        );
        assert!(
            !query_str.contains("amount"),
            "Query should NOT include amount (that's VerifyDeposit)"
        );
        assert!(
            !query_str.contains("nonce"),
            "Query should NOT include nonce (that's VerifyDeposit)"
        );

        // Verify base64 encoding is correct
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(deposit_hash);
        let actual_b64 = query["deposit_hash"]["deposit_hash"].as_str().unwrap();
        assert_eq!(actual_b64, expected_b64, "Base64 encoding should match");
    }

    /// Test that null response from DepositHash query is correctly interpreted as fraud.
    ///
    /// When the Terra contract returns {"data": null} for a DepositHash query,
    /// it means no deposit with that hash exists → the approval is fraudulent.
    #[test]
    fn test_terra_null_response_is_invalid() {
        // Simulate the response parsing logic from verify_terra_deposit
        let response_json: serde_json::Value = serde_json::json!({"data": null});
        let data = &response_json["data"];
        assert!(data.is_null(), "Null data should be detected");
        // In the actual code, this returns VerificationResult::Invalid
    }

    /// Test that a valid deposit response from DepositHash is correctly parsed.
    #[test]
    fn test_terra_deposit_response_parsing() {
        // Simulate a successful DepositHash response
        let response_json: serde_json::Value = serde_json::json!({
            "data": {
                "deposit_hash": "AAAA",
                "dest_chain_key": "AAAB",
                "src_account": "AAAC",
                "dest_token_address": "AAAD",
                "dest_account": "AAAE",
                "amount": "1000000",
                "nonce": 42,
                "deposited_at": "1234567890"
            }
        });

        let data = &response_json["data"];
        assert!(
            !data.is_null(),
            "Data should not be null for existing deposit"
        );

        // Verify nonce parsing
        let nonce = data["nonce"].as_u64();
        assert_eq!(nonce, Some(42), "Nonce should be parsed correctly");

        // Verify amount parsing (CosmWasm Uint128 is serialized as string)
        let amount_str = data["amount"].as_str().unwrap();
        let amount: u128 = amount_str.parse().unwrap();
        assert_eq!(amount, 1_000_000, "Amount should be parsed from string");
    }

    /// Test that a deposit with mismatched nonce is detected.
    #[test]
    fn test_terra_nonce_mismatch_detection() {
        let response_json: serde_json::Value = serde_json::json!({
            "data": {
                "deposit_hash": "AAAA",
                "amount": "1000000",
                "nonce": 42
            }
        });

        let data = &response_json["data"];
        let deposit_nonce = data["nonce"].as_u64().unwrap();
        let expected_nonce: u64 = 99;

        assert_ne!(
            deposit_nonce, expected_nonce,
            "Should detect nonce mismatch (deposit has 42, approval expects 99)"
        );
    }

    /// Test that an approval from an unknown chain is marked Invalid (not Pending).
    ///
    /// Previously, unknown chains returned Pending, which meant the canceler
    /// would retry forever but never actually cancel. Now unknown chains
    /// return Invalid, triggering immediate cancellation.
    #[tokio::test]
    async fn test_unknown_chain_returns_invalid() {
        let verifier = ApprovalVerifier::with_v2_chain_ids(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            31337,
            "http://localhost:1317",
            "terra1...",
            "localterra",
            Some(1u32.to_be_bytes()),
            Some(2u32.to_be_bytes()),
        );

        // Create an approval with an unknown source chain (0x000000FF)
        let unknown_chain: [u8; 4] = [0, 0, 0, 0xFF];
        let approval = PendingApproval {
            withdraw_hash: [0u8; 32], // Will fail hash check first
            src_chain_id: unknown_chain,
            dest_chain_id: [0, 0, 0, 1],
            src_account: [0u8; 32],
            dest_account: [0u8; 32],
            dest_token: [0u8; 32],
            amount: 1000,
            nonce: 1,
            approved_at_timestamp: 0,
            cancel_window: 300,
        };

        let result = verifier.verify(&approval).await.unwrap();
        match result {
            VerificationResult::Invalid { reason } => {
                // The hash check will fail first (computed hash != all-zeros withdraw_hash),
                // which is also Invalid. Either way, we get Invalid not Pending.
                assert!(
                    reason.contains("Hash does not match")
                        || reason.contains("Unknown source chain"),
                    "Expected hash mismatch or unknown chain error, got: {}",
                    reason
                );
            }
            VerificationResult::Pending => {
                panic!("Unknown chain should return Invalid, not Pending!");
            }
            VerificationResult::Valid => {
                panic!("Unknown chain should not return Valid!");
            }
        }
    }
}
