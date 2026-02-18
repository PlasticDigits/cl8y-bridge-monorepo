//! Terra User EOA (Externally Owned Account) Helpers
//!
//! Utilities for simulating Terra user accounts in E2E tests, including
//! deposit, withdraw, and balance checking operations.
//!
//! ## TerraUser Operations
//!
//! - `deposit_native()` - Deposit native LUNA to the bridge
//! - `deposit_cw20()` - Deposit CW20 tokens to the bridge
//! - `withdraw_submit()` - Submit a withdrawal on Terra
//! - `get_native_balance()` - Get native token balance
//! - `get_cw20_balance()` - Get CW20 token balance

use bip39::Mnemonic;
use eyre::{eyre, Result};

// ============================================================================
// Terra User
// ============================================================================

/// Represents a Terra user account for testing
#[derive(Debug, Clone)]
pub struct TerraUser {
    /// Mnemonic phrase
    pub mnemonic: String,
    /// Address
    pub address: String,
}

impl TerraUser {
    /// Create from mnemonic phrase
    pub fn from_mnemonic(mnemonic: &str) -> Result<Self> {
        use cosmrs::{bip32::DerivationPath, crypto::secp256k1::SigningKey};

        // Parse and validate the mnemonic
        let m: Mnemonic = mnemonic
            .parse()
            .map_err(|e| eyre!("Invalid mnemonic: {}", e))?;
        let seed = m.to_seed("");

        // Derive the key using Terra's derivation path (coin type 330)
        let path: DerivationPath = "m/44'/330'/0'/0/0"
            .parse()
            .map_err(|e| eyre!("Invalid derivation path: {:?}", e))?;

        let signing_key = SigningKey::derive_from_path(seed, &path)
            .map_err(|e| eyre!("Failed to derive key: {:?}", e))?;

        // Get the public key and derive the address
        let public_key = signing_key.public_key();
        let account_id = public_key.account_id("terra")?;
        let address = account_id.to_string();

        Ok(Self {
            mnemonic: mnemonic.to_string(),
            address,
        })
    }

    /// Get address
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Get address as bytes32 (decode bech32, left-pad)
    pub fn address_bytes32(&self) -> Result<[u8; 32]> {
        terra_address_to_bytes32(&self.address)
    }
}

#[cfg(feature = "terra")]
impl TerraUser {
    /// Create a signer for this user
    pub fn create_signer(
        &self,
        lcd_url: &str,
        chain_id: &str,
    ) -> Result<crate::terra::signer::TerraSigner> {
        crate::terra::signer::TerraSigner::from_mnemonic(lcd_url, chain_id, &self.mnemonic)
    }

    // =========================================================================
    // Deposit Operations
    // =========================================================================

    /// Deposit native tokens (uluna) to the Terra bridge
    ///
    /// Calls the bridge contract's deposit function with uluna funds attached
    #[allow(clippy::too_many_arguments)]
    pub async fn deposit_native(
        &self,
        lcd_url: &str,
        chain_id: &str,
        bridge_address: &str,
        dest_chain: [u8; 4],
        dest_account: [u8; 32],
        amount: u128,
        denom: &str,
    ) -> Result<String> {
        use base64::Engine;
        let encoder = base64::engine::general_purpose::STANDARD;

        let signer = self.create_signer(lcd_url, chain_id)?;

        // Build the deposit message
        let deposit_msg = serde_json::json!({
            "deposit": {
                "dest_chain": encoder.encode(dest_chain),
                "dest_account": encoder.encode(dest_account)
            }
        });

        let funds = vec![(denom.to_string(), amount)];

        let result = signer
            .sign_and_broadcast_execute(bridge_address, &deposit_msg, funds)
            .await?;

        if !result.success {
            return Err(eyre!(
                "deposit_native failed: {}",
                result.raw_log.unwrap_or_default()
            ));
        }

        Ok(result.tx_hash)
    }

    /// Deposit CW20 tokens to the Terra bridge
    ///
    /// Sends CW20 tokens to the bridge contract with a deposit message
    #[allow(clippy::too_many_arguments)]
    pub async fn deposit_cw20(
        &self,
        lcd_url: &str,
        chain_id: &str,
        bridge_address: &str,
        token_address: &str,
        amount: u128,
        dest_chain: [u8; 4],
        dest_account: [u8; 32],
    ) -> Result<String> {
        use base64::Engine;
        let encoder = base64::engine::general_purpose::STANDARD;

        let signer = self.create_signer(lcd_url, chain_id)?;

        // Build the inner deposit message (will be base64-encoded in the CW20 send)
        let deposit_msg = serde_json::json!({
            "deposit": {
                "dest_chain": encoder.encode(dest_chain),
                "dest_account": encoder.encode(dest_account)
            }
        });

        let deposit_msg_str = serde_json::to_string(&deposit_msg)?;

        // Build the CW20 send message
        let send_msg =
            crate::terra::tokens::build_cw20_send_msg(bridge_address, amount, &deposit_msg_str);

        let result = signer
            .sign_and_broadcast_execute(token_address, &send_msg, vec![])
            .await?;

        if !result.success {
            return Err(eyre!(
                "deposit_cw20 failed: {}",
                result.raw_log.unwrap_or_default()
            ));
        }

        Ok(result.tx_hash)
    }

    // =========================================================================
    // Withdrawal Operations
    // =========================================================================

    /// Submit a withdrawal on Terra (V2 user-initiated flow)
    #[allow(clippy::too_many_arguments)]
    pub async fn withdraw_submit(
        &self,
        lcd_url: &str,
        chain_id: &str,
        bridge_address: &str,
        src_chain: [u8; 4],
        token: &str,
        amount: u128,
        nonce: u64,
    ) -> Result<String> {
        use base64::Engine;
        let encoder = base64::engine::general_purpose::STANDARD;

        let signer = self.create_signer(lcd_url, chain_id)?;

        let msg = crate::terra::contracts::ExecuteMsgV2::WithdrawSubmit {
            src_chain: encoder.encode(src_chain),
            token: token.to_string(),
            amount: amount.to_string(),
            nonce,
        };

        let result = signer
            .sign_and_broadcast_execute(bridge_address, &msg, vec![])
            .await?;

        if !result.success {
            return Err(eyre!(
                "withdraw_submit failed: {}",
                result.raw_log.unwrap_or_default()
            ));
        }

        Ok(result.tx_hash)
    }

    /// Execute a withdrawal (unlock mode) on Terra
    pub async fn withdraw_execute_unlock(
        &self,
        lcd_url: &str,
        chain_id: &str,
        bridge_address: &str,
        xchain_hash_id: [u8; 32],
    ) -> Result<String> {
        let signer = self.create_signer(lcd_url, chain_id)?;

        let msg = crate::terra::contracts::build_withdraw_execute_unlock_msg_v2(xchain_hash_id);

        let result = signer
            .sign_and_broadcast_execute(bridge_address, &msg, vec![])
            .await?;

        if !result.success {
            return Err(eyre!(
                "withdraw_execute_unlock failed: {}",
                result.raw_log.unwrap_or_default()
            ));
        }

        Ok(result.tx_hash)
    }

    /// Execute a withdrawal (mint mode) on Terra
    pub async fn withdraw_execute_mint(
        &self,
        lcd_url: &str,
        chain_id: &str,
        bridge_address: &str,
        xchain_hash_id: [u8; 32],
    ) -> Result<String> {
        let signer = self.create_signer(lcd_url, chain_id)?;

        let msg = crate::terra::contracts::build_withdraw_execute_mint_msg_v2(xchain_hash_id);

        let result = signer
            .sign_and_broadcast_execute(bridge_address, &msg, vec![])
            .await?;

        if !result.success {
            return Err(eyre!(
                "withdraw_execute_mint failed: {}",
                result.raw_log.unwrap_or_default()
            ));
        }

        Ok(result.tx_hash)
    }

    // =========================================================================
    // Balance Checking
    // =========================================================================

    /// Get native token balance (uluna, uusd, etc.)
    pub async fn get_native_balance(&self, lcd_url: &str, denom: &str) -> Result<u128> {
        crate::terra::tokens::query_native_balance(lcd_url, &self.address, denom).await
    }

    /// Get CW20 token balance
    pub async fn get_cw20_balance(&self, lcd_url: &str, token_address: &str) -> Result<u128> {
        crate::terra::tokens::query_cw20_balance(lcd_url, token_address, &self.address).await
    }

    // =========================================================================
    // Bridge Query Helpers
    // =========================================================================

    /// Query pending withdrawal info from the Terra bridge
    pub async fn get_pending_withdraw(
        &self,
        lcd_url: &str,
        bridge_address: &str,
        xchain_hash_id: [u8; 32],
    ) -> Result<crate::terra::contracts::PendingWithdrawResponse> {
        let query_client = crate::terra::queries::TerraQueryClient::new(lcd_url, bridge_address);
        query_client.get_pending_withdraw(xchain_hash_id).await
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Convert Terra address to bytes32 (decode bech32, left-pad)
///
/// Supports both 20-byte wallet addresses and 32-byte contract addresses.
pub fn terra_address_to_bytes32(address: &str) -> Result<[u8; 32]> {
    let (raw, hrp) = crate::address_codec::decode_bech32_address_raw(address)?;

    if hrp != "terra" {
        return Err(eyre!("Expected 'terra' prefix, got '{}'", hrp));
    }

    let mut result = [0u8; 32];
    if raw.len() == 32 {
        result.copy_from_slice(&raw);
    } else {
        let start = 32 - raw.len();
        result[start..].copy_from_slice(&raw);
    }
    Ok(result)
}

/// Generate a test mnemonic (NOT cryptographically secure - for testing only)
pub fn generate_test_mnemonic() -> String {
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terra_user_from_mnemonic() {
        let mnemonic = generate_test_mnemonic();
        let user = TerraUser::from_mnemonic(&mnemonic).unwrap();
        assert!(user.address.starts_with("terra"));
    }

    #[test]
    fn test_terra_user_address_bytes32() {
        let mnemonic = generate_test_mnemonic();
        let user = TerraUser::from_mnemonic(&mnemonic).unwrap();
        let bytes32 = user.address_bytes32().unwrap();

        // Should be 32 bytes
        assert_eq!(bytes32.len(), 32);
        // First 12 bytes should be zero (20-byte address left-padded)
        assert!(bytes32[..12].iter().all(|&b| b == 0));
    }
}
