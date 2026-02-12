//! EVM RPC Client Wrapper
//!
//! Provides a high-level client for interacting with EVM chains via JSON-RPC.

use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use eyre::{eyre, Result};
use tracing::info;

/// EVM client configuration
#[derive(Debug, Clone)]
pub struct EvmClientConfig {
    /// RPC URL (e.g., "http://localhost:8545")
    pub rpc_url: String,
    /// Chain ID
    pub chain_id: u64,
    /// Private key for signing (optional, required for write operations)
    pub private_key: Option<String>,
}

/// Read-only EVM RPC client
pub struct EvmClientReadOnly {
    /// The alloy provider
    pub provider: RootProvider<Http<Client>>,
    /// Chain ID
    pub chain_id: u64,
}

impl EvmClientReadOnly {
    /// Create a new read-only EVM client
    pub async fn new(rpc_url: &str, chain_id: u64) -> Result<Self> {
        let provider = ProviderBuilder::new().on_http(
            rpc_url
                .parse()
                .map_err(|e| eyre!("Invalid RPC URL: {}", e))?,
        );

        info!(rpc_url = %rpc_url, chain_id = chain_id, "Created read-only EVM client");

        Ok(Self { provider, chain_id })
    }

    /// Get the current block number
    pub async fn get_block_number(&self) -> Result<u64> {
        let block = self.provider.get_block_number().await?;
        Ok(block)
    }

    /// Get the ETH balance of an address
    pub async fn get_balance(&self, address: Address) -> Result<U256> {
        let balance = self.provider.get_balance(address).await?;
        Ok(balance)
    }

    /// Get the chain ID from the RPC
    pub async fn get_chain_id(&self) -> Result<u64> {
        let chain_id = self.provider.get_chain_id().await?;
        Ok(chain_id)
    }
}

/// EVM RPC client with signing capabilities
pub struct EvmClientWithSigner {
    /// The alloy provider with wallet
    #[allow(clippy::type_complexity)]
    provider: alloy::providers::fillers::FillProvider<
        alloy::providers::fillers::JoinFill<
            alloy::providers::Identity,
            alloy::providers::fillers::WalletFiller<EthereumWallet>,
        >,
        RootProvider<Http<Client>>,
        Http<Client>,
        alloy::network::Ethereum,
    >,
    /// Chain ID
    pub chain_id: u64,
    /// Signer address
    pub signer_address: Address,
}

impl EvmClientWithSigner {
    /// Create a new EVM client with signing capabilities
    pub async fn new(rpc_url: &str, chain_id: u64, private_key: &str) -> Result<Self> {
        // Parse private key
        let signer: PrivateKeySigner = private_key
            .parse()
            .map_err(|e| eyre!("Invalid private key: {}", e))?;

        let address = signer.address();
        let wallet = EthereumWallet::from(signer);

        let provider = ProviderBuilder::new().wallet(wallet).on_http(
            rpc_url
                .parse()
                .map_err(|e| eyre!("Invalid RPC URL: {}", e))?,
        );

        info!(
            rpc_url = %rpc_url,
            chain_id = chain_id,
            address = %address,
            "Created EVM client with signer"
        );

        Ok(Self {
            provider,
            chain_id,
            signer_address: address,
        })
    }

    /// Get the current block number
    pub async fn get_block_number(&self) -> Result<u64> {
        let block = self.provider.get_block_number().await?;
        Ok(block)
    }

    /// Get the ETH balance of an address
    pub async fn get_balance(&self, address: Address) -> Result<U256> {
        let balance = self.provider.get_balance(address).await?;
        Ok(balance)
    }

    /// Get the chain ID from the RPC
    pub async fn get_chain_id(&self) -> Result<u64> {
        let chain_id = self.provider.get_chain_id().await?;
        Ok(chain_id)
    }

    /// Get the signer address
    pub fn get_signer_address(&self) -> Address {
        self.signer_address
    }
}

/// Unified EVM client that can be either read-only or with signer
pub enum EvmClient {
    ReadOnly(EvmClientReadOnly),
    WithSigner(EvmClientWithSigner),
}

impl EvmClient {
    /// Create a new read-only EVM client
    pub async fn new_readonly(rpc_url: &str, chain_id: u64) -> Result<Self> {
        Ok(EvmClient::ReadOnly(
            EvmClientReadOnly::new(rpc_url, chain_id).await?,
        ))
    }

    /// Create a new EVM client with signing capabilities
    pub async fn new_with_signer(rpc_url: &str, chain_id: u64, private_key: &str) -> Result<Self> {
        Ok(EvmClient::WithSigner(
            EvmClientWithSigner::new(rpc_url, chain_id, private_key).await?,
        ))
    }

    /// Get the current block number
    pub async fn get_block_number(&self) -> Result<u64> {
        match self {
            EvmClient::ReadOnly(c) => c.get_block_number().await,
            EvmClient::WithSigner(c) => c.get_block_number().await,
        }
    }

    /// Get the ETH balance of an address
    pub async fn get_balance(&self, address: Address) -> Result<U256> {
        match self {
            EvmClient::ReadOnly(c) => c.get_balance(address).await,
            EvmClient::WithSigner(c) => c.get_balance(address).await,
        }
    }

    /// Get the chain ID from the RPC
    pub async fn get_chain_id(&self) -> Result<u64> {
        match self {
            EvmClient::ReadOnly(c) => c.get_chain_id().await,
            EvmClient::WithSigner(c) => c.get_chain_id().await,
        }
    }

    /// Check if the client has a signer
    pub fn has_signer(&self) -> bool {
        matches!(self, EvmClient::WithSigner(_))
    }

    /// Get the signer address (None if read-only)
    pub fn get_signer_address(&self) -> Option<Address> {
        match self {
            EvmClient::ReadOnly(_) => None,
            EvmClient::WithSigner(c) => Some(c.signer_address),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = EvmClientConfig {
            rpc_url: "http://localhost:8545".to_string(),
            chain_id: 31337,
            private_key: None,
        };

        assert_eq!(config.chain_id, 31337);
        assert!(config.private_key.is_none());
    }
}
