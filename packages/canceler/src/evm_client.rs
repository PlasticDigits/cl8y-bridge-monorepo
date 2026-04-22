//! EVM Client for canceler transactions
//!
//! Handles signing and submitting CancelWithdrawApproval transactions to EVM chains.
//!
//! # Transaction Building
//!
//! Uses Alloy's `ProviderBuilder::with_recommended_fillers()` for nonce and gas limit.
//! Cancel txs set an explicit legacy `gas_price` from `eth_gasPrice`, floored per **native EVM
//! chain id** (e.g. BSC **0.1 gwei**, opBNB **0.00001 gwei**) so relays and bad fee quotes do not
//! reject submissions.
//!
//! **RPC failover:** tries each URL from `EVM_RPC_URL` (comma-separated, same as operator) until
//! reads and `withdrawCancel` succeed.

#![allow(dead_code)]

use alloy::network::EthereumWallet;
use alloy::primitives::{Address, FixedBytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use eyre::{eyre, Result, WrapErr};
use multichain_rs::{evm_consensus_latest_block, EvmRpcReadPolicy};
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::hash::bytes32_to_hex;

/// BSC mainnet (`chain_id` 56): floor **0.1 gwei** (1e8 wei).
const BSC_MAINNET_CHAIN_ID: u64 = 56;
const MIN_CANCEL_GAS_PRICE_WEI_BSC: u128 = 100_000_000;

/// opBNB mainnet (`chain_id` 204): floor **0.00001 gwei** (= 1e4 wei).
const OPBNB_MAINNET_CHAIN_ID: u64 = 204;
const MIN_CANCEL_GAS_PRICE_WEI_OPBNB: u128 = 10_000;

/// Fallback floor for EVM chains without a specific rule (conservative).
const MIN_CANCEL_GAS_PRICE_WEI_DEFAULT: u128 = MIN_CANCEL_GAS_PRICE_WEI_BSC;

/// Minimum gas price (wei) applied after `eth_gasPrice` for cancel txs on this chain.
#[inline]
fn min_cancel_gas_price_floor_wei(evm_chain_id: u64) -> u128 {
    match evm_chain_id {
        BSC_MAINNET_CHAIN_ID => MIN_CANCEL_GAS_PRICE_WEI_BSC,
        OPBNB_MAINNET_CHAIN_ID => MIN_CANCEL_GAS_PRICE_WEI_OPBNB,
        _ => MIN_CANCEL_GAS_PRICE_WEI_DEFAULT,
    }
}

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
        function withdrawCancel(bytes32 xchainHashId) external;

        /// Get pending withdrawal info (V2: PendingWithdraw struct — must match Bridge.sol / watcher)
        function getPendingWithdraw(bytes32 xchainHashId) external view returns (
            bytes4 srcChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            address recipient,
            uint256 amount,
            uint64 nonce,
            uint8 srcDecimals,
            uint8 destDecimals,
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
    rpc_urls: Vec<String>,
    bridge_address: Address,
    signer: PrivateKeySigner,
    /// Native EVM chain id (`eth_chainId`) — selects cancel-tx gas price floor.
    evm_chain_id: u64,
}

impl EvmClient {
    /// Resolve gas price for a cancel tx on `rpc_url`: `max(eth_gasPrice, chain floor)`, or the floor if `eth_gasPrice` errors.
    async fn resolve_cancel_gas_price_for_url(&self, rpc_url: &str) -> Result<u128> {
        let floor = min_cancel_gas_price_floor_wei(self.evm_chain_id);
        let url = rpc_url.parse().wrap_err("Invalid RPC URL")?;
        let provider = ProviderBuilder::new().on_http(url);

        match provider.get_gas_price().await {
            Ok(quoted) => {
                let effective = quoted.max(floor);
                if effective > quoted {
                    debug!(
                        evm_chain_id = self.evm_chain_id,
                        quoted_wei = quoted,
                        effective_wei = effective,
                        floor_wei = floor,
                        "Raised cancel tx gas price to per-chain floor"
                    );
                } else {
                    debug!(
                        evm_chain_id = self.evm_chain_id,
                        gas_price_wei = effective,
                        "Using network gas price for cancel tx (≥ floor)"
                    );
                }
                Ok(effective)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    evm_chain_id = self.evm_chain_id,
                    gas_price_wei = floor,
                    "eth_gasPrice failed; using per-chain floor for cancel tx"
                );
                Ok(floor)
            }
        }
    }

    /// `rpc_urls` — non-empty; first is primary (same order as comma-separated `EVM_RPC_URL`).
    pub fn new(
        rpc_urls: Vec<String>,
        bridge_address: &str,
        private_key: &str,
        evm_chain_id: u64,
    ) -> Result<Self> {
        if rpc_urls.is_empty() {
            return Err(eyre!("at least one EVM RPC URL is required"));
        }
        let bridge_address =
            Address::from_str(bridge_address).wrap_err("Invalid bridge address")?;
        let signer: PrivateKeySigner = private_key.parse().wrap_err("Invalid private key")?;

        info!(
            canceler_address = %signer.address(),
            bridge = %bridge_address,
            evm_chain_id,
            rpc_endpoint_count = rpc_urls.len(),
            min_cancel_gas_floor_wei = min_cancel_gas_price_floor_wei(evm_chain_id),
            "EVM client initialized"
        );

        Ok(Self {
            rpc_urls,
            bridge_address,
            signer,
            evm_chain_id,
        })
    }

    async fn try_cancel_on_url(&self, rpc_url: &str, xchain_hash_id: [u8; 32]) -> Result<String> {
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = CL8YBridge::new(self.bridge_address, &provider);

        let pending = contract
            .getPendingWithdraw(FixedBytes::from(xchain_hash_id))
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

        let gas_price_wei = self.resolve_cancel_gas_price_for_url(rpc_url).await?;

        info!(
            xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
            nonce = pending.nonce,
            amount = %pending.amount,
            approved_at = %pending.approvedAt,
            gas_price_wei,
            rpc_url = %rpc_url,
            "Submitting withdrawCancel transaction"
        );

        let call = contract
            .withdrawCancel(FixedBytes::from(xchain_hash_id))
            .gas_price(gas_price_wei);

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send cancel tx: {}", e))?;

        let tx_hash = *pending_tx.tx_hash();
        info!(tx_hash = %tx_hash, "Cancel transaction sent");

        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("Cancel transaction reverted"));
        }

        info!(
            tx_hash = %tx_hash,
            xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
            "Approval successfully cancelled on EVM"
        );

        Ok(format!("0x{:x}", tx_hash))
    }

    /// Cancel a withdraw approval on EVM (consensus primary first, then remaining URLs for broadcast resilience).
    pub async fn cancel_withdraw_approval(&self, xchain_hash_id: [u8; 32]) -> Result<String> {
        let policy = EvmRpcReadPolicy::from_env_for_url_count(self.rpc_urls.len())?;
        let head = evm_consensus_latest_block(&self.rpc_urls, &policy).await?;
        let mut order: Vec<usize> = Vec::with_capacity(self.rpc_urls.len());
        order.push(head.provider_index);
        for i in 0..self.rpc_urls.len() {
            if i != head.provider_index {
                order.push(i);
            }
        }
        let mut last_err: Option<eyre::Report> = None;
        for i in order {
            match self
                .try_cancel_on_url(&self.rpc_urls[i], xchain_hash_id)
                .await
            {
                Ok(v) => return Ok(v),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.expect("non-empty rpc urls"))
    }

    async fn try_can_cancel_on_url(&self, rpc_url: &str, xchain_hash_id: [u8; 32]) -> Result<bool> {
        let wallet = EthereumWallet::from(self.signer.clone());
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(rpc_url.parse().wrap_err("Invalid RPC URL")?);

        let contract = CL8YBridge::new(self.bridge_address, &provider);

        let pending = contract
            .getPendingWithdraw(FixedBytes::from(xchain_hash_id))
            .call()
            .await
            .map_err(|e| eyre!("Failed to query pending withdrawal: {}", e))?;

        let cancellable = !pending.submittedAt.is_zero()
            && pending.approved
            && !pending.cancelled
            && !pending.executed;

        debug!(
            xchain_hash_id = %bytes32_to_hex(&xchain_hash_id),
            submitted_at = %pending.submittedAt,
            approved = pending.approved,
            cancelled = pending.cancelled,
            executed = pending.executed,
            cancellable = cancellable,
            rpc_url = %rpc_url,
            "Checked cancellability of withdrawal"
        );

        Ok(cancellable)
    }

    /// Check if a withdrawal can be cancelled (uses RPC head quorum — same truth as polling).
    pub async fn can_cancel(&self, xchain_hash_id: [u8; 32]) -> Result<bool> {
        let policy = EvmRpcReadPolicy::from_env_for_url_count(self.rpc_urls.len())?;
        let head = evm_consensus_latest_block(&self.rpc_urls, &policy).await?;
        let url = self.rpc_urls[head.provider_index].clone();
        self.try_can_cancel_on_url(&url, xchain_hash_id).await
    }

    /// Get the canceler's address
    pub fn address(&self) -> Address {
        self.signer.address()
    }
}
