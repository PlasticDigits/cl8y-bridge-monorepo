#![allow(dead_code)]

use alloy_primitives::keccak256;
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a canonical chain identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ChainKey(pub [u8; 32]);

impl ChainKey {
    /// Create an EVM chain key: keccak256("EVM", chainId)
    pub fn evm(chain_id: u64) -> Self {
        let mut input = b"EVM".to_vec();
        input.extend(chain_id.to_be_bytes());
        let hash = keccak256(&input);
        ChainKey(hash.0)
    }

    /// Create a Cosmos chain key: keccak256("COSMOS", chainId, addressPrefix)
    pub fn cosmos(chain_id: &str, address_prefix: &str) -> Self {
        let mut input = b"COSMOS".to_vec();
        input.extend(chain_id.as_bytes());
        input.extend(b":");
        input.extend(address_prefix.as_bytes());
        let hash = keccak256(&input);
        ChainKey(hash.0)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create from hex string
    pub fn from_hex(hex: &str) -> Result<Self, eyre::Error> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let bytes = hex::decode(hex)?;
        if bytes.len() != 32 {
            return Err(eyre!("ChainKey must be 32 bytes"));
        }
        let mut result = [0u8; 32];
        result.copy_from_slice(&bytes);
        Ok(ChainKey(result))
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode_upper(self.0))
    }
}

/// Processing status for deposits, approvals, and releases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
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
    pub fn from_hex(hex: &str) -> Result<Self, eyre::Error> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let bytes = hex::decode(hex)?;
        if bytes.len() != 20 {
            return Err(eyre!("EvmAddress must be 20 bytes"));
        }
        let mut result = [0u8; 20];
        result.copy_from_slice(&bytes);
        Ok(EvmAddress(result))
    }

    /// Convert to checksummed hex string with 0x prefix
    pub fn as_hex(&self) -> String {
        let bytes32 = self.as_bytes32();
        let bytes = bytes32.as_slice();
        let hex_lower = hex::encode_upper(bytes);
        let mut result = String::with_capacity(42);
        result.push('0');
        result.push('x');
        for (i, c) in hex_lower.chars().enumerate() {
            let byte = bytes[i];
            let uppercase = if byte & 0x80 != 0 {
                c.to_ascii_uppercase()
            } else {
                c
            };
            result.push(uppercase);
        }
        result
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
}

impl fmt::Display for EvmAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

/// Unique identifier for a withdrawal approval
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WithdrawHash(pub [u8; 32]);

impl WithdrawHash {
    /// Compute withdraw hash: keccak256(abi.encode(srcChainKey, token, to, amount, nonce))
    pub fn compute(
        src_chain_key: &ChainKey,
        token: &EvmAddress,
        to: &EvmAddress,
        amount: &str,
        nonce: u64,
    ) -> Self {
        let mut input = Vec::new();
        input.extend_from_slice(&src_chain_key.0);
        input.extend_from_slice(&token.0);
        input.extend_from_slice(&to.0);
        input.extend_from_slice(amount.as_bytes());
        input.extend_from_slice(&nonce.to_be_bytes());
        let hash = keccak256(&input);
        WithdrawHash(hash.0)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode_upper(self.0))
    }
}