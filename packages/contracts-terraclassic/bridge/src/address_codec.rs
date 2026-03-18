//! Universal Cross-Chain Address Encoding
//!
//! This module provides unified address encoding/decoding that matches
//! the EVM AddressCodecLib.sol implementation.
//!
//! ## Address Format
//!
//! EVM/Cosmos addresses are stored as 32 bytes:
//! ```text
//! | Chain Type (4 bytes) | Raw Address (20 bytes) | Reserved (8 bytes) |
//! ```
//!
//! Solana addresses use all 28 remaining bytes in the 32-byte encoding:
//! ```text
//! | Chain Type (4 bytes) | Pubkey[0..28] (28 bytes) |
//! ```
//!
//! ## Chain Type Codes
//!
//! - `0x00000001`: EVM (Ethereum, BSC, Polygon, etc.)
//! - `0x00000002`: Cosmos/Terra (Terra Classic, Osmosis)
//! - `0x00000003`: Solana
//! - `0x00000004`: Bitcoin (future)
//!
//! ## Raw Address
//!
//! - EVM: 20-byte address directly
//! - Cosmos: 20-byte address from bech32 decoding
//! - Solana: 32-byte Ed25519 public key

use cosmwasm_std::{Addr, StdError, StdResult};

// ============================================================================
// Chain Type Constants
// ============================================================================

/// Chain type for EVM-compatible chains
pub const CHAIN_TYPE_EVM: u32 = 1;

/// Chain type for Cosmos/Terra chains
pub const CHAIN_TYPE_COSMOS: u32 = 2;

/// Chain type for Solana
pub const CHAIN_TYPE_SOLANA: u32 = 3;

/// Chain type for Bitcoin (future)
pub const CHAIN_TYPE_BITCOIN: u32 = 4;

// ============================================================================
// Universal Address Structure
// ============================================================================

/// Internal representation for variable-length raw addresses
#[derive(Debug, Clone, PartialEq, Eq)]
enum RawAddress {
    Short([u8; 20]),
    Full([u8; 32]),
}

/// Universal address that can represent addresses from any supported chain
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniversalAddress {
    /// Chain type code (4 bytes)
    pub chain_type: u32,
    raw: RawAddress,
    /// Reserved bytes (8 bytes, only used for Short addresses)
    pub reserved: [u8; 8],
}

impl UniversalAddress {
    /// Create a new UniversalAddress with default reserved bytes
    pub fn new(chain_type: u32, raw_address: [u8; 20]) -> StdResult<Self> {
        if chain_type == 0 {
            return Err(StdError::generic_err("Invalid chain type: 0"));
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
    ) -> StdResult<Self> {
        if chain_type == 0 {
            return Err(StdError::generic_err("Invalid chain type: 0"));
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
    pub fn from_evm(addr: &str) -> StdResult<Self> {
        let raw = parse_evm_address(addr)?;
        Self::new(CHAIN_TYPE_EVM, raw)
    }

    /// Create a Cosmos address from a bech32 string (e.g., "terra1...")
    pub fn from_cosmos(addr: &str) -> StdResult<Self> {
        let raw = decode_bech32_address(addr)?;
        Self::new(CHAIN_TYPE_COSMOS, raw)
    }

    /// Create from a CosmWasm Addr
    pub fn from_addr(addr: &Addr) -> StdResult<Self> {
        Self::from_cosmos(addr.as_str())
    }

    /// Create a Solana address from a 32-byte public key
    pub fn from_solana(pubkey: &[u8; 32]) -> StdResult<Self> {
        Ok(Self {
            chain_type: CHAIN_TYPE_SOLANA,
            raw: RawAddress::Full(*pubkey),
            reserved: [0u8; 8],
        })
    }

    // ============================================================================
    // Serialization
    // ============================================================================

    /// Convert to 32-byte array
    ///
    /// Short (EVM/Cosmos): | chain_type (4) | raw_address (20) | reserved (8) |
    /// Full (Solana): | chain_type (4) | pubkey[0..28] (28) |
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
    pub fn from_bytes32(bytes: &[u8; 32]) -> StdResult<Self> {
        let chain_type = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if chain_type == 0 {
            return Err(StdError::generic_err("Invalid chain type: 0"));
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
    pub fn from_bytes32_strict(bytes: &[u8; 32]) -> StdResult<Self> {
        let addr = Self::from_bytes32(bytes)?;

        if addr.reserved != [0u8; 8] {
            return Err(StdError::generic_err("Non-zero reserved bytes"));
        }

        Ok(addr)
    }

    // ============================================================================
    // Chain-Specific Formatters
    // ============================================================================

    /// Convert to EVM hex string (0x-prefixed)
    ///
    /// Returns error if chain type is not EVM
    pub fn to_evm_string(&self) -> StdResult<String> {
        if self.chain_type != CHAIN_TYPE_EVM {
            return Err(StdError::generic_err(format!(
                "Expected EVM chain type (1), got {}",
                self.chain_type
            )));
        }
        match &self.raw {
            RawAddress::Short(raw) => Ok(format!("0x{}", hex::encode(raw))),
            RawAddress::Full(_) => Err(StdError::generic_err("EVM address must be 20 bytes")),
        }
    }

    /// Convert to Cosmos bech32 string with given prefix
    ///
    /// Returns error if chain type is not Cosmos
    pub fn to_cosmos_string(&self, hrp: &str) -> StdResult<String> {
        if self.chain_type != CHAIN_TYPE_COSMOS {
            return Err(StdError::generic_err(format!(
                "Expected Cosmos chain type (2), got {}",
                self.chain_type
            )));
        }
        match &self.raw {
            RawAddress::Short(raw) => encode_bech32_address(raw, hrp),
            RawAddress::Full(_) => Err(StdError::generic_err("Cosmos address must be 20 bytes")),
        }
    }

    /// Convert to Terra address string
    pub fn to_terra_string(&self) -> StdResult<String> {
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
    pub fn raw_address_20(&self) -> StdResult<&[u8; 20]> {
        match &self.raw {
            RawAddress::Short(raw) => Ok(raw),
            RawAddress::Full(_) => Err(StdError::generic_err(
                "Address is 32 bytes (Solana), not 20",
            )),
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
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse a 0x-prefixed hex EVM address to 20 bytes
pub fn parse_evm_address(addr: &str) -> StdResult<[u8; 20]> {
    let hex_str = addr.strip_prefix("0x").unwrap_or(addr);

    if hex_str.len() != 40 {
        return Err(StdError::generic_err(format!(
            "Invalid EVM address length: expected 40 hex chars, got {}",
            hex_str.len()
        )));
    }

    let bytes =
        hex::decode(hex_str).map_err(|e| StdError::generic_err(format!("Invalid hex: {}", e)))?;

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
/// Uses a simple bech32 decoder that works with CosmWasm addresses
pub fn decode_bech32_address(addr: &str) -> StdResult<[u8; 20]> {
    // Simple bech32 decoding for Cosmos addresses
    // Format: hrp + "1" + base32_data + checksum

    let parts: Vec<&str> = addr.rsplitn(2, '1').collect();
    if parts.len() != 2 {
        return Err(StdError::generic_err("Invalid bech32 format"));
    }

    let data_part = parts[0];
    // The data part includes the address data + 6 char checksum
    if data_part.len() < 7 {
        return Err(StdError::generic_err("Bech32 data too short"));
    }

    // Remove the 6-character checksum
    let data_without_checksum = &data_part[..data_part.len() - 6];

    // Decode base32 (Bech32 alphabet)
    let decoded = decode_bech32_data(data_without_checksum)?;

    // Convert from 5-bit groups to 8-bit bytes
    let bytes = convert_bits(&decoded, 5, 8, false)?;

    if bytes.len() != 20 {
        return Err(StdError::generic_err(format!(
            "Invalid address length: expected 20 bytes, got {}",
            bytes.len()
        )));
    }

    let mut result = [0u8; 20];
    result.copy_from_slice(&bytes);
    Ok(result)
}

/// Encode raw 20 bytes to a bech32 address with given prefix
pub fn encode_bech32_address(bytes: &[u8; 20], hrp: &str) -> StdResult<String> {
    // Convert 8-bit bytes to 5-bit groups
    let data5 = convert_bits(bytes, 8, 5, true)?;

    // Encode as base32
    let data_str = encode_bech32_data(&data5);

    // Compute checksum
    let checksum = compute_bech32_checksum(hrp, &data5)?;
    let checksum_str = encode_bech32_data(&checksum);

    Ok(format!("{}1{}{}", hrp, data_str, checksum_str))
}

/// Convert bits between different group sizes
fn convert_bits(data: &[u8], from_bits: u32, to_bits: u32, pad: bool) -> StdResult<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut result = Vec::new();
    let max_v = (1u32 << to_bits) - 1;

    for &value in data {
        let v = value as u32;
        acc = (acc << from_bits) | v;
        bits += from_bits;

        while bits >= to_bits {
            bits -= to_bits;
            result.push(((acc >> bits) & max_v) as u8);
        }
    }

    if pad && bits > 0 {
        result.push(((acc << (to_bits - bits)) & max_v) as u8);
    } else if !pad && bits >= from_bits {
        return Err(StdError::generic_err("Invalid padding"));
    }

    Ok(result)
}

/// Bech32 character set
const BECH32_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// Decode bech32 base32 data
fn decode_bech32_data(data: &str) -> StdResult<Vec<u8>> {
    let mut result = Vec::with_capacity(data.len());

    for c in data.chars() {
        let idx = BECH32_CHARSET
            .iter()
            .position(|&x| x as char == c)
            .ok_or_else(|| StdError::generic_err(format!("Invalid bech32 character: {}", c)))?;
        result.push(idx as u8);
    }

    Ok(result)
}

/// Encode bytes to bech32 base32 string
fn encode_bech32_data(data: &[u8]) -> String {
    data.iter()
        .map(|&b| BECH32_CHARSET[b as usize] as char)
        .collect()
}

/// Compute bech32 checksum
fn compute_bech32_checksum(hrp: &str, data: &[u8]) -> StdResult<Vec<u8>> {
    let mut values = expand_hrp(hrp);
    values.extend_from_slice(data);
    values.extend_from_slice(&[0, 0, 0, 0, 0, 0]);

    let polymod = bech32_polymod(&values) ^ 1;

    let mut checksum = Vec::with_capacity(6);
    for i in 0..6 {
        checksum.push(((polymod >> (5 * (5 - i))) & 31) as u8);
    }

    Ok(checksum)
}

/// Expand HRP for checksum calculation
fn expand_hrp(hrp: &str) -> Vec<u8> {
    let mut result = Vec::with_capacity(hrp.len() * 2 + 1);

    for c in hrp.chars() {
        result.push((c as u8) >> 5);
    }
    result.push(0);
    for c in hrp.chars() {
        result.push((c as u8) & 31);
    }

    result
}

/// Bech32 polymod function
fn bech32_polymod(values: &[u8]) -> u32 {
    const GENERATOR: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];

    let mut chk: u32 = 1;
    for &v in values {
        let top = chk >> 25;
        chk = ((chk & 0x01ff_ffff) << 5) ^ (v as u32);
        for (i, gen) in GENERATOR.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= gen;
            }
        }
    }
    chk
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
        // This is a valid Terra address
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
        assert_eq!(recovered.to_lowercase(), terra_addr.to_lowercase());
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
        let mut bytes32: [u8; 32] = [0u8; 32];
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

    // ========================================================================
    // Regression Tests — lock in exact byte-level behavior before refactoring
    // ========================================================================

    const REGRESSION_EVM_ADDR: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    const REGRESSION_TERRA_ADDR: &str = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";

    #[rustfmt::skip]
    const REGRESSION_EVM_BYTES32: [u8; 32] = [
        0x00, 0x00, 0x00, 0x01,
        0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce,
        0x6a, 0xb8, 0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[rustfmt::skip]
    const REGRESSION_COSMOS_BYTES32: [u8; 32] = [
        0x00, 0x00, 0x00, 0x02,
        0x35, 0x74, 0x30, 0x74, 0x95, 0x6c, 0x71, 0x08, 0x00, 0xe8,
        0x31, 0x98, 0x01, 0x1c, 0xcb, 0xd4, 0xdd, 0xf1, 0x55, 0x6d,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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

        assert_eq!(&b[0..4], &[0x00, 0x00, 0x00, 0x01]);
        assert_eq!(&b[4..24], addr.raw_address_bytes());
        assert_eq!(&b[24..32], &[0u8; 8]);
    }

    #[test]
    fn regression_bytes32_layout_cosmos() {
        let addr = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        let b = addr.to_bytes32();

        assert_eq!(&b[0..4], &[0x00, 0x00, 0x00, 0x02]);
        assert_eq!(&b[4..24], addr.raw_address_bytes());
        assert_eq!(&b[24..32], &[0u8; 8]);
    }

    #[test]
    fn regression_strict_validation() {
        let mut bytes = REGRESSION_EVM_BYTES32;
        bytes[31] = 0xff;
        assert!(UniversalAddress::from_bytes32_strict(&bytes).is_err());
        assert!(UniversalAddress::from_bytes32_strict(&REGRESSION_EVM_BYTES32).is_ok());
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
        assert!(!unknown.is_valid_chain_type());
    }

    #[test]
    fn regression_new_with_reserved() {
        let reserved = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let raw = [0xaau8; 20];
        let addr = UniversalAddress::new_with_reserved(CHAIN_TYPE_EVM, raw, reserved).unwrap();
        let b = addr.to_bytes32();
        assert_eq!(&b[24..32], &reserved);
    }

    #[test]
    fn regression_from_addr() {
        let addr = Addr::unchecked(REGRESSION_TERRA_ADDR);
        let from_addr = UniversalAddress::from_addr(&addr).unwrap();
        let from_cosmos = UniversalAddress::from_cosmos(REGRESSION_TERRA_ADDR).unwrap();
        assert_eq!(from_addr, from_cosmos);
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
}
