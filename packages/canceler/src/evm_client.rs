//! EVM Client for canceler transactions
//!
//! Handles signing and submitting CancelWithdrawApproval transactions to EVM chains.
//!
//! # Transaction Building
//!
//! Uses Alloy's `ProviderBuilder::with_recommended_fillers()` to automatically
//! populate transaction fields (nonce, gas_limit, max_fee_per_gas, max_priority_fee_per_gas).
//! Without this, transactions will fail with missing property errors.
//!
//! # Usage
//!
//! ```ignore
//! let client = EvmClient::new(config).await?;
//! client.cancel_withdraw_approval(withdraw_hash).await?;
//! ```

#![allow(dead_code)]

use alloy::network::EthereumWallet;
use alloy::primitives::{Address, FixedBytes};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use eyre::{eyre, Result, WrapErr};
use std::str::FromStr;
use tracing::{debug, info};

use crate::hash::bytes32_to_hex;

sol! {
    /// CL8YBridge contract interface for cancellation
    #[sol(rpc)]
    contract CL8YBridge {
        /// Cancel a previously approved withdrawal
        function cancelWithdrawApproval(bytes32 withdrawHash) external;

        /// Get approval info for a given withdraw hash
        function getWithdrawApproval(bytes32 withdrawHash) external view returns (
            uint256 fee,
            address feeRecipient,
            uint64 approvedAt,
            bool isApproved,
            bool deductFromAmount,
            bool cancelled,
            bool executed
        );

        /// Query the withdraw delay in seconds
        function withdrawDelay() external view returns (uint256);
    }
}

/// EVM client for canceler transactions
pub struct EvmClient {
    rpc_url: String,
    bridge_address: Address,
    signer: PrivateKeySigner,
}

impl EvmClient {
    /// Create a new EVM client
    pub fn new(rpc_url: &str, bridge_address: &str, private_key: &str) -> Result<Self> {
        let bridge_address =
            Address::from_str(bridge_address).wrap_err("Invalid bridge address")?;
        let signer: PrivateKeySigner = private_key.parse().wrap_err("Invalid private key")?;

        info!(
            canceler_address = %signer.address(),
            bridge = %bridge_address,
            "EVM client initialized"
        );

        Ok(Self {
            rpc_url: rpc_url.to_string(),
            bridge_address,
            signer,
        })
    }

    /// Cancel a withdraw approval on EVM
    pub async fn cancel_withdraw_approval(&self, withdraw_hash: [u8; 32]) -> Result<String> {
        // Build provider with signer and gas filler
        // Use with_recommended_fillers() to automatically fill nonce, gas, and fees
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = CL8YBridge::new(self.bridge_address, &provider);

        // First check if the approval exists and is not already cancelled/executed
        let approval = contract
            .getWithdrawApproval(FixedBytes::from(withdraw_hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get approval: {}", e))?;

        if !approval.isApproved {
            return Err(eyre!("Approval does not exist"));
        }
        if approval.cancelled {
            return Err(eyre!("Approval already cancelled"));
        }
        if approval.executed {
            return Err(eyre!("Approval already executed"));
        }

        debug!(
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            "Submitting cancelWithdrawApproval"
        );

        // Submit cancel transaction
        let call = contract.cancelWithdrawApproval(FixedBytes::from(withdraw_hash));

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send cancel tx: {}", e))?;

        let tx_hash = *pending_tx.tx_hash();
        info!(tx_hash = %tx_hash, "Cancel transaction sent");

        // Wait for confirmation
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("Cancel transaction reverted"));
        }

        info!(
            tx_hash = %tx_hash,
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            "Approval successfully cancelled on EVM"
        );

        Ok(format!("0x{:x}", tx_hash))
    }

    /// Check if an approval can be cancelled (exists, not cancelled, not executed)
    pub async fn can_cancel(&self, withdraw_hash: [u8; 32]) -> Result<bool> {
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = CL8YBridge::new(self.bridge_address, &provider);

        let approval = contract
            .getWithdrawApproval(FixedBytes::from(withdraw_hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get approval: {}", e))?;

        Ok(approval.isApproved && !approval.cancelled && !approval.executed)
    }

    /// Get the canceler's address
    pub fn address(&self) -> Address {
        self.signer.address()
    }
}
