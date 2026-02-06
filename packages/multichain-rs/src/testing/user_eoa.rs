//! EVM User EOA (Externally Owned Account) Helpers
//!
//! Utilities for simulating EVM user accounts in E2E tests, including
//! deposit, withdraw, and balance checking operations.
//!
//! ## EvmUser Operations
//!
//! - `deposit_native()` - Deposit native ETH to the bridge
//! - `deposit_erc20()` - Deposit ERC20 tokens to the bridge
//! - `withdraw_submit()` - Submit a withdrawal on the destination chain
//! - `get_eth_balance()` - Get native ETH balance
//! - `get_erc20_balance()` - Get ERC20 token balance

use alloy::primitives::Address;
use eyre::{eyre, Result};

#[cfg(feature = "evm")]
use alloy::{
    primitives::{FixedBytes, U256},
    providers::Provider,
};

// ============================================================================
// EVM User
// ============================================================================

/// Represents an EVM user account for testing
#[derive(Debug, Clone)]
pub struct EvmUser {
    /// Private key (hex string with 0x prefix)
    pub private_key: String,
    /// Address
    pub address: Address,
}

impl EvmUser {
    /// Create from private key hex string
    pub fn from_private_key(private_key: &str) -> Result<Self> {
        let pk = private_key.strip_prefix("0x").unwrap_or(private_key);
        let pk_bytes = hex::decode(pk)?;
        if pk_bytes.len() != 32 {
            return Err(eyre!("Private key must be 32 bytes"));
        }

        let signer: alloy::signers::local::PrivateKeySigner = private_key.parse()?;
        let address = signer.address();

        Ok(Self {
            private_key: format!("0x{}", pk),
            address,
        })
    }

    /// Get address as hex string
    pub fn address_hex(&self) -> String {
        format!("{:?}", self.address)
    }

    /// Get address as bytes32 (left-padded with zeros)
    pub fn address_bytes32(&self) -> [u8; 32] {
        evm_address_to_bytes32(&self.address)
    }
}

#[cfg(feature = "evm")]
impl EvmUser {
    /// Create a signer for this user
    pub fn create_signer(
        &self,
        rpc_url: &str,
        chain_id: u64,
    ) -> Result<crate::evm::signer::EvmSigner> {
        crate::evm::signer::EvmSigner::from_private_key(rpc_url, chain_id, &self.private_key)
    }

    // =========================================================================
    // Deposit Operations
    // =========================================================================

    /// Deposit native ETH to the bridge contract
    ///
    /// Calls `bridge.depositNative(destChain, destAccount)` with `msg.value = amount`
    pub async fn deposit_native(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        dest_chain: [u8; 4],
        dest_account: [u8; 32],
        amount: U256,
    ) -> Result<FixedBytes<32>> {
        use crate::evm::contracts::Bridge;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let contract = Bridge::new(bridge_address, signer.provider());

        let call = contract
            .depositNative(FixedBytes(dest_chain), FixedBytes(dest_account))
            .value(amount);

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send depositNative: {}", e))?;
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("depositNative transaction reverted"));
        }

        Ok(receipt.transaction_hash)
    }

    /// Deposit ERC20 tokens to the bridge contract (lock/unlock mode)
    ///
    /// Calls `erc20.approve(bridge, amount)` then `bridge.depositERC20(token, amount, destChain, destAccount)`
    pub async fn deposit_erc20(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        token_address: Address,
        amount: U256,
        dest_chain: [u8; 4],
        dest_account: [u8; 32],
    ) -> Result<FixedBytes<32>> {
        use crate::evm::contracts::{Bridge, ERC20};

        let signer = self.create_signer(rpc_url, chain_id)?;

        // Step 1: Approve bridge to spend tokens
        let erc20 = ERC20::new(token_address, signer.provider());
        let approve_call = erc20.approve(bridge_address, amount);
        let approve_tx = approve_call
            .send()
            .await
            .map_err(|e| eyre!("Failed to approve: {}", e))?;
        let approve_receipt = approve_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get approve receipt: {}", e))?;

        if !approve_receipt.status() {
            return Err(eyre!("ERC20 approve transaction reverted"));
        }

        // Step 2: Deposit tokens
        let bridge = Bridge::new(bridge_address, signer.provider());
        let deposit_call = bridge.depositERC20(
            token_address,
            amount,
            FixedBytes(dest_chain),
            FixedBytes(dest_account),
        );

        let pending_tx = deposit_call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send depositERC20: {}", e))?;
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("depositERC20 transaction reverted"));
        }

        Ok(receipt.transaction_hash)
    }

    /// Deposit mintable ERC20 tokens to the bridge contract (mint/burn mode)
    pub async fn deposit_erc20_mintable(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        token_address: Address,
        amount: U256,
        dest_chain: [u8; 4],
        dest_account: [u8; 32],
    ) -> Result<FixedBytes<32>> {
        use crate::evm::contracts::{Bridge, ERC20};

        let signer = self.create_signer(rpc_url, chain_id)?;

        // Step 1: Approve bridge to spend tokens
        let erc20 = ERC20::new(token_address, signer.provider());
        let approve_call = erc20.approve(bridge_address, amount);
        let approve_tx = approve_call
            .send()
            .await
            .map_err(|e| eyre!("Failed to approve: {}", e))?;
        let approve_receipt = approve_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get approve receipt: {}", e))?;

        if !approve_receipt.status() {
            return Err(eyre!("ERC20 approve transaction reverted"));
        }

        // Step 2: Deposit mintable tokens
        let bridge = Bridge::new(bridge_address, signer.provider());
        let deposit_call = bridge.depositERC20Mintable(
            token_address,
            amount,
            FixedBytes(dest_chain),
            FixedBytes(dest_account),
        );

        let pending_tx = deposit_call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send depositERC20Mintable: {}", e))?;
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("depositERC20Mintable transaction reverted"));
        }

        Ok(receipt.transaction_hash)
    }

    // =========================================================================
    // Withdrawal Operations
    // =========================================================================

    /// Submit a withdrawal on the destination EVM chain
    ///
    /// Calls `bridge.withdrawSubmit(srcChain, token, amount, nonce)` with operator gas
    pub async fn withdraw_submit(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        src_chain: [u8; 4],
        token_address: Address,
        amount: U256,
        nonce: u64,
        operator_gas: U256,
    ) -> Result<FixedBytes<32>> {
        use crate::evm::contracts::Bridge;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let bridge = Bridge::new(bridge_address, signer.provider());

        let call = bridge
            .withdrawSubmit(FixedBytes(src_chain), token_address, amount, nonce)
            .value(operator_gas);

        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send withdrawSubmit: {}", e))?;
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("withdrawSubmit transaction reverted"));
        }

        Ok(receipt.transaction_hash)
    }

    /// Execute an approved withdrawal (unlock mode)
    pub async fn withdraw_execute_unlock(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        withdraw_hash: [u8; 32],
    ) -> Result<FixedBytes<32>> {
        use crate::evm::contracts::Bridge;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let bridge = Bridge::new(bridge_address, signer.provider());

        let call = bridge.withdrawExecuteUnlock(FixedBytes(withdraw_hash));
        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send withdrawExecuteUnlock: {}", e))?;
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("withdrawExecuteUnlock transaction reverted"));
        }

        Ok(receipt.transaction_hash)
    }

    /// Execute an approved withdrawal (mint mode)
    pub async fn withdraw_execute_mint(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        withdraw_hash: [u8; 32],
    ) -> Result<FixedBytes<32>> {
        use crate::evm::contracts::Bridge;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let bridge = Bridge::new(bridge_address, signer.provider());

        let call = bridge.withdrawExecuteMint(FixedBytes(withdraw_hash));
        let pending_tx = call
            .send()
            .await
            .map_err(|e| eyre!("Failed to send withdrawExecuteMint: {}", e))?;
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| eyre!("Failed to get receipt: {}", e))?;

        if !receipt.status() {
            return Err(eyre!("withdrawExecuteMint transaction reverted"));
        }

        Ok(receipt.transaction_hash)
    }

    // =========================================================================
    // Balance Checking
    // =========================================================================

    /// Get the ETH balance of this user
    pub async fn get_eth_balance(&self, rpc_url: &str, chain_id: u64) -> Result<U256> {
        let signer = self.create_signer(rpc_url, chain_id)?;
        signer.get_balance().await
    }

    /// Get the ERC20 token balance of this user
    pub async fn get_erc20_balance(
        &self,
        rpc_url: &str,
        chain_id: u64,
        token_address: Address,
    ) -> Result<U256> {
        use crate::evm::contracts::ERC20;
        let signer = self.create_signer(rpc_url, chain_id)?;
        let contract = ERC20::new(token_address, signer.provider());

        let balance = contract
            .balanceOf(self.address)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get ERC20 balance: {}", e))?;

        Ok(balance._0)
    }

    /// Get the ERC20 allowance for a spender
    pub async fn get_erc20_allowance(
        &self,
        rpc_url: &str,
        chain_id: u64,
        token_address: Address,
        spender: Address,
    ) -> Result<U256> {
        use crate::evm::contracts::ERC20;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let contract = ERC20::new(token_address, signer.provider());

        let allowance = contract
            .allowance(self.address, spender)
            .call()
            .await
            .map_err(|e| eyre!("Failed to get ERC20 allowance: {}", e))?;

        Ok(allowance._0)
    }

    // =========================================================================
    // Bridge Query Helpers
    // =========================================================================

    /// Get the current deposit nonce from the bridge
    pub async fn get_deposit_nonce(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
    ) -> Result<u64> {
        use crate::evm::contracts::Bridge;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let bridge = Bridge::new(bridge_address, signer.provider());

        let nonce = bridge
            .getDepositNonce()
            .call()
            .await
            .map_err(|e| eyre!("Failed to get deposit nonce: {}", e))?;

        Ok(nonce._0)
    }

    /// Get pending withdrawal info
    pub async fn get_pending_withdraw(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        withdraw_hash: [u8; 32],
    ) -> Result<PendingWithdrawInfo> {
        use crate::evm::contracts::Bridge;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let bridge = Bridge::new(bridge_address, signer.provider());

        let result = bridge
            .getPendingWithdraw(FixedBytes(withdraw_hash))
            .call()
            .await
            .map_err(|e| eyre!("Failed to get pending withdraw: {}", e))?;

        Ok(PendingWithdrawInfo {
            src_chain: result.srcChain.0,
            src_account: result.srcAccount.0,
            token: result.token,
            recipient: result.recipient,
            amount: result.amount,
            nonce: result.nonce,
            operator_gas: result.operatorGas,
            submitted_at: result.submittedAt,
            approved_at: result.approvedAt,
            approved: result.approved,
            cancelled: result.cancelled,
            executed: result.executed,
        })
    }

    /// Calculate fee for a deposit amount
    pub async fn calculate_fee(
        &self,
        rpc_url: &str,
        chain_id: u64,
        bridge_address: Address,
        amount: U256,
    ) -> Result<U256> {
        use crate::evm::contracts::Bridge;

        let signer = self.create_signer(rpc_url, chain_id)?;
        let bridge = Bridge::new(bridge_address, signer.provider());

        let fee = bridge
            .calculateFee(self.address, amount)
            .call()
            .await
            .map_err(|e| eyre!("Failed to calculate fee: {}", e))?;

        Ok(fee.feeAmount)
    }
}

/// Pending withdrawal information from the bridge contract
#[cfg(feature = "evm")]
#[derive(Debug, Clone)]
pub struct PendingWithdrawInfo {
    pub src_chain: [u8; 4],
    pub src_account: [u8; 32],
    pub token: Address,
    pub recipient: Address,
    pub amount: U256,
    pub nonce: u64,
    pub operator_gas: U256,
    pub submitted_at: U256,
    pub approved_at: U256,
    pub approved: bool,
    pub cancelled: bool,
    pub executed: bool,
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Convert EVM address to bytes32 (left-padded)
pub fn evm_address_to_bytes32(address: &Address) -> [u8; 32] {
    let mut result = [0u8; 32];
    result[12..].copy_from_slice(address.as_slice());
    result
}

/// Generate a random test private key (NOT cryptographically secure - for testing only)
pub fn generate_test_private_key(seed: u64) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let hash1 = hasher.finish();

    (seed + 1).hash(&mut hasher);
    let hash2 = hasher.finish();

    (seed + 2).hash(&mut hasher);
    let hash3 = hasher.finish();

    (seed + 3).hash(&mut hasher);
    let hash4 = hasher.finish();

    format!("0x{:016x}{:016x}{:016x}{:016x}", hash1, hash2, hash3, hash4)
}

// ============================================================================
// Wait-for Helpers
// ============================================================================

/// Wait for a condition to be true with exponential backoff
pub async fn wait_for<F, Fut>(
    description: &str,
    check: F,
    timeout: std::time::Duration,
    initial_interval: std::time::Duration,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<bool>>,
{
    let start = std::time::Instant::now();
    let mut interval = initial_interval;
    let max_interval = std::time::Duration::from_secs(5);

    while start.elapsed() < timeout {
        match check().await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::debug!(error = %e, "Check failed for '{}', retrying", description);
            }
        }

        tokio::time::sleep(interval).await;
        interval = std::cmp::min(interval * 2, max_interval);
    }

    Err(eyre!(
        "Timeout waiting for '{}' after {:?}",
        description,
        timeout
    ))
}

/// Wait for an EVM transaction to be confirmed
#[cfg(feature = "evm")]
pub async fn wait_for_evm_tx(
    rpc_url: &str,
    _chain_id: u64,
    tx_hash: FixedBytes<32>,
    timeout: std::time::Duration,
) -> Result<bool> {
    use alloy::providers::ProviderBuilder;

    let provider = ProviderBuilder::new().on_http(
        rpc_url
            .parse()
            .map_err(|e| eyre!("Invalid RPC URL: {}", e))?,
    );

    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(receipt) = provider.get_transaction_receipt(tx_hash).await? {
            return Ok(receipt.status());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    Err(eyre!("Timeout waiting for transaction {:?}", tx_hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evm_user_from_private_key() {
        // Anvil's default first account private key
        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let user = EvmUser::from_private_key(pk).unwrap();
        assert_eq!(
            user.address_hex().to_lowercase(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }

    #[test]
    fn test_evm_user_address_bytes32() {
        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let user = EvmUser::from_private_key(pk).unwrap();
        let bytes32 = user.address_bytes32();

        // First 12 bytes should be zero
        assert!(bytes32[..12].iter().all(|&b| b == 0));
        // Last 20 bytes should be the address
        assert_eq!(&bytes32[12..], user.address.as_slice());
    }

    #[test]
    fn test_generate_test_private_key() {
        let pk1 = generate_test_private_key(1);
        let pk2 = generate_test_private_key(2);
        assert_ne!(pk1, pk2);
        assert!(pk1.starts_with("0x"));
        assert_eq!(pk1.len(), 66); // 0x + 64 hex chars
    }
}
