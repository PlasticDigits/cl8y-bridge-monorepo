//! EVM Query Helpers
//!
//! Provides typed convenience functions for querying EVM bridge contracts,
//! chain registry, token registry, and on-chain state.
//!
//! Mirrors the Terra `queries.rs` module for symmetric API coverage.

use alloy::{
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    transports::http::{Client, Http},
};
use eyre::{eyre, Result};

use crate::evm::contracts::{Bridge, ChainRegistry, LockUnlock, TokenRegistry, ERC20};
use crate::types::ChainId;

/// EVM bridge query client
///
/// Provides typed query methods for the EVM bridge and registry contracts.
/// Uses a read-only provider (no signer needed).
pub struct EvmQueryClient {
    /// Read-only provider
    provider: RootProvider<Http<Client>>,
    /// Bridge contract address
    bridge_address: Address,
    /// Chain ID (retained for context)
    #[allow(dead_code)]
    chain_id: u64,
}

impl EvmQueryClient {
    /// Create a new query client
    pub fn new(rpc_url: &str, bridge_address: Address, chain_id: u64) -> Result<Self> {
        let provider = ProviderBuilder::new().on_http(
            rpc_url
                .parse()
                .map_err(|e| eyre!("Invalid RPC URL: {}", e))?,
        );

        Ok(Self {
            provider,
            bridge_address,
            chain_id,
        })
    }

    /// Get the underlying provider
    pub fn provider(&self) -> &RootProvider<Http<Client>> {
        &self.provider
    }

    // =========================================================================
    // Bridge Contract Queries
    // =========================================================================

    /// Get the ChainRegistry contract address from the bridge
    pub async fn get_chain_registry_address(&self) -> Result<Address> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .chainRegistry()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get chain registry address: {}", e))?;

        Ok(result._0)
    }

    /// Get this chain's registered 4-byte chain ID
    pub async fn get_this_chain_id(&self) -> Result<ChainId> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .getThisChainId()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get chain ID: {}", e))?;

        Ok(ChainId::from_bytes(result._0.0))
    }

    /// Get the cancel window duration in seconds
    pub async fn get_cancel_window(&self) -> Result<u64> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .getCancelWindow()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get cancel window: {}", e))?;

        Ok(result._0.try_into().unwrap_or(u64::MAX))
    }

    /// Get the current deposit nonce
    pub async fn get_deposit_nonce(&self) -> Result<u64> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .getDepositNonce()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get deposit nonce: {}", e))?;

        Ok(result._0)
    }

    /// Check if an address is an operator
    pub async fn is_operator(&self, account: Address) -> Result<bool> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .isOperator(account)
            .call()
            .await
            .map_err(|e| eyre!("Failed to check operator: {}", e))?;

        Ok(result._0)
    }

    /// Check if an address is a canceler
    pub async fn is_canceler(&self, account: Address) -> Result<bool> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .isCanceler(account)
            .call()
            .await
            .map_err(|e| eyre!("Failed to check canceler: {}", e))?;

        Ok(result._0)
    }

    /// Get pending withdrawal info
    pub async fn get_pending_withdraw(
        &self,
        withdraw_hash: [u8; 32],
    ) -> Result<PendingWithdrawInfo> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .getPendingWithdraw(FixedBytes(withdraw_hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get pending withdraw: {}", e))?;

        Ok(PendingWithdrawInfo {
            src_chain: ChainId::from_bytes(result.srcChain.0),
            src_account: result.srcAccount.0,
            token: result.token,
            recipient: result.recipient,
            amount: result.amount,
            nonce: result.nonce,
            src_decimals: result.srcDecimals,
            dest_decimals: result.destDecimals,
            operator_gas: result.operatorGas,
            submitted_at: result.submittedAt,
            approved_at: result.approvedAt,
            approved: result.approved,
            cancelled: result.cancelled,
            executed: result.executed,
        })
    }

    /// Calculate fee for a deposit amount
    pub async fn calculate_fee(&self, depositor: Address, amount: U256) -> Result<U256> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .calculateFee(depositor, amount)
            .call()
            .await
            .map_err(|e| eyre!("Failed to calculate fee: {}", e))?;

        Ok(result.feeAmount)
    }

    /// Get fee info for an account (bps + type)
    pub async fn get_account_fee(&self, account: Address) -> Result<(u64, String)> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .getAccountFee(account)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get account fee: {}", e))?;

        let bps: u64 = result.feeBps.try_into().unwrap_or(u64::MAX);
        Ok((bps, result.feeType))
    }

    /// Check if account has a custom fee
    pub async fn has_custom_fee(&self, account: Address) -> Result<bool> {
        let bridge = Bridge::new(self.bridge_address, &self.provider);
        let result = bridge
            .hasCustomFee(account)
            .call()
            .await
            .map_err(|e| eyre!("Failed to check custom fee: {}", e))?;

        Ok(result.hasCustom)
    }

    // =========================================================================
    // Chain Registry Queries
    // =========================================================================

    /// Query the chain registry contract
    pub async fn is_chain_registered(
        &self,
        registry_address: Address,
        chain_id: ChainId,
    ) -> Result<bool> {
        let registry = ChainRegistry::new(registry_address, &self.provider);
        let result = registry
            .isChainRegistered(FixedBytes(*chain_id.as_bytes()))
            .call()
            .await
            .map_err(|e| eyre!("Failed to check chain registration: {}", e))?;

        Ok(result.registered)
    }

    /// Get all registered chain IDs
    pub async fn get_registered_chains(&self, registry_address: Address) -> Result<Vec<ChainId>> {
        let registry = ChainRegistry::new(registry_address, &self.provider);
        let result = registry
            .getRegisteredChains()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get registered chains: {}", e))?;

        Ok(result
            .chainIds
            .iter()
            .map(|id| ChainId::from_bytes(id.0))
            .collect())
    }

    /// Get the chain hash for a chain ID
    pub async fn get_chain_hash(
        &self,
        registry_address: Address,
        chain_id: ChainId,
    ) -> Result<[u8; 32]> {
        let registry = ChainRegistry::new(registry_address, &self.provider);
        let result = registry
            .getChainHash(FixedBytes(*chain_id.as_bytes()))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get chain hash: {}", e))?;

        Ok(result.hash.0)
    }

    /// Get chain count
    pub async fn get_chain_count(&self, registry_address: Address) -> Result<u64> {
        let registry = ChainRegistry::new(registry_address, &self.provider);
        let result = registry
            .getChainCount()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get chain count: {}", e))?;

        Ok(result.count.try_into().unwrap_or(u64::MAX))
    }

    /// Compute the identifier hash (pure function)
    /// Used to look up chain ID from identifier strings like "terraclassic_columbus-5"
    pub async fn compute_identifier_hash(
        &self,
        registry_address: Address,
        identifier: &str,
    ) -> Result<[u8; 32]> {
        let registry = ChainRegistry::new(registry_address, &self.provider);
        let result = registry
            .computeIdentifierHash(identifier.to_string())
            .call()
            .await
            .map_err(|e| eyre!("Failed to compute identifier hash: {}", e))?;

        Ok(result.hash.0)
    }

    /// Get chain ID from its identifier hash
    pub async fn get_chain_id_from_hash(
        &self,
        registry_address: Address,
        hash: [u8; 32],
    ) -> Result<ChainId> {
        let registry = ChainRegistry::new(registry_address, &self.provider);
        let result = registry
            .getChainIdFromHash(FixedBytes(hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get chain ID from hash: {}", e))?;

        Ok(ChainId::from_bytes(result.chainId.0))
    }

    // =========================================================================
    // Token Registry Queries
    // =========================================================================

    /// Check if a token is registered
    pub async fn is_token_registered(
        &self,
        registry_address: Address,
        token: Address,
    ) -> Result<bool> {
        let registry = TokenRegistry::new(registry_address, &self.provider);
        let result = registry
            .isTokenRegistered(token)
            .call()
            .await
            .map_err(|e| eyre!("Failed to check token registration: {}", e))?;

        Ok(result.registered)
    }

    /// Get the token type (LockUnlock = 0, MintBurn = 1)
    pub async fn get_token_type(&self, registry_address: Address, token: Address) -> Result<u8> {
        let registry = TokenRegistry::new(registry_address, &self.provider);
        let result = registry
            .getTokenType(token)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get token type: {}", e))?;

        Ok(result.tokenType)
    }

    /// Get destination token for a chain
    pub async fn get_dest_token(
        &self,
        registry_address: Address,
        token: Address,
        dest_chain: ChainId,
    ) -> Result<[u8; 32]> {
        let registry = TokenRegistry::new(registry_address, &self.provider);
        let result = registry
            .getDestToken(token, FixedBytes(*dest_chain.as_bytes()))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get dest token: {}", e))?;

        Ok(result.destToken.0)
    }

    /// Get all registered tokens
    pub async fn get_all_tokens(&self, registry_address: Address) -> Result<Vec<Address>> {
        let registry = TokenRegistry::new(registry_address, &self.provider);
        let result = registry
            .getAllTokens()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get all tokens: {}", e))?;

        Ok(result.tokens)
    }

    /// Get token count
    pub async fn get_token_count(&self, registry_address: Address) -> Result<u64> {
        let registry = TokenRegistry::new(registry_address, &self.provider);
        let result = registry
            .getTokenCount()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get token count: {}", e))?;

        Ok(result.count.try_into().unwrap_or(u64::MAX))
    }

    // =========================================================================
    // LockUnlock Queries
    // =========================================================================

    /// Get locked balance for a token
    pub async fn get_locked_balance(
        &self,
        lock_unlock_address: Address,
        token: Address,
    ) -> Result<U256> {
        let contract = LockUnlock::new(lock_unlock_address, &self.provider);
        let result = contract
            .getLockedBalance(token)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get locked balance: {}", e))?;

        Ok(result.balance)
    }

    // =========================================================================
    // Balance Queries
    // =========================================================================

    /// Get ETH balance for an address
    pub async fn get_eth_balance(&self, address: Address) -> Result<U256> {
        let balance = self.provider.get_balance(address).await?;
        Ok(balance)
    }

    /// Get ERC20 token balance
    pub async fn get_erc20_balance(
        &self,
        token_address: Address,
        account: Address,
    ) -> Result<U256> {
        let contract = ERC20::new(token_address, &self.provider);
        let result = contract
            .balanceOf(account)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get ERC20 balance: {}", e))?;

        Ok(result._0)
    }

    /// Get ERC20 allowance
    pub async fn get_erc20_allowance(
        &self,
        token_address: Address,
        owner: Address,
        spender: Address,
    ) -> Result<U256> {
        let contract = ERC20::new(token_address, &self.provider);
        let result = contract
            .allowance(owner, spender)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get allowance: {}", e))?;

        Ok(result._0)
    }

    /// Get current block number
    pub async fn get_block_number(&self) -> Result<u64> {
        let block = self.provider.get_block_number().await?;
        Ok(block)
    }

    // =========================================================================
    // Transaction Queries
    // =========================================================================

    /// Get a transaction receipt by hash
    pub async fn get_tx_receipt(
        &self,
        tx_hash: alloy::primitives::FixedBytes<32>,
    ) -> Result<Option<alloy::rpc::types::TransactionReceipt>> {
        let receipt = self.provider.get_transaction_receipt(tx_hash).await?;
        Ok(receipt)
    }

    /// Check if a transaction succeeded
    pub async fn check_tx_success(
        &self,
        tx_hash: alloy::primitives::FixedBytes<32>,
    ) -> Result<Option<bool>> {
        match self.provider.get_transaction_receipt(tx_hash).await? {
            Some(receipt) => Ok(Some(receipt.status())),
            None => Ok(None),
        }
    }

    /// Get transaction logs for a specific transaction
    pub async fn get_tx_logs(
        &self,
        tx_hash: alloy::primitives::FixedBytes<32>,
    ) -> Result<Vec<alloy::rpc::types::Log>> {
        match self.provider.get_transaction_receipt(tx_hash).await? {
            Some(receipt) => Ok(receipt.inner.logs().to_vec()),
            None => Ok(Vec::new()),
        }
    }

    // =========================================================================
    // Fee Parameter Queries
    // =========================================================================

    /// Get the fee BPS and type for an account (returns (bps, fee_type_string))
    pub async fn get_fee_info(&self, account: Address) -> Result<(u64, String)> {
        self.get_account_fee(account).await
    }
}

/// Pending withdrawal information from the bridge contract
#[derive(Debug, Clone)]
pub struct PendingWithdrawInfo {
    pub src_chain: ChainId,
    pub src_account: [u8; 32],
    pub token: Address,
    pub recipient: Address,
    pub amount: U256,
    pub nonce: u64,
    pub src_decimals: u8,
    pub dest_decimals: u8,
    pub operator_gas: U256,
    pub submitted_at: U256,
    pub approved_at: U256,
    pub approved: bool,
    pub cancelled: bool,
    pub executed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_withdraw_info() {
        let info = PendingWithdrawInfo {
            src_chain: ChainId::from_u32(1),
            src_account: [0u8; 32],
            token: Address::ZERO,
            recipient: Address::ZERO,
            amount: U256::from(1_000_000u64),
            nonce: 1,
            src_decimals: 18,
            dest_decimals: 18,
            operator_gas: U256::ZERO,
            submitted_at: U256::ZERO,
            approved_at: U256::ZERO,
            approved: false,
            cancelled: false,
            executed: false,
        };

        assert_eq!(info.src_chain.to_u32(), 1);
        assert!(!info.approved);
    }
}
