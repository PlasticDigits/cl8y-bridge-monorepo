//! Universal Cross-Chain Address Encoding
//!
//! This module provides unified address encoding/decoding that matches
//! both the EVM AddressCodecLib.sol and Terra address_codec.rs implementations.
//!
//! ## Address Format
//!
//! All addresses are stored as 32 bytes with format:
//! ```text
//! | Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
//! ```
//!
//! ## Chain Type Codes
//!
//! - `0x00000001`: EVM (Ethereum, BSC, Polygon, etc.)
//! - `0x00000002`: Cosmos/Terra (Terra Classic, Osmosis)
//! - `0x00000003`: Solana (future)
//! - `0x00000004`: Bitcoin (future)

use bech32::{self, FromBase32, ToBase32, Variant};
use eyre::{eyre, Result};
use std::fmt;

// ============================================================================
// Chain Type Constants
// ============================================================================

/// Chain type for EVM-compatible chains
pub const CHAIN_TYPE_EVM: u32 = 1;

/// Chain type for Cosmos/Terra chains
pub const CHAIN_TYPE_COSMOS: u32 = 2;

/// Chain type for Solana (future)
pub const CHAIN_TYPE_SOLANA: u32 = 3;

/// Chain type for Bitcoin (future)
pub const CHAIN_TYPE_BITCOIN: u32 = 4;

// ============================================================================
// Universal Address Structure
// ============================================================================

/// Universal address that can represent addresses from any supported chain
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UniversalAddress {
    /// Chain type code (4 bytes)
    pub chain_type: u32,
    /// Raw 20-byte address
    pub raw_address: [u8; 20],
    /// Reserved bytes for future use (8 bytes)
    pub reserved: [u8; 8],
}

impl UniversalAddress {
    /// Create a new UniversalAddress with default reserved bytes
    pub fn new(chain_type: u32, raw_address: [u8; 20]) -> Result<Self> {
        if chain_type == 0 {
            return Err(eyre!("Invalid chain type: 0"));
        }
        Ok(Self {
            chain_type,
            raw_address,
            reserved: [0u8; 8],
        })
    }

    /// Create a new UniversalAddress with explicit reserved bytes
    pub fn new_with_reserved(
        chain_type: u32,
        raw_address: [u8; 20],
        reserved: [u8; 8],
    ) -> Result<Self> {
        if chain_type == 0 {
            return Err(eyre!("Invalid chain type: 0"));
        }
        Ok(Self {
            chain_type,
            raw_address,
            reserved,
        })
    }

    // ============================================================================
    // Chain-Specific Constructors
    // ============================================================================

    /// Create an EVM address from a 0x-prefixed hex string
    pub fn from_evm(addr: &str) -> Result<Self> {
        let raw = parse_evm_address(addr)?;
        Self::new(CHAIN_TYPE_EVM, raw)
    }

    /// Create a Cosmos address from a bech32 string (e.g., "terra1...")
    pub fn from_cosmos(addr: &str) -> Result<Self> {
        let (raw, _hrp) = decode_bech32_address(addr)?;
        Self::new(CHAIN_TYPE_COSMOS, raw)
    }

    // ============================================================================
    // Serialization
    // ============================================================================

    /// Convert to 32-byte array
    ///
    /// Layout: | chain_type (4) | raw_address (20) | reserved (8) |
    pub fn to_bytes32(&self) -> [u8; 32] {
        let mut result = [0u8; 32];

        // Chain type in big-endian (first 4 bytes)
        result[0..4].copy_from_slice(&self.chain_type.to_be_bytes());

        // Raw address (bytes 4-23)
        result[4..24].copy_from_slice(&self.raw_address);

        // Reserved (bytes 24-31)
        result[24..32].copy_from_slice(&self.reserved);

        result
    }

    /// Parse from 32-byte array
    pub fn from_bytes32(bytes: &[u8; 32]) -> Result<Self> {
        // Extract chain type (first 4 bytes, big-endian)
        let chain_type = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        if chain_type == 0 {
            return Err(eyre!("Invalid chain type: 0"));
        }

        // Extract raw address (bytes 4-23)
        let mut raw_address = [0u8; 20];
        raw_address.copy_from_slice(&bytes[4..24]);

        // Extract reserved (bytes 24-31)
        let mut reserved = [0u8; 8];
        reserved.copy_from_slice(&bytes[24..32]);

        Ok(Self {
            chain_type,
            raw_address,
            reserved,
        })
    }

    /// Parse from 32-byte array with strict validation (reserved must be zero)
    pub fn from_bytes32_strict(bytes: &[u8; 32]) -> Result<Self> {
        let addr = Self::from_bytes32(bytes)?;

        if addr.reserved != [0u8; 8] {
            return Err(eyre!("Non-zero reserved bytes"));
        }

        Ok(addr)
    }

    /// Parse from slice (must be exactly 32 bytes)
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(eyre!(
                "Invalid length: expected 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Self::from_bytes32(&arr)
    }

    // ============================================================================
    // Chain-Specific Formatters
    // ============================================================================

    /// Convert to EVM hex string (0x-prefixed)
    ///
    /// Returns error if chain type is not EVM
    pub fn to_evm_string(&self) -> Result<String> {
        if self.chain_type != CHAIN_TYPE_EVM {
            return Err(eyre!(
                "Expected EVM chain type (1), got {}",
                self.chain_type
            ));
        }
        Ok(format!("0x{}", hex::encode(self.raw_address)))
    }

    /// Convert to Cosmos bech32 string with given prefix
    ///
    /// Returns error if chain type is not Cosmos
    pub fn to_cosmos_string(&self, hrp: &str) -> Result<String> {
        if self.chain_type != CHAIN_TYPE_COSMOS {
            return Err(eyre!(
                "Expected Cosmos chain type (2), got {}",
                self.chain_type
            ));
        }
        encode_bech32_address(&self.raw_address, hrp)
    }

    /// Convert to Terra address string
    pub fn to_terra_string(&self) -> Result<String> {
        self.to_cosmos_string("terra")
    }

    // ============================================================================
    // Validation
    // ============================================================================

    /// Check if this is an EVM address
    pub fn is_evm(&self) -> bool {
        self.chain_type == CHAIN_TYPE_EVM
    }

    /// Check if this is a Cosmos address
    pub fn is_cosmos(&self) -> bool {
        self.chain_type == CHAIN_TYPE_COSMOS
    }

    /// Check if the chain type is valid (known)
    pub fn is_valid_chain_type(&self) -> bool {
        self.chain_type >= CHAIN_TYPE_EVM && self.chain_type <= CHAIN_TYPE_BITCOIN
    }
}

impl fmt::Display for UniversalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.chain_type {
            CHAIN_TYPE_EVM => write!(f, "EVM:{}", hex::encode(self.raw_address)),
            CHAIN_TYPE_COSMOS => write!(f, "COSMOS:{}", hex::encode(self.raw_address)),
            CHAIN_TYPE_SOLANA => write!(f, "SOLANA:{}", hex::encode(self.raw_address)),
            CHAIN_TYPE_BITCOIN => write!(f, "BITCOIN:{}", hex::encode(self.raw_address)),
            _ => write!(
                f,
                "UNKNOWN({}){}",
                self.chain_type,
                hex::encode(self.raw_address)
            ),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse a 0x-prefixed hex EVM address to 20 bytes
pub fn parse_evm_address(addr: &str) -> Result<[u8; 20]> {
    let hex_str = addr.strip_prefix("0x").unwrap_or(addr);

    if hex_str.len() != 40 {
        return Err(eyre!(
            "Invalid EVM address length: expected 40 hex chars, got {}",
            hex_str.len()
        ));
    }

    let bytes = hex::decode(hex_str)?;

    let mut result = [0u8; 20];
    result.copy_from_slice(&bytes);
    Ok(result)
}

/// Encode 20 bytes to EVM hex string with 0x prefix
pub fn encode_evm_address(bytes: &[u8; 20]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Decode a bech32 address to raw 20 bytes
///
/// Returns (raw_bytes, hrp) where hrp is the human-readable prefix
pub fn decode_bech32_address(addr: &str) -> Result<([u8; 20], String)> {
    let (hrp, data, _variant) =
        bech32::decode(addr).map_err(|e| eyre!("Invalid bech32 address: {}", e))?;

    let bytes = Vec::<u8>::from_base32(&data).map_err(|e| eyre!("Invalid base32 data: {}", e))?;

    if bytes.len() != 20 {
        return Err(eyre!(
            "Invalid address length: expected 20 bytes, got {}",
            bytes.len()
        ));
    }

    let mut result = [0u8; 20];
    result.copy_from_slice(&bytes);
    Ok((result, hrp))
}

/// Decode a bech32 address to raw bytes (variable length: 20 or 32 bytes)
///
/// Unlike [`decode_bech32_address`] which only accepts 20-byte addresses,
/// this function supports both 20-byte (wallet) and 32-byte (contract) addresses.
/// Returns (raw_bytes, hrp) where hrp is the human-readable prefix.
pub fn decode_bech32_address_raw(addr: &str) -> Result<(Vec<u8>, String)> {
    let (hrp, data, _variant) =
        bech32::decode(addr).map_err(|e| eyre!("Invalid bech32 address: {}", e))?;

    let bytes = Vec::<u8>::from_base32(&data).map_err(|e| eyre!("Invalid base32 data: {}", e))?;

    if bytes.len() != 20 && bytes.len() != 32 {
        return Err(eyre!(
            "Invalid address length: expected 20 or 32 bytes, got {}",
            bytes.len()
        ));
    }

    Ok((bytes, hrp))
}

/// Encode raw 20 bytes to a bech32 address with given prefix
pub fn encode_bech32_address(bytes: &[u8; 20], hrp: &str) -> Result<String> {
    let encoded = bech32::encode(hrp, bytes.to_base32(), Variant::Bech32)
        .map_err(|e| eyre!("Failed to encode bech32: {}", e))?;
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evm_address_encode_decode() {
        let evm_addr = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
        let universal = UniversalAddress::from_evm(evm_addr).unwrap();

        assert_eq!(universal.chain_type, CHAIN_TYPE_EVM);
        assert!(universal.is_evm());
        assert!(!universal.is_cosmos());

        let bytes32 = universal.to_bytes32();

        // Chain type should be in first 4 bytes
        assert_eq!(&bytes32[0..4], &[0, 0, 0, 1]);

        // Parse back
        let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
        assert_eq!(parsed.chain_type, CHAIN_TYPE_EVM);
        assert_eq!(parsed.raw_address, universal.raw_address);

        // Convert to string
        let recovered = parsed.to_evm_string().unwrap();
        assert_eq!(recovered.to_lowercase(), evm_addr.to_lowercase());
    }

    #[test]
    fn test_cosmos_address_encode_decode() {
        let terra_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let universal = UniversalAddress::from_cosmos(terra_addr).unwrap();

        assert_eq!(universal.chain_type, CHAIN_TYPE_COSMOS);
        assert!(universal.is_cosmos());
        assert!(!universal.is_evm());

        let bytes32 = universal.to_bytes32();

        // Chain type should be in first 4 bytes (Cosmos = 2)
        assert_eq!(&bytes32[0..4], &[0, 0, 0, 2]);

        // Parse back
        let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
        assert_eq!(parsed.chain_type, CHAIN_TYPE_COSMOS);
        assert_eq!(parsed.raw_address, universal.raw_address);

        // Convert to string
        let recovered = parsed.to_terra_string().unwrap();
        assert_eq!(recovered, terra_addr);
    }

    #[test]
    fn test_bytes32_roundtrip() {
        let evm_addr = "0xdead000000000000000000000000000000000000";
        let universal = UniversalAddress::from_evm(evm_addr).unwrap();

        let bytes32 = universal.to_bytes32();
        let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();

        assert_eq!(universal, parsed);
    }

    #[test]
    fn test_invalid_chain_type() {
        let result = UniversalAddress::new(0, [0u8; 20]);
        assert!(result.is_err());

        let bytes32 = [0u8; 32];
        let result = UniversalAddress::from_bytes32(&bytes32);
        assert!(result.is_err());
    }

    #[test]
    fn test_strict_validation() {
        let mut bytes32 = [0u8; 32];
        // Set valid chain type
        bytes32[3] = 1;
        // Set non-zero reserved
        bytes32[31] = 1;

        let result = UniversalAddress::from_bytes32_strict(&bytes32);
        assert!(result.is_err());

        // With zero reserved, should succeed
        bytes32[31] = 0;
        let result = UniversalAddress::from_bytes32_strict(&bytes32);
        assert!(result.is_ok());
    }

    #[test]
    fn test_display() {
        let evm_addr =
            UniversalAddress::from_evm("0xdead000000000000000000000000000000000000").unwrap();
        let display = format!("{}", evm_addr);
        assert!(display.contains("EVM:"));
        assert!(display.contains("dead"));
    }
}
