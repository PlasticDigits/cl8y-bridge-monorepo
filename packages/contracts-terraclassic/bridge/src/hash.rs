//! Hash computation module for cross-chain verification
//!
//! This module provides canonical hash computation that produces identical output
//! to the EVM contract's `_computeTransferId` function.
//!
//! # EVM Reference
//! ```solidity
//! function _computeTransferId(
//!     bytes32 srcChainKey,
//!     bytes32 destChainKey,
//!     bytes32 destTokenAddress,
//!     bytes32 destAccount,
//!     uint256 amount,
//!     uint256 nonce
//! ) internal pure returns (bytes32) {
//!     return keccak256(abi.encode(...));
//! }
//! ```
//!
//! # Byte Layout (192 bytes total)
//! - Bytes 0-31:    srcChainKey (32 bytes)
//! - Bytes 32-63:   destChainKey (32 bytes)
//! - Bytes 64-95:   destTokenAddress (32 bytes)
//! - Bytes 96-127:  destAccount (32 bytes)
//! - Bytes 128-159: amount (uint256, big-endian, left-padded)
//! - Bytes 160-191: nonce (uint256, big-endian, left-padded)

use cosmwasm_std::{Addr, Deps, StdResult};
use tiny_keccak::{Hasher, Keccak};

/// Terra Classic chain ID for chain key computation
pub const TERRA_CLASSIC_CHAIN_ID: &str = "columbus-5";

/// Compute keccak256 hash of arbitrary data
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Compute the canonical transferId hash matching EVM's _computeTransferId
///
/// This function produces identical output to the Solidity:
/// `keccak256(abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce))`
///
/// # Arguments
/// * `src_chain_key` - 32-byte source chain identifier
/// * `dest_chain_key` - 32-byte destination chain identifier
/// * `dest_token_address` - 32-byte destination token address
/// * `dest_account` - 32-byte destination account
/// * `amount` - Transfer amount (u128, will be left-padded to 32 bytes)
/// * `nonce` - Unique nonce (u64, will be left-padded to 32 bytes)
///
/// # Returns
/// 32-byte keccak256 hash
pub fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    dest_token_address: &[u8; 32],
    dest_account: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    // Pre-allocate exact size: 6 * 32 = 192 bytes
    let mut data = [0u8; 192];

    // Copy fixed-size values (first 4 slots)
    data[0..32].copy_from_slice(src_chain_key);
    data[32..64].copy_from_slice(dest_chain_key);
    data[64..96].copy_from_slice(dest_token_address);
    data[96..128].copy_from_slice(dest_account);

    // uint256 amount - left-padded to 32 bytes, big-endian
    // u128 (16 bytes) goes into bytes 16-31, bytes 0-15 remain zero
    let amount_bytes = amount.to_be_bytes();
    data[128 + 16..160].copy_from_slice(&amount_bytes);

    // uint256 nonce - left-padded to 32 bytes, big-endian
    // u64 (8 bytes) goes into bytes 24-31, bytes 0-23 remain zero
    let nonce_bytes = nonce.to_be_bytes();
    data[160 + 24..192].copy_from_slice(&nonce_bytes);

    keccak256(&data)
}

/// Compute chain key for EVM chains
///
/// Matches: `keccak256(abi.encode("EVM", bytes32(chainId)))`
///
/// # abi.encode layout for (string, bytes32):
/// - Bytes 0-31:   Offset to string data (0x40 = 64)
/// - Bytes 32-63:  bytes32 rawChainKey (chain_id left-padded)
/// - Bytes 64-95:  String length (3 for "EVM")
/// - Bytes 96-127: String data padded to 32 bytes
pub fn evm_chain_key(chain_id: u64) -> [u8; 32] {
    let mut data = [0u8; 128];

    // Offset to string data: 64 (0x40)
    data[31] = 64;

    // rawChainKey as bytes32 (chain_id left-padded to 32 bytes)
    let chain_id_bytes = chain_id.to_be_bytes();
    data[32 + 24..64].copy_from_slice(&chain_id_bytes);

    // String length: 3 ("EVM")
    data[64 + 31] = 3;

    // String data: "EVM" (ASCII bytes)
    data[96..99].copy_from_slice(b"EVM");

    keccak256(&data)
}

/// Compute chain key for Cosmos/CosmWasm chains
///
/// Matches: `keccak256(abi.encode("COSMW", keccak256(abi.encode(chainId))))`
///
/// # Process:
/// 1. Compute inner hash: `keccak256(abi.encode(chainId_string))`
/// 2. Compute outer hash: `keccak256(abi.encode("COSMW", inner_hash))`
pub fn cosmos_chain_key(chain_id: &str) -> [u8; 32] {
    // Step 1: Compute inner hash of abi.encode(string)
    let inner_hash = abi_encode_string_hash(chain_id);

    // Step 2: Compute outer hash with chain type "COSMW"
    abi_encode_chain_key("COSMW", &inner_hash)
}

/// Get the Terra Classic chain key (hardcoded for columbus-5)
pub fn terra_chain_key() -> [u8; 32] {
    cosmos_chain_key(TERRA_CLASSIC_CHAIN_ID)
}

/// Encode a Terra/Cosmos address as 32 bytes (left-padded)
///
/// Cosmos addresses are 20 bytes in canonical form.
/// We left-pad with zeros to 32 bytes to match EVM's address encoding.
pub fn encode_terra_address(deps: Deps, addr: &Addr) -> StdResult<[u8; 32]> {
    let canonical = deps.api.addr_canonicalize(addr.as_str())?;
    let bytes = canonical.as_slice();

    let mut result = [0u8; 32];
    // Left-pad: 20-byte address goes in last 20 bytes (positions 12-31)
    let start = 32 - bytes.len();
    result[start..].copy_from_slice(bytes);

    Ok(result)
}

/// Encode a token denom/address as 32 bytes
///
/// For native denoms: Returns keccak256 hash of the denom string
/// For CW20: Canonicalizes the address and left-pads to 32 bytes
pub fn encode_token_address(deps: Deps, token: &str) -> StdResult<[u8; 32]> {
    // Try to validate as address first
    if let Ok(addr) = deps.api.addr_validate(token) {
        encode_terra_address(deps, &addr)
    } else {
        // Native denom - hash the string
        Ok(keccak256(token.as_bytes()))
    }
}

/// Convert 32-byte hash to hex string (for attributes/logging)
pub fn bytes32_to_hex(bytes: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(66);
    hex.push_str("0x");
    for byte in bytes {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

/// Parse hex string (with or without 0x prefix) to 32-byte array
pub fn hex_to_bytes32(hex: &str) -> Result<[u8; 32], &'static str> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    if hex.len() != 64 {
        return Err("Invalid hex length: expected 64 characters");
    }

    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| "Invalid hex character")?;
    }
    Ok(result)
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Helper to compute keccak256(abi.encode(string))
fn abi_encode_string_hash(s: &str) -> [u8; 32] {
    // abi.encode for dynamic string:
    // - 32 bytes: offset (0x20 = 32)
    // - 32 bytes: length
    // - ceil(len/32)*32 bytes: data padded to 32-byte boundary

    let str_bytes = s.as_bytes();
    let len = str_bytes.len();
    let padded_len = ((len + 31) / 32) * 32;

    let total_size = 32 + 32 + padded_len;
    let mut data = vec![0u8; total_size];

    // Offset: 0x20 = 32
    data[31] = 32;

    // Length (as uint256, left-padded)
    data[32 + 24..64].copy_from_slice(&(len as u64).to_be_bytes());

    // String data
    data[64..64 + len].copy_from_slice(str_bytes);

    keccak256(&data)
}

/// Helper to compute chain key hash for (string chainType, bytes32 rawKey)
fn abi_encode_chain_key(chain_type: &str, raw_key: &[u8; 32]) -> [u8; 32] {
    // abi.encode(string, bytes32) layout:
    // - 32 bytes: offset to string (0x40 = 64)
    // - 32 bytes: bytes32 value
    // - 32 bytes: string length
    // - ceil(len/32)*32 bytes: string data padded

    let type_bytes = chain_type.as_bytes();
    let type_len = type_bytes.len();
    let padded_type_len = ((type_len + 31) / 32) * 32;

    let total_size = 64 + 32 + padded_type_len;
    let mut data = vec![0u8; total_size];

    // Offset to string: 64 (0x40)
    data[31] = 64;

    // bytes32 raw_key
    data[32..64].copy_from_slice(raw_key);

    // String length (as uint256, left-padded)
    data[64 + 24..96].copy_from_slice(&(type_len as u64).to_be_bytes());

    // String data
    data[96..96 + type_len].copy_from_slice(type_bytes);

    keccak256(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vector 1: All zeros - matches EVM HashVectors.t.sol testVector1_AllZeros
    #[test]
    fn test_vector1_all_zeros() {
        let result = compute_transfer_id(
            &[0u8; 32],
            &[0u8; 32],
            &[0u8; 32],
            &[0u8; 32],
            0,
            0,
        );

        assert_eq!(
            bytes32_to_hex(&result),
            "0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d"
        );
    }

    /// Vector 2: Simple values - matches EVM HashVectors.t.sol testVector2_SimpleValues
    #[test]
    fn test_vector2_simple_values() {
        // srcChainKey = 1, destChainKey = 2, destTokenAddress = 3, destAccount = 4
        let mut src_chain_key = [0u8; 32];
        src_chain_key[31] = 1;

        let mut dest_chain_key = [0u8; 32];
        dest_chain_key[31] = 2;

        let mut dest_token_address = [0u8; 32];
        dest_token_address[31] = 3;

        let mut dest_account = [0u8; 32];
        dest_account[31] = 4;

        let amount: u128 = 1_000_000_000_000_000_000; // 1e18
        let nonce: u64 = 42;

        let result = compute_transfer_id(
            &src_chain_key,
            &dest_chain_key,
            &dest_token_address,
            &dest_account,
            amount,
            nonce,
        );

        assert_eq!(
            bytes32_to_hex(&result),
            "0x7226dd6b664f0c50fb3e50adfa82057dab4819f592ef9d35c08b9c4531b05150"
        );
    }

    /// Vector 6: Maximum values - matches EVM HashVectors.t.sol testVector6_MaxValues
    #[test]
    fn test_vector6_max_values() {
        let result = compute_transfer_id(
            &[0xff; 32],
            &[0xff; 32],
            &[0xff; 32],
            &[0xff; 32],
            u128::MAX, // 340282366920938463463374607431768211455
            u64::MAX,  // 18446744073709551615
        );

        assert_eq!(
            bytes32_to_hex(&result),
            "0x8433decfd52c831dd32c2ce04d46812b4dc8c2ee057f1edae791837d230be522"
        );
    }

    /// Test amount encoding - verifies left-padding is correct
    #[test]
    fn test_amount_encoding() {
        let mut data = [0u8; 32];
        let amount: u128 = 1_000_000_000_000_000_000; // 1e18
        let amount_bytes = amount.to_be_bytes();
        data[16..32].copy_from_slice(&amount_bytes);

        // First 16 bytes should be zero (left-padding)
        assert_eq!(&data[0..16], &[0u8; 16]);
    }

    /// Test nonce encoding - verifies left-padding is correct
    #[test]
    fn test_nonce_encoding() {
        let mut data = [0u8; 32];
        let nonce: u64 = 42;
        let nonce_bytes = nonce.to_be_bytes();
        data[24..32].copy_from_slice(&nonce_bytes);

        // First 24 bytes should be zero (left-padding)
        assert_eq!(&data[0..24], &[0u8; 24]);
        // Last byte should be 42
        assert_eq!(data[31], 42);
    }

    /// Test hex conversion round-trip
    #[test]
    fn test_hex_roundtrip() {
        let original = [
            0x1e, 0x99, 0x0e, 0x27, 0xf0, 0xd7, 0x97, 0x6b,
            0xf2, 0xad, 0xbd, 0x60, 0xe2, 0x03, 0x84, 0xda,
            0x01, 0x25, 0xb7, 0x6e, 0x28, 0x85, 0xa9, 0x6a,
            0xa7, 0x07, 0xbc, 0xb0, 0x54, 0x10, 0x8b, 0x0d,
        ];

        let hex = bytes32_to_hex(&original);
        assert_eq!(hex, "0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d");

        let parsed = hex_to_bytes32(&hex).unwrap();
        assert_eq!(parsed, original);

        // Also test without 0x prefix
        let parsed_no_prefix = hex_to_bytes32(&hex[2..]).unwrap();
        assert_eq!(parsed_no_prefix, original);
    }

    /// Test keccak256 produces expected output for known input
    #[test]
    fn test_keccak256_basic() {
        // keccak256("hello") = 0x1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8
        let result = keccak256(b"hello");
        assert_eq!(
            bytes32_to_hex(&result),
            "0x1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8"
        );
    }

    /// Vector 3: BSC chain key - matches EVM HashVectors.t.sol testVector3_BSCChainKey
    #[test]
    fn test_vector3_bsc_chain_key() {
        let bsc_key = evm_chain_key(56);
        assert_eq!(
            bytes32_to_hex(&bsc_key),
            "0xe2debc38147727fd4c36e012d1d8335aebec2bcb98c3b1aae5dde65ddcd74367"
        );
    }

    /// Vector 4: Terra chain key - matches EVM HashVectors.t.sol testVector4_TerraChainKey
    #[test]
    fn test_vector4_terra_chain_key() {
        let terra_key = cosmos_chain_key("columbus-5");
        assert_eq!(
            bytes32_to_hex(&terra_key),
            "0x0ece70814ff48c843659d2c2cfd2138d070b75d11f9fd81e424873e90a47d8b3"
        );
    }

    /// Vector 5: Realistic BSC->Terra transfer - matches EVM HashVectors.t.sol testVector5_RealisticTransfer
    #[test]
    fn test_vector5_realistic_transfer() {
        let src_chain_key = evm_chain_key(56); // BSC
        let dest_chain_key = cosmos_chain_key("columbus-5"); // Terra

        // Token: USDT on BSC (0x55d398326f99059fF775485246999027B3197955) left-padded
        let dest_token_address = hex_to_bytes32("0x00000000000000000000000055d398326f99059ff775485246999027b3197955").unwrap();

        // Recipient: example address left-padded
        let dest_account = hex_to_bytes32("0x0000000000000000000000001234567890abcdef1234567890abcdef12345678").unwrap();

        let amount: u128 = 1_000_000; // 1 USDT (6 decimals)
        let nonce: u64 = 1;

        let result = compute_transfer_id(
            &src_chain_key,
            &dest_chain_key,
            &dest_token_address,
            &dest_account,
            amount,
            nonce,
        );

        assert_eq!(
            bytes32_to_hex(&result),
            "0x16ccad826b64971ab063989a5d66ef27a97e962f463ad917f76a4d2a313e2c79"
        );
    }

    /// Test terra_chain_key() helper matches cosmos_chain_key("columbus-5")
    #[test]
    fn test_terra_chain_key_helper() {
        let from_helper = terra_chain_key();
        let from_function = cosmos_chain_key("columbus-5");
        assert_eq!(from_helper, from_function);
    }
}
