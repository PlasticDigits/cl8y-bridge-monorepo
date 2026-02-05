//! Common types for cross-chain operations
//!
//! This module provides shared types used across operator, canceler, and E2E packages.

#![allow(dead_code)]

use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// V2 Chain ID (4 bytes)
// ============================================================================

/// Represents a 4-byte chain ID
///
/// Chains are identified by a sequential 4-byte ID assigned during
/// registration in the ChainRegistry contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ChainId(pub [u8; 4]);

impl ChainId {
    /// Create from u32
    pub fn from_u32(id: u32) -> Self {
        ChainId(id.to_be_bytes())
    }

    /// Convert to u32
    pub fn to_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    /// Create from 4 bytes
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        ChainId(bytes)
    }

    /// Create from hex string (with or without 0x prefix)
    pub fn from_hex(hex: &str) -> Result<Self> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let bytes = hex::decode(hex)?;
        if bytes.len() != 4 {
            return Err(eyre!("ChainId must be 4 bytes, got {}", bytes.len()));
        }
        let mut result = [0u8; 4];
        result.copy_from_slice(&bytes);
        Ok(ChainId(result))
    }

    /// Convert to hex string with 0x prefix
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_u32())
    }
}

impl From<u32> for ChainId {
    fn from(id: u32) -> Self {
        ChainId::from_u32(id)
    }
}

impl From<[u8; 4]> for ChainId {
    fn from(bytes: [u8; 4]) -> Self {
        ChainId(bytes)
    }
}

/// Processing status for deposits, approvals, and releases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Pending,
    Submitted,
    Confirmed,
    Failed,
    Cancelled,
    Reorged,
}

impl Status {
    /// Get the status as a lowercase string
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Submitted => "submitted",
            Status::Confirmed => "confirmed",
            Status::Failed => "failed",
            Status::Cancelled => "cancelled",
            Status::Reorged => "reorged",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// EVM address (20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvmAddress(pub [u8; 20]);

impl EvmAddress {
    /// Create from hex string (with or without 0x prefix)
    ///
    /// Accepts both 20-byte addresses (40 hex chars) and 32-byte padded addresses
    /// (64 hex chars). For 32-byte addresses, the first 12 bytes must be zeros
    /// and the last 20 bytes are extracted.
    pub fn from_hex(hex: &str) -> Result<Self, eyre::Error> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let bytes = hex::decode(hex)?;

        match bytes.len() {
            20 => {
                let mut result = [0u8; 20];
                result.copy_from_slice(&bytes);
                Ok(EvmAddress(result))
            }
            32 => {
                // 32-byte padded address - first 12 bytes should be zeros
                if bytes[..12].iter().any(|&b| b != 0) {
                    return Err(eyre!(
                        "32-byte address has non-zero padding: expected 12 leading zero bytes"
                    ));
                }
                let mut result = [0u8; 20];
                result.copy_from_slice(&bytes[12..]);
                Ok(EvmAddress(result))
            }
            len => Err(eyre!(
                "EvmAddress must be 20 or 32 bytes, got {} bytes",
                len
            )),
        }
    }

    /// Convert to hex string with 0x prefix
    pub fn as_hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }

    /// Convert to bytes32 (left-padded with zeros)
    pub fn as_bytes32(&self) -> [u8; 32] {
        let mut result = [0u8; 32];
        result[12..].copy_from_slice(&self.0);
        result
    }

    /// Create from bytes32 (extract last 20 bytes)
    pub fn from_bytes32(bytes: &[u8; 32]) -> Self {
        let mut result = [0u8; 20];
        result.copy_from_slice(&bytes[12..]);
        EvmAddress(result)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
}

impl fmt::Display for EvmAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

/// Unique identifier for a withdrawal
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WithdrawHash(pub [u8; 32]);

impl WithdrawHash {
    /// Compute withdraw hash from V2 parameters
    pub fn compute(
        src_chain: &ChainId,
        dest_chain: &ChainId,
        token: &[u8; 32],
        src_account: &[u8; 32],
        amount: u128,
        nonce: u64,
    ) -> Self {
        let hash = crate::hash::compute_withdraw_hash(
            src_chain.as_bytes(),
            dest_chain.as_bytes(),
            token,
            src_account,
            amount,
            nonce,
        );
        WithdrawHash(hash)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }

    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        WithdrawHash(bytes)
    }

    /// Create from hex string
    pub fn from_hex(hex: &str) -> Result<Self> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let bytes = hex::decode(hex)?;
        if bytes.len() != 32 {
            return Err(eyre!("WithdrawHash must be 32 bytes"));
        }
        let mut result = [0u8; 32];
        result.copy_from_slice(&bytes);
        Ok(WithdrawHash(result))
    }
}

impl fmt::Display for WithdrawHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Token type for bridging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenType {
    /// Native chain token (ETH, LUNA, etc.)
    Native,
    /// Lock/unlock token (external ERC20, etc.)
    LockUnlock,
    /// Mint/burn token (bridged wrapped tokens)
    MintBurn,
}

impl TokenType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenType::Native => "native",
            TokenType::LockUnlock => "lock_unlock",
            TokenType::MintBurn => "mint_burn",
        }
    }
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_from_u32() {
        let id = ChainId::from_u32(1);
        assert_eq!(id.to_u32(), 1);
        assert_eq!(id.0, [0, 0, 0, 1]);
    }

    #[test]
    fn test_chain_id_from_hex() {
        let id = ChainId::from_hex("0x00000001").unwrap();
        assert_eq!(id.to_u32(), 1);

        let id2 = ChainId::from_hex("00000002").unwrap();
        assert_eq!(id2.to_u32(), 2);
    }

    #[test]
    fn test_chain_id_to_hex() {
        let id = ChainId::from_u32(256);
        assert_eq!(id.to_hex(), "0x00000100");
    }

    #[test]
    fn test_chain_id_display() {
        let id = ChainId::from_u32(42);
        assert_eq!(format!("{}", id), "42");
    }

    #[test]
    fn test_chain_id_from_bytes() {
        let bytes: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
        let id = ChainId::from_bytes(bytes);
        assert_eq!(id.0, bytes);
        assert_eq!(id.to_u32(), 0x12345678);
    }

    #[test]
    fn test_evm_address_from_hex() {
        let addr = EvmAddress::from_hex("0xdead000000000000000000000000000000000000").unwrap();
        assert_eq!(addr.0[0], 0xde);
        assert_eq!(addr.0[1], 0xad);
    }

    #[test]
    fn test_evm_address_from_hex_without_prefix() {
        let addr = EvmAddress::from_hex("dead000000000000000000000000000000000000").unwrap();
        assert_eq!(addr.0[0], 0xde);
    }

    #[test]
    fn test_evm_address_invalid_length() {
        let result = EvmAddress::from_hex("0xdead");
        assert!(result.is_err());
    }

    #[test]
    fn test_evm_address_bytes32_roundtrip() {
        let addr = EvmAddress::from_hex("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
        let bytes32 = addr.as_bytes32();
        let recovered = EvmAddress::from_bytes32(&bytes32);
        assert_eq!(addr, recovered);
    }

    #[test]
    fn test_withdraw_hash_hex() {
        let hash = WithdrawHash::from_bytes([1u8; 32]);
        let hex = hash.to_hex();
        assert!(hex.starts_with("0x"));
        assert_eq!(hex.len(), 66);

        let parsed = WithdrawHash::from_hex(&hex).unwrap();
        assert_eq!(hash, parsed);
    }

    #[test]
    fn test_status_as_str() {
        assert_eq!(Status::Pending.as_str(), "pending");
        assert_eq!(Status::Submitted.as_str(), "submitted");
        assert_eq!(Status::Confirmed.as_str(), "confirmed");
        assert_eq!(Status::Failed.as_str(), "failed");
        assert_eq!(Status::Cancelled.as_str(), "cancelled");
        assert_eq!(Status::Reorged.as_str(), "reorged");
    }

    #[test]
    fn test_token_type_display() {
        assert_eq!(format!("{}", TokenType::Native), "native");
        assert_eq!(format!("{}", TokenType::LockUnlock), "lock_unlock");
        assert_eq!(format!("{}", TokenType::MintBurn), "mint_burn");
    }
}
