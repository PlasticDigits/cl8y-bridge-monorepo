//! User EOA (Externally Owned Account) Helpers
//!
//! Utilities for simulating user accounts in E2E tests.

use alloy::primitives::Address;
use bip39::Mnemonic;
use eyre::{eyre, Result};

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
}

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
}

/// Convert EVM address to bytes32 (left-padded)
pub fn evm_address_to_bytes32(address: &Address) -> [u8; 32] {
    let mut result = [0u8; 32];
    result[12..].copy_from_slice(address.as_slice());
    result
}

/// Convert Terra address to bytes32 (decode bech32, left-pad)
pub fn terra_address_to_bytes32(address: &str) -> Result<[u8; 32]> {
    let (raw, hrp) = crate::address_codec::decode_bech32_address(address)?;

    if hrp != "terra" {
        return Err(eyre!("Expected 'terra' prefix, got '{}'", hrp));
    }

    let mut result = [0u8; 32];
    result[12..].copy_from_slice(&raw);
    Ok(result)
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

/// Generate a test mnemonic (NOT cryptographically secure - for testing only)
pub fn generate_test_mnemonic() -> String {
    // Use a fixed test mnemonic for deterministic tests
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        .to_string()
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
    fn test_terra_user_from_mnemonic() {
        let mnemonic = generate_test_mnemonic();
        let user = TerraUser::from_mnemonic(&mnemonic).unwrap();
        assert!(user.address.starts_with("terra"));
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
