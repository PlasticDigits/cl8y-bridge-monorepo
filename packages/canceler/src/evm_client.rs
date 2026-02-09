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
    /// Bridge contract interface for cancellation (V2)
    ///
    /// IMPORTANT: Function names must exactly match the Solidity contract:
    /// - `withdrawCancel` (NOT `cancelWithdrawApproval`)
    /// - `getPendingWithdraw` (NOT `getWithdrawApproval`)
    /// - `getCancelWindow` (NOT `withdrawDelay`)
    #[sol(rpc)]
    contract CL8YBridge {
        /// Cancel a pending withdrawal (V2: onlyCanceler)
        function withdrawCancel(bytes32 withdrawHash) external;

        /// Get pending withdrawal info (V2: PendingWithdraw struct)
        function getPendingWithdraw(bytes32 withdrawHash) external view returns (
            bytes4 srcChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            address recipient,
            uint256 amount,
            uint64 nonce,
            uint256 operatorGas,
            uint256 submittedAt,
            uint256 approvedAt,
            bool approved,
            bool cancelled,
            bool executed
        );

        /// Query the cancel window in seconds (V2)
        function getCancelWindow() external view returns (uint256);
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

        // First check if the withdrawal exists and is cancellable
        let pending = contract
            .getPendingWithdraw(FixedBytes::from(withdraw_hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to query pending withdrawal: {}", e))?;

        if pending.submittedAt.is_zero() {
            return Err(eyre!("Withdrawal does not exist (submittedAt=0)"));
        }
        if !pending.approved {
            return Err(eyre!("Withdrawal not yet approved"));
        }
        if pending.cancelled {
            return Err(eyre!("Withdrawal already cancelled"));
        }
        if pending.executed {
            return Err(eyre!("Withdrawal already executed"));
        }

        info!(
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            nonce = pending.nonce,
            amount = %pending.amount,
            approved_at = %pending.approvedAt,
            "Submitting withdrawCancel transaction"
        );

        // Submit cancel transaction
        let call = contract.withdrawCancel(FixedBytes::from(withdraw_hash));

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

    /// Check if a withdrawal can be cancelled (submitted, approved, not cancelled, not executed)
    pub async fn can_cancel(&self, withdraw_hash: [u8; 32]) -> Result<bool> {
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(self.rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = CL8YBridge::new(self.bridge_address, &provider);

        let pending = contract
            .getPendingWithdraw(FixedBytes::from(withdraw_hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to query pending withdrawal: {}", e))?;

        let cancellable = !pending.submittedAt.is_zero()
            && pending.approved
            && !pending.cancelled
            && !pending.executed;

        debug!(
            withdraw_hash = %bytes32_to_hex(&withdraw_hash),
            submitted_at = %pending.submittedAt,
            approved = pending.approved,
            cancelled = pending.cancelled,
            executed = pending.executed,
            cancellable = cancellable,
            "Checked cancellability of withdrawal"
        );

        Ok(cancellable)
    }

    /// Get the canceler's address
    pub fn address(&self) -> Address {
        self.signer.address()
    }
}
