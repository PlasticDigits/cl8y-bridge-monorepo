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

/// Internal representation for variable-length raw addresses
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RawAddress {
    Short([u8; 20]),
    Full([u8; 32]),
}

/// Universal address that can represent addresses from any supported chain
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UniversalAddress {
    /// Chain type code (4 bytes)
    pub chain_type: u32,
    raw: RawAddress,
    /// Reserved bytes (8 bytes, only used for Short addresses)
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
            raw: RawAddress::Short(raw_address),
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
            raw: RawAddress::Short(raw_address),
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

    /// Create a Solana address from a 32-byte public key
    pub fn from_solana(pubkey: &[u8; 32]) -> Result<Self> {
        Ok(Self {
            chain_type: CHAIN_TYPE_SOLANA,
            raw: RawAddress::Full(*pubkey),
            reserved: [0u8; 8],
        })
    }

    /// Create a Solana address from a base58-encoded string
    pub fn from_solana_base58(addr: &str) -> Result<Self> {
        let bytes = bs58::decode(addr)
            .into_vec()
            .map_err(|e| eyre!("Invalid base58 address: {}", e))?;
        if bytes.len() != 32 {
            return Err(eyre!(
                "Invalid Solana address length: expected 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&bytes);
        Self::from_solana(&pubkey)
    }

    // ============================================================================
    // Serialization
    // ============================================================================

    /// Convert to 32-byte array
    ///
    /// Short (EVM/Cosmos): | chain_type (4) | raw_address (20) | reserved (8) |
    /// Full (Solana): | chain_type (4) | pubkey\[0..28\] (28) | — lossy, use to_bytes() for lossless
    pub fn to_bytes32(&self) -> [u8; 32] {
        let mut result = [0u8; 32];
        result[0..4].copy_from_slice(&self.chain_type.to_be_bytes());
        match &self.raw {
            RawAddress::Short(raw) => {
                result[4..24].copy_from_slice(raw);
                result[24..32].copy_from_slice(&self.reserved);
            }
            RawAddress::Full(raw) => {
                result[4..32].copy_from_slice(&raw[0..28]);
            }
        }
        result
    }

    /// Parse from 32-byte array
    pub fn from_bytes32(bytes: &[u8; 32]) -> Result<Self> {
        let chain_type = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if chain_type == 0 {
            return Err(eyre!("Invalid chain type: 0"));
        }
        if chain_type == CHAIN_TYPE_SOLANA {
            let mut pubkey = [0u8; 32];
            pubkey[0..28].copy_from_slice(&bytes[4..32]);
            Ok(Self {
                chain_type,
                raw: RawAddress::Full(pubkey),
                reserved: [0u8; 8],
            })
        } else {
            let mut raw_address = [0u8; 20];
            raw_address.copy_from_slice(&bytes[4..24]);
            let mut reserved = [0u8; 8];
            reserved.copy_from_slice(&bytes[24..32]);
            Ok(Self {
                chain_type,
                raw: RawAddress::Short(raw_address),
                reserved,
            })
        }
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
        match &self.raw {
            RawAddress::Short(raw) => Ok(format!("0x{}", hex::encode(raw))),
            RawAddress::Full(_) => Err(eyre!("EVM address must be 20 bytes")),
        }
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
        match &self.raw {
            RawAddress::Short(raw) => encode_bech32_address(raw, hrp),
            RawAddress::Full(_) => Err(eyre!("Cosmos address must be 20 bytes")),
        }
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

    /// Check if this is a Solana address
    pub fn is_solana(&self) -> bool {
        self.chain_type == CHAIN_TYPE_SOLANA
    }

    /// Check if the chain type is valid (known)
    pub fn is_valid_chain_type(&self) -> bool {
        self.chain_type >= CHAIN_TYPE_EVM && self.chain_type <= CHAIN_TYPE_BITCOIN
    }

    // ============================================================================
    // Raw Address Accessors
    // ============================================================================

    /// Returns the raw address bytes (20 for EVM/Cosmos, 32 for Solana)
    pub fn raw_address_bytes(&self) -> &[u8] {
        match &self.raw {
            RawAddress::Short(raw) => raw,
            RawAddress::Full(raw) => raw,
        }
    }

    /// Returns the 20-byte raw address, or error if the address is 32 bytes (Solana)
    pub fn raw_address_20(&self) -> Result<&[u8; 20]> {
        match &self.raw {
            RawAddress::Short(raw) => Ok(raw),
            RawAddress::Full(_) => Err(eyre!("Address is 32 bytes (Solana), not 20")),
        }
    }

    /// Returns 32 bytes for hash computation.
    /// EVM/Cosmos: 12 zero bytes followed by the 20-byte address (left-padded).
    /// Solana: full 32-byte public key.
    pub fn to_hash_bytes(&self) -> [u8; 32] {
        match &self.raw {
            RawAddress::Short(raw) => {
                let mut result = [0u8; 32];
                result[12..32].copy_from_slice(raw);
                result
            }
            RawAddress::Full(raw) => *raw,
        }
    }

    /// Lossless serialization.
    /// EVM/Cosmos: 32 bytes (same as to_bytes32).
    /// Solana: 36 bytes (4-byte chain_type + 32-byte pubkey).
    pub fn to_bytes(&self) -> Vec<u8> {
        match &self.raw {
            RawAddress::Short(_) => self.to_bytes32().to_vec(),
            RawAddress::Full(raw) => {
                let mut result = Vec::with_capacity(36);
                result.extend_from_slice(&self.chain_type.to_be_bytes());
                result.extend_from_slice(raw);
                result
            }
        }
    }

    /// Parse from lossless byte encoding (inverse of to_bytes).
    /// Accepts 32 bytes (EVM/Cosmos) or 36 bytes (Solana).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        match bytes.len() {
            32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(bytes);
                Self::from_bytes32(&arr)
            }
            36 => {
                let chain_type =
                    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if chain_type != CHAIN_TYPE_SOLANA {
                    return Err(eyre!(
                        "36-byte encoding only valid for Solana (chain_type=3), got {}",
                        chain_type
                    ));
                }
                let mut pubkey = [0u8; 32];
                pubkey.copy_from_slice(&bytes[4..36]);
                Self::from_solana(&pubkey)
            }
            other => Err(eyre!(
                "Invalid length: expected 32 or 36 bytes, got {}",
                other
            )),
        }
    }

    /// Convert to Solana base58-encoded address string
    pub fn to_solana_string(&self) -> Result<String> {
        if self.chain_type != CHAIN_TYPE_SOLANA {
            return Err(eyre!(
                "Expected Solana chain type (3), got {}",
                self.chain_type
            ));
        }
        match &self.raw {
            RawAddress::Full(raw) => Ok(bs58::encode(raw).into_string()),
            RawAddress::Short(_) => Err(eyre!("Solana address must be 32 bytes")),
        }
    }
}

impl fmt::Display for UniversalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.chain_type {
            CHAIN_TYPE_EVM => write!(f, "EVM:{}", hex::encode(self.raw_address_bytes())),
            CHAIN_TYPE_COSMOS => {
                write!(f, "COSMOS:{}", hex::encode(self.raw_address_bytes()))
            }
            CHAIN_TYPE_SOLANA => match &self.raw {
                RawAddress::Full(raw) => {
                    write!(f, "SOLANA:{}", bs58::encode(raw).into_string())
                }
                RawAddress::Short(raw) => write!(f, "SOLANA:{}", hex::encode(raw)),
            },
            CHAIN_TYPE_BITCOIN => {
                write!(f, "BITCOIN:{}", hex::encode(self.raw_address_bytes()))
            }
            _ => write!(
                f,
                "UNKNOWN({}):{}",
                self.chain_type,
                hex::encode(self.raw_address_bytes())
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
        assert_eq!(parsed.raw_address_bytes(), universal.raw_address_bytes());

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
        assert_eq!(parsed.raw_address_bytes(), universal.raw_address_bytes());

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

    // ========================================================================
    // Regression Tests — lock in exact byte-level behavior before refactoring
    // ========================================================================

    const REGRESSION_EVM_ADDR: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    const REGRESSION_TERRA_ADDR: &str = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";

    #[rustfmt::skip]
    const REGRESSION_EVM_BYTES32: [u8; 32] = [
        0x00, 0x00, 0x00, 0x01, // chain_type = 1 (EVM)
        0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce,
        0x6a, 0xb8, 0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66, // raw address (20 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved (8 bytes)
    ];

    #[rustfmt::skip]
    const REGRESSION_COSMOS_BYTES32: [u8; 32] = [
        0x00, 0x00, 0x00, 0x02, // chain_type = 2 (Cosmos)
        0x35, 0x74, 0x30, 0x74, 0x95, 0x6c, 0x71, 0x08, 0x00, 0xe8,
        0x31, 0x98, 0x01, 0x1c, 0xcb, 0xd4, 0xdd, 0xf1, 0x55, 0x6d, // raw address (20 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved (8 bytes)
    ];

    #[test]
    fn regression_evm_to_bytes32() {
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        assert_eq!(addr.to_bytes32(), REGRESSION_EVM_BYTES32);
    }

    #[test]
    fn regression_evm_roundtrip() {
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        let bytes32 = addr.to_bytes32();
        let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
        let recovered = parsed.to_evm_string().unwrap();
        assert_eq!(recovered.to_lowercase(), REGRESSION_EVM_ADDR.to_lowercase());
    }

    #[test]
    fn regression_cosmos_to_bytes32() {
        let addr = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        assert_eq!(addr.to_bytes32(), REGRESSION_COSMOS_BYTES32);
    }

    #[test]
    fn regression_cosmos_roundtrip() {
        let addr = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        let bytes32 = addr.to_bytes32();
        let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
        let recovered = parsed.to_terra_string().unwrap();
        assert_eq!(recovered, REGRESSION_TERRA_ADDR);
    }

    #[test]
    fn regression_bytes32_layout_evm() {
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        let b = addr.to_bytes32();

        assert_eq!(&b[0..4], &[0x00, 0x00, 0x00, 0x01], "chain_type must be EVM (1)");
        assert_eq!(&b[4..24], addr.raw_address_bytes(), "bytes 4..24 must be raw address");
        assert_eq!(&b[24..32], &[0u8; 8], "bytes 24..32 must be zero reserved");
    }

    #[test]
    fn regression_bytes32_layout_cosmos() {
        let addr = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        let b = addr.to_bytes32();

        assert_eq!(&b[0..4], &[0x00, 0x00, 0x00, 0x02], "chain_type must be Cosmos (2)");
        assert_eq!(&b[4..24], addr.raw_address_bytes(), "bytes 4..24 must be raw address");
        assert_eq!(&b[24..32], &[0u8; 8], "bytes 24..32 must be zero reserved");
    }

    #[test]
    fn regression_strict_validation() {
        let mut bytes = REGRESSION_EVM_BYTES32;
        bytes[31] = 0xff; // non-zero reserved
        let result = UniversalAddress::from_bytes32_strict(&bytes);
        assert!(result.is_err(), "non-zero reserved must be rejected by strict");

        let result = UniversalAddress::from_bytes32_strict(&REGRESSION_EVM_BYTES32);
        assert!(result.is_ok(), "zero reserved must be accepted by strict");
    }

    #[test]
    fn regression_chain_type_checks() {
        let evm = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        assert!(evm.is_evm());
        assert!(!evm.is_cosmos());
        assert!(evm.is_valid_chain_type());

        let cosmos = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        assert!(!cosmos.is_evm());
        assert!(cosmos.is_cosmos());
        assert!(cosmos.is_valid_chain_type());

        let unknown = UniversalAddress::new(99, [0u8; 20]).unwrap();
        assert!(!unknown.is_evm());
        assert!(!unknown.is_cosmos());
        assert!(!unknown.is_valid_chain_type());
    }

    #[test]
    fn regression_new_with_reserved() {
        let reserved = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let raw = [0xaau8; 20];
        let addr = UniversalAddress::new_with_reserved(CHAIN_TYPE_EVM, raw, reserved).unwrap();
        let b = addr.to_bytes32();
        assert_eq!(&b[24..32], &reserved, "reserved bytes must be preserved in to_bytes32");
    }

    #[test]
    fn regression_display_format() {
        let evm = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        let display = format!("{}", evm);
        assert!(display.starts_with("EVM:"), "EVM display must start with 'EVM:'");
        assert!(display.contains("f39fd6e51aad88f6f4ce6ab8827279cfffb92266"));

        let cosmos = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        let display = format!("{}", cosmos);
        assert!(display.starts_with("COSMOS:"), "Cosmos display must start with 'COSMOS:'");
        assert!(display.contains("35743074956c710800e83198011ccbd4ddf1556d"));
    }

    #[test]
    fn regression_cross_codebase_parity_evm() {
        let expected_hex = "00000001f39fd6e51aad88f6f4ce6ab8827279cfffb922660000000000000000";
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        assert_eq!(hex::encode(addr.to_bytes32()), expected_hex);
    }

    #[test]
    fn regression_cross_codebase_parity_cosmos() {
        let expected_hex = "0000000235743074956c710800e83198011ccbd4ddf1556d0000000000000000";
        let addr = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        assert_eq!(hex::encode(addr.to_bytes32()), expected_hex);
    }

    // ========================================================================
    // Solana Tests
    // ========================================================================

    #[test]
    fn test_solana_from_pubkey() {
        let mut pubkey = [0u8; 32];
        for i in 0..32 {
            pubkey[i] = i as u8;
        }
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        assert_eq!(addr.chain_type, CHAIN_TYPE_SOLANA);
        assert!(addr.is_solana());
        assert!(!addr.is_evm());
        assert!(!addr.is_cosmos());
        assert!(addr.is_valid_chain_type());
    }

    #[test]
    fn test_solana_roundtrip_base58() {
        let mut pubkey = [0u8; 32];
        for i in 0..32 {
            pubkey[i] = (i + 1) as u8;
        }
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        let base58_str = addr.to_solana_string().unwrap();
        let recovered = UniversalAddress::from_solana_base58(&base58_str).unwrap();
        assert_eq!(recovered.raw_address_bytes(), addr.raw_address_bytes());
        assert_eq!(recovered.chain_type, CHAIN_TYPE_SOLANA);
    }

    #[test]
    fn test_solana_to_hash_bytes() {
        let mut pubkey = [0u8; 32];
        for i in 0..32 {
            pubkey[i] = i as u8;
        }
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        assert_eq!(addr.to_hash_bytes(), pubkey);
    }

    #[test]
    fn test_solana_raw_address_bytes() {
        let mut pubkey = [0u8; 32];
        for i in 0..32 {
            pubkey[i] = i as u8;
        }
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        assert_eq!(addr.raw_address_bytes().len(), 32);
        assert_eq!(addr.raw_address_bytes(), &pubkey[..]);
    }

    #[test]
    fn test_evm_to_hash_bytes() {
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        let hash_bytes = addr.to_hash_bytes();
        assert_eq!(&hash_bytes[0..12], &[0u8; 12]);
        assert_eq!(&hash_bytes[12..32], addr.raw_address_bytes());
    }

    #[test]
    fn test_evm_raw_address_20_works() {
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        let raw20 = addr.raw_address_20();
        assert!(raw20.is_ok());
        assert_eq!(raw20.unwrap().len(), 20);
    }

    #[test]
    fn test_solana_raw_address_20_errors() {
        let pubkey = [42u8; 32];
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        assert!(addr.raw_address_20().is_err());
    }

    #[test]
    fn test_to_bytes_roundtrip_evm() {
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        let bytes = addr.to_bytes();
        assert_eq!(bytes.len(), 32);
        let recovered = UniversalAddress::from_bytes(&bytes).unwrap();
        assert_eq!(recovered.chain_type, addr.chain_type);
        assert_eq!(recovered.raw_address_bytes(), addr.raw_address_bytes());
    }

    #[test]
    fn test_to_bytes_roundtrip_solana() {
        let mut pubkey = [0u8; 32];
        for i in 0..32 {
            pubkey[i] = (i as u8).wrapping_mul(7).wrapping_add(3);
        }
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        let bytes = addr.to_bytes();
        assert_eq!(bytes.len(), 36);
        let recovered = UniversalAddress::from_bytes(&bytes).unwrap();
        assert_eq!(recovered.chain_type, CHAIN_TYPE_SOLANA);
        assert_eq!(recovered.raw_address_bytes(), &pubkey[..]);
    }

    #[test]
    fn test_backward_compat_raw_address() {
        let addr = UniversalAddress::from_evm(REGRESSION_EVM_ADDR).unwrap();
        let expected_raw = parse_evm_address(REGRESSION_EVM_ADDR).unwrap();
        assert_eq!(addr.raw_address_bytes(), &expected_raw[..]);
    }
}
