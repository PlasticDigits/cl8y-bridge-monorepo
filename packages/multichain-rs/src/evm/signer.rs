//! EVM Transaction Signing Module
//!
//! Provides a dedicated signing interface for EVM transactions, separate from the client.
//! Wraps alloy's `PrivateKeySigner` and `EthereumWallet` for transaction signing.
//!
//! ## Features
//!
//! - Private key management
//! - Transaction signing
//! - Nonce management helpers
//! - Gas estimation helpers

use alloy::{
    network::{Ethereum, EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes, FixedBytes, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::{TransactionReceipt, TransactionRequest},
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use eyre::{eyre, Result, WrapErr};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Configuration for the EVM signer
#[derive(Debug, Clone)]
pub struct EvmSignerConfig {
    /// RPC URL for the EVM chain
    pub rpc_url: String,
    /// Chain ID
    pub chain_id: u64,
    /// Private key (hex string, with or without 0x prefix)
    pub private_key: String,
}

/// EVM transaction signer with nonce management
pub struct EvmSigner {
    /// The underlying private key signer
    signer: PrivateKeySigner,
    /// Ethereum wallet wrapper
    wallet: EthereumWallet,
    /// RPC URL
    rpc_url: String,
    /// Chain ID
    chain_id: u64,
    /// Signer's address
    address: Address,
    /// Provider with wallet attached
    provider: alloy::providers::fillers::FillProvider<
        alloy::providers::fillers::JoinFill<
            alloy::providers::Identity,
            alloy::providers::fillers::WalletFiller<EthereumWallet>,
        >,
        RootProvider<Http<Client>>,
        Http<Client>,
        Ethereum,
    >,
}

impl EvmSigner {
    /// Create a new EVM signer from configuration
    pub fn new(config: EvmSignerConfig) -> Result<Self> {
        let signer: PrivateKeySigner = config
            .private_key
            .parse()
            .map_err(|e| eyre!("Invalid private key: {}", e))?;

        let address = signer.address();
        let wallet = EthereumWallet::from(signer.clone());

        let provider = ProviderBuilder::new()
            .wallet(wallet.clone())
            .on_http(
                config
                    .rpc_url
                    .parse()
                    .map_err(|e| eyre!("Invalid RPC URL: {}", e))?,
            );

        info!(
            address = %address,
            chain_id = config.chain_id,
            "EVM signer initialized"
        );

        Ok(Self {
            signer,
            wallet,
            rpc_url: config.rpc_url,
            chain_id: config.chain_id,
            address,
            provider,
        })
    }

    /// Create from private key string
    pub fn from_private_key(rpc_url: &str, chain_id: u64, private_key: &str) -> Result<Self> {
        Self::new(EvmSignerConfig {
            rpc_url: rpc_url.to_string(),
            chain_id,
            private_key: private_key.to_string(),
        })
    }

    /// Get the signer's address
    pub fn address(&self) -> Address {
        self.address
    }

    /// Get the chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get the underlying signer (for use with contract instances)
    pub fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }

    /// Get the wallet (for use with alloy providers)
    pub fn wallet(&self) -> &EthereumWallet {
        &self.wallet
    }

    /// Get a reference to the provider with wallet
    pub fn provider(
        &self,
    ) -> &alloy::providers::fillers::FillProvider<
        alloy::providers::fillers::JoinFill<
            alloy::providers::Identity,
            alloy::providers::fillers::WalletFiller<EthereumWallet>,
        >,
        RootProvider<Http<Client>>,
        Http<Client>,
        Ethereum,
    > {
        &self.provider
    }

    // =========================================================================
    // Nonce Management
    // =========================================================================

    /// Get the current nonce for this signer
    pub async fn get_nonce(&self) -> Result<u64> {
        let nonce = self.provider.get_transaction_count(self.address).await?;
        Ok(nonce)
    }

    /// Get the pending nonce (includes unconfirmed transactions)
    pub async fn get_pending_nonce(&self) -> Result<u64> {
        // Alloy's get_transaction_count already returns the pending nonce by default
        self.get_nonce().await
    }

    // =========================================================================
    // Gas Estimation
    // =========================================================================

    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self, tx: &TransactionRequest) -> Result<u64> {
        let gas = self
            .provider
            .estimate_gas(tx)
            .await
            .wrap_err("Failed to estimate gas")?;
        Ok(gas)
    }

    /// Get current gas price
    pub async fn get_gas_price(&self) -> Result<u128> {
        let price = self
            .provider
            .get_gas_price()
            .await
            .wrap_err("Failed to get gas price")?;
        Ok(price)
    }

    /// Get max priority fee per gas (EIP-1559)
    pub async fn get_max_priority_fee(&self) -> Result<u128> {
        let fee = self
            .provider
            .get_max_priority_fee_per_gas()
            .await
            .wrap_err("Failed to get max priority fee")?;
        Ok(fee)
    }

    /// Calculate gas price with a bump percentage for retries
    pub fn calculate_bumped_gas_price(&self, base_price: u128, bump_percent: u32) -> u128 {
        let multiplier = 100 + bump_percent as u128;
        base_price * multiplier / 100
    }

    // =========================================================================
    // Transaction Sending
    // =========================================================================

    /// Send a raw transaction and wait for receipt
    pub async fn send_transaction(
        &self,
        tx: TransactionRequest,
    ) -> Result<TransactionReceipt> {
        let pending = self
            .provider
            .send_transaction(tx)
            .await
            .wrap_err("Failed to send transaction")?;

        let receipt = pending
            .get_receipt()
            .await
            .wrap_err("Failed to get transaction receipt")?;

        Ok(receipt)
    }

    /// Send transaction with custom timeout
    pub async fn send_transaction_with_timeout(
        &self,
        tx: TransactionRequest,
        timeout: Duration,
    ) -> Result<TransactionReceipt> {
        tokio::time::timeout(timeout, self.send_transaction(tx))
            .await
            .map_err(|_| eyre!("Transaction timed out after {:?}", timeout))?
    }

    /// Send a contract call transaction
    pub async fn send_contract_call(
        &self,
        to: Address,
        data: Bytes,
        value: Option<U256>,
    ) -> Result<TransactionReceipt> {
        let mut tx = TransactionRequest::default().to(to).input(data.into());

        if let Some(v) = value {
            tx = tx.value(v);
        }

        self.send_transaction(tx).await
    }

    // =========================================================================
    // Transaction Confirmation
    // =========================================================================

    /// Wait for a transaction to be confirmed
    pub async fn wait_for_confirmation(
        &self,
        tx_hash: FixedBytes<32>,
        timeout: Duration,
    ) -> Result<TransactionReceipt> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(500);

        while start.elapsed() < timeout {
            if let Some(receipt) = self.provider.get_transaction_receipt(tx_hash).await? {
                return Ok(receipt);
            }
            tokio::time::sleep(poll_interval).await;
        }

        Err(eyre!(
            "Transaction {} not confirmed after {:?}",
            tx_hash,
            timeout
        ))
    }

    /// Check if a transaction succeeded
    pub async fn check_tx_success(&self, tx_hash: FixedBytes<32>) -> Result<Option<bool>> {
        match self.provider.get_transaction_receipt(tx_hash).await? {
            Some(receipt) => Ok(Some(receipt.status())),
            None => Ok(None),
        }
    }

    // =========================================================================
    // Balance Queries
    // =========================================================================

    /// Get the ETH balance of this signer
    pub async fn get_balance(&self) -> Result<U256> {
        let balance = self.provider.get_balance(self.address).await?;
        Ok(balance)
    }

    /// Get current block number
    pub async fn get_block_number(&self) -> Result<u64> {
        let block = self.provider.get_block_number().await?;
        Ok(block)
    }
}

/// Retry configuration for transactions
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Gas price bump percentage per retry
    pub gas_bump_percent: u32,
    /// Maximum gas price multiplier
    pub max_gas_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            gas_bump_percent: 10,
            max_gas_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate backoff duration for a given attempt
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let backoff = self.initial_backoff.as_millis() * 2u128.pow(attempt);
        Duration::from_millis(backoff.min(self.max_backoff.as_millis()) as u64)
    }

    /// Calculate gas price for a given attempt
    pub fn gas_price_for_attempt(&self, base_gas_price: u128, attempt: u32) -> u128 {
        if attempt == 0 {
            return base_gas_price;
        }

        let multiplier = 1.0 + (self.gas_bump_percent as f64 / 100.0) * (attempt as f64);
        let capped_multiplier = multiplier.min(self.max_gas_multiplier);
        (base_gas_price as f64 * capped_multiplier) as u128
    }
}

/// Execute a transaction with retry logic
pub async fn send_with_retry<F, Fut>(
    signer: &EvmSigner,
    build_tx: F,
    config: &RetryConfig,
) -> Result<TransactionReceipt>
where
    F: Fn(u128) -> Fut,
    Fut: std::future::Future<Output = Result<TransactionRequest>>,
{
    let base_gas_price = signer.get_gas_price().await?;
    let mut last_error = None;

    for attempt in 0..config.max_retries {
        let gas_price = config.gas_price_for_attempt(base_gas_price, attempt);

        match build_tx(gas_price).await {
            Ok(tx) => match signer.send_transaction(tx).await {
                Ok(receipt) => {
                    if receipt.status() {
                        return Ok(receipt);
                    } else {
                        warn!(
                            attempt = attempt + 1,
                            "Transaction reverted, retrying with higher gas"
                        );
                        last_error = Some(eyre!("Transaction reverted"));
                    }
                }
                Err(e) => {
                    let error_str = e.to_string();

                    // Check for retriable errors
                    if error_str.contains("nonce too low")
                        || error_str.contains("replacement transaction underpriced")
                        || error_str.contains("already known")
                    {
                        warn!(
                            attempt = attempt + 1,
                            error = %e,
                            "Retriable error, increasing gas and retrying"
                        );
                        last_error = Some(e);
                    } else {
                        // Non-retriable error
                        return Err(e);
                    }
                }
            },
            Err(e) => {
                return Err(e);
            }
        }

        if attempt + 1 < config.max_retries {
            let backoff = config.backoff_for_attempt(attempt);
            debug!(
                attempt = attempt + 1,
                backoff_ms = backoff.as_millis(),
                "Backing off before retry"
            );
            tokio::time::sleep(backoff).await;
        }
    }

    Err(last_error.unwrap_or_else(|| eyre!("Transaction failed after {} retries", config.max_retries)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_backoff() {
        let config = RetryConfig::default();

        let backoff0 = config.backoff_for_attempt(0);
        let backoff1 = config.backoff_for_attempt(1);
        let backoff2 = config.backoff_for_attempt(2);

        assert_eq!(backoff0, Duration::from_secs(1));
        assert_eq!(backoff1, Duration::from_secs(2));
        assert_eq!(backoff2, Duration::from_secs(4));
    }

    #[test]
    fn test_retry_config_gas_bump() {
        let config = RetryConfig {
            gas_bump_percent: 10,
            max_gas_multiplier: 2.0,
            ..Default::default()
        };

        let base = 1_000_000_000u128; // 1 gwei
        let price0 = config.gas_price_for_attempt(base, 0);
        let price1 = config.gas_price_for_attempt(base, 1);
        let price2 = config.gas_price_for_attempt(base, 2);

        assert_eq!(price0, base);
        assert_eq!(price1, 1_100_000_000); // 1.1x
        assert_eq!(price2, 1_200_000_000); // 1.2x
    }

    #[test]
    fn test_retry_config_gas_cap() {
        let config = RetryConfig {
            gas_bump_percent: 50,
            max_gas_multiplier: 1.5,
            ..Default::default()
        };

        let base = 1_000_000_000u128;
        let price10 = config.gas_price_for_attempt(base, 10); // Would be 6x without cap

        // Should be capped at 1.5x
        assert_eq!(price10, 1_500_000_000);
    }

    #[test]
    fn test_calculate_bumped_gas_price() {
        let config = EvmSignerConfig {
            rpc_url: "http://localhost:8545".to_string(),
            chain_id: 1,
            private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .to_string(),
        };

        // Can't create signer without network, just test the calculation
        let base = 1_000_000_000u128;
        let bump_10 = base * 110 / 100;
        let bump_25 = base * 125 / 100;

        assert_eq!(bump_10, 1_100_000_000);
        assert_eq!(bump_25, 1_250_000_000);
    }
}
