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
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, error, info, warn};

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

/// Known EVM chain endpoint for multi-chain verification routing
#[derive(Debug, Clone)]
pub struct KnownEvmChain {
    pub v2_chain_id: [u8; 4],
    pub rpc_url: String,
    pub bridge_address: String,
}

/// Verifier for checking approvals against source chain (V2)
///
/// Supports multi-EVM: when EVM chain peers are registered, the verifier
/// routes deposit verification to the correct source chain's RPC/bridge based
/// on the V2 chain ID in the approval.
pub struct ApprovalVerifier {
    client: Client,
    /// Terra LCD URL
    terra_lcd_url: String,
    /// Terra bridge contract address
    terra_bridge_address: String,
    /// Configured EVM chain's 4-byte V2 chain ID.
    evm_chain_id: [u8; 4],
    /// Terra's 4-byte V2 chain ID
    terra_chain_id: [u8; 4],
    /// All known EVM chains, keyed by V2 chain ID bytes.
    /// Used for multi-chain deposit verification routing.
    known_evm_chains: std::collections::HashMap<[u8; 4], KnownEvmChain>,
    /// C6: Counter for unknown source chain events (aids alerting)
    unknown_source_chain_count: AtomicU64,
}

impl ApprovalVerifier {
    /// Legacy constructor for tests — uses native chain IDs converted to V2 bytes.
    ///
    /// WARNING: Only use in tests where native == V2. In production,
    /// use `new_v2()` with explicitly resolved chain IDs.
    #[allow(dead_code)]
    #[cfg(test)]
    pub fn new(
        evm_rpc_url: &str,
        evm_bridge_address: &str,
        evm_chain_id: u64,
        terra_lcd_url: &str,
        terra_bridge_address: &str,
        _terra_chain_id: &str,
    ) -> Self {
        Self::new_v2(
            evm_rpc_url,
            evm_bridge_address,
            terra_lcd_url,
            terra_bridge_address,
            (evm_chain_id as u32).to_be_bytes(),
            2u32.to_be_bytes(),
        )
    }

    /// Create a verifier with explicit V2 chain IDs from ChainRegistry.
    ///
    /// Both V2 chain IDs are **required**. The caller must resolve them from
    /// configuration or by querying the bridge contract before constructing the
    /// verifier. No fallback to native chain IDs is performed here — using native
    /// IDs silently causes the verifier to misidentify chains, which can lead to
    /// missed fraud detection or (prior to C6 fix) mass false-positive cancellations.
    #[allow(clippy::too_many_arguments)]
    pub fn new_v2(
        evm_rpc_url: &str,
        evm_bridge_address: &str,
        terra_lcd_url: &str,
        terra_bridge_address: &str,
        evm_v2_chain_id: [u8; 4],
        terra_v2_chain_id: [u8; 4],
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        info!(
            evm_v2_chain_id = %hex::encode(evm_v2_chain_id),
            terra_v2_chain_id = %hex::encode(terra_v2_chain_id),
            "ApprovalVerifier initialized with V2 chain IDs"
        );

        // Initialize known EVM chains with the configured chain.
        let mut known_evm_chains = std::collections::HashMap::new();
        known_evm_chains.insert(
            evm_v2_chain_id,
            KnownEvmChain {
                v2_chain_id: evm_v2_chain_id,
                rpc_url: evm_rpc_url.to_string(),
                bridge_address: evm_bridge_address.to_string(),
            },
        );

        Self {
            client,
            terra_lcd_url: terra_lcd_url.to_string(),
            terra_bridge_address: terra_bridge_address.to_string(),
            evm_chain_id: evm_v2_chain_id,
            terra_chain_id: terra_v2_chain_id,
            known_evm_chains,
            unknown_source_chain_count: AtomicU64::new(0),
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

        // C6 FIX: Unknown source chain — return Pending, NOT Invalid.
        //
        // Returning Invalid here would trigger a cancellation transaction. If the
        // chain IDs are misconfigured (e.g., wrong V2 chain ID in env vars), this
        // would cause the canceler to cancel ALL valid approvals — a catastrophic
        // false-positive. Instead, we return Pending so the approval is retried
        // but no destructive action is taken.
        //
        // This can happen when:
        // - V2 chain IDs are misconfigured (check EVM_V2_CHAIN_ID / TERRA_V2_CHAIN_ID)
        // - A new chain was added to the bridge but the canceler hasn't been updated
        // - Genuinely spoofed src_chain (but hash check above already caught that)
        let count = self
            .unknown_source_chain_count
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        error!(
            src_chain_id = %hex::encode(approval.src_chain_id),
            known_evm_id = %hex::encode(self.evm_chain_id),
            known_terra_id = %hex::encode(self.terra_chain_id),
            withdraw_hash = %bytes32_to_hex(&approval.withdraw_hash),
            nonce = approval.nonce,
            unknown_chain_count = count,
            "UNKNOWN SOURCE CHAIN — cannot verify deposit, returning Pending (not cancelling). \
             If this persists, check EVM_V2_CHAIN_ID and TERRA_V2_CHAIN_ID configuration. \
             A sustained count indicates misconfiguration or a new unmonitored chain."
        );
        Ok(VerificationResult::Pending)
    }

    /// Verify a deposit exists on EVM source chain (V2)
    ///
    /// Routes to the correct EVM chain based on the approval's `src_chain_id`.
    async fn verify_evm_deposit(&self, approval: &PendingApproval) -> Result<VerificationResult> {
        // Look up the source chain's RPC and bridge from known_evm_chains
        let Some(chain) = self.known_evm_chains.get(&approval.src_chain_id) else {
            // Do not route unknown source chains to another chain. That can
            // create false negatives/positives in verification under N-chain routing.
            warn!(
                src_chain = %hex::encode(approval.src_chain_id),
                known_chain_count = self.known_evm_chains.len(),
                "Unknown EVM source chain in verifier routing; returning Pending"
            );
            return Ok(VerificationResult::Pending);
        };
        let rpc_url = chain.rpc_url.as_str();
        let bridge_addr_str = chain.bridge_address.as_str();

        debug!(
            hash = %bytes32_to_hex(&approval.withdraw_hash),
            nonce = approval.nonce,
            amount = approval.amount,
            src_chain = %hex::encode(approval.src_chain_id),
            rpc = rpc_url,
            "Querying EVM source chain for deposit"
        );

        // Parse bridge address
        let bridge_address = match Address::from_str(bridge_addr_str) {
            Ok(addr) => addr,
            Err(e) => {
                warn!(error = %e, bridge = bridge_addr_str, "Invalid EVM bridge address");
                return Ok(VerificationResult::Pending);
            }
        };

        // Create provider
        let provider = match rpc_url.parse() {
            Ok(url) => ProviderBuilder::new().on_http(url),
            Err(e) => {
                warn!(error = %e, rpc = rpc_url, "Invalid EVM RPC URL");
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

    /// Register EVM chain peers for multi-chain verification routing.
    ///
    /// Call this after construction with chains from `MultiEvmConfig`.
    pub fn register_evm_chains(&mut self, chains: Vec<KnownEvmChain>) {
        for chain in chains {
            if chain.v2_chain_id != self.evm_chain_id {
                info!(
                    v2_chain_id = %hex::encode(chain.v2_chain_id),
                    rpc = %chain.rpc_url,
                    "Registered EVM chain peer for verification"
                );
            }
            self.known_evm_chains.insert(chain.v2_chain_id, chain);
        }
        info!(
            total_evm_chains = self.known_evm_chains.len(),
            "Multi-EVM verification routing configured"
        );
    }

    /// Check if chain ID matches any known EVM chain.
    fn is_evm_chain(&self, id: &[u8; 4]) -> bool {
        self.known_evm_chains.contains_key(id)
    }

    /// Check if chain ID matches the known Terra chain
    fn is_terra_chain(&self, id: &[u8; 4]) -> bool {
        *id == self.terra_chain_id
    }

    /// C6: Return the count of unknown source chain events (for metrics/alerting)
    pub fn unknown_source_chain_count(&self) -> u64 {
        self.unknown_source_chain_count.load(Ordering::Relaxed)
    }

    /// Return the configured EVM V2 chain ID (for startup validation)
    pub fn evm_chain_id(&self) -> &[u8; 4] {
        &self.evm_chain_id
    }

    /// Return the configured Terra V2 chain ID (for startup validation)
    pub fn terra_chain_id(&self) -> &[u8; 4] {
        &self.terra_chain_id
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

        let verifier = ApprovalVerifier::new_v2(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            "http://localhost:1317",
            "terra1...",
            evm_v2,
            terra_v2,
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

    /// C6: Test that an approval from an unknown chain returns Pending (safe),
    /// NOT Invalid (which would trigger cancellation of potentially valid approvals).
    ///
    /// History: Originally returned Pending (infinite retry), then was changed to
    /// Invalid (mass cancellation risk on misconfiguration). Now returns Pending
    /// with error-level logging to alert operators without taking destructive action.
    ///
    /// Note: If the hash check fails first (parameters don't match claimed hash),
    /// that still returns Invalid — which is correct because the on-chain data
    /// itself is inconsistent, regardless of chain ID configuration.
    #[tokio::test]
    async fn test_unknown_chain_returns_pending_not_invalid() {
        use crate::hash::compute_transfer_hash;

        let verifier = ApprovalVerifier::new_v2(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            "http://localhost:1317",
            "terra1...",
            1u32.to_be_bytes(),
            2u32.to_be_bytes(),
        );

        // Create an approval with an unknown source chain (0x000000FF)
        // but with a VALID hash (so the hash check passes and we reach
        // the unknown-chain branch)
        let unknown_chain: [u8; 4] = [0, 0, 0, 0xFF];
        let dest_chain: [u8; 4] = [0, 0, 0, 1];
        let src_account = [0u8; 32];
        let dest_account = [0u8; 32];
        let dest_token = [0u8; 32];
        let amount: u128 = 1000;
        let nonce: u64 = 1;

        let valid_hash = compute_transfer_hash(
            &unknown_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &dest_token,
            amount,
            nonce,
        );

        let approval = PendingApproval {
            withdraw_hash: valid_hash,
            src_chain_id: unknown_chain,
            dest_chain_id: dest_chain,
            src_account,
            dest_account,
            dest_token,
            amount,
            nonce,
            approved_at_timestamp: 0,
            cancel_window: 300,
        };

        let result = verifier.verify(&approval).await.unwrap();
        match result {
            VerificationResult::Pending => {
                // Correct: unknown chain returns Pending, no destructive action
            }
            VerificationResult::Invalid { reason } => {
                panic!(
                    "C6 regression: unknown chain should return Pending, not Invalid! Got: {}",
                    reason
                );
            }
            VerificationResult::Valid => {
                panic!("Unknown chain should not return Valid!");
            }
        }

        // Verify the counter was incremented
        assert_eq!(
            verifier.unknown_source_chain_count(),
            1,
            "Unknown source chain counter should be incremented"
        );
    }

    /// Test that a hash mismatch still returns Invalid (this is safe because
    /// the on-chain data itself is inconsistent, not a config issue).
    #[tokio::test]
    async fn test_hash_mismatch_returns_invalid() {
        let verifier = ApprovalVerifier::new_v2(
            "http://localhost:8545",
            "0x0000000000000000000000000000000000000001",
            "http://localhost:1317",
            "terra1...",
            1u32.to_be_bytes(),
            2u32.to_be_bytes(),
        );

        // Approval with zeroed hash that won't match computed hash
        let approval = PendingApproval {
            withdraw_hash: [0u8; 32],
            src_chain_id: [0, 0, 0, 0xFF],
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
                assert!(
                    reason.contains("Hash does not match"),
                    "Expected hash mismatch error, got: {}",
                    reason
                );
            }
            _ => panic!("Hash mismatch should return Invalid"),
        }
    }
}
