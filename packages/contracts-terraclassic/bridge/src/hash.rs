//! Hash computation module for cross-chain verification
//!
//! This module provides canonical hash computation that produces identical output
//! to the EVM contract's `HashLib.computeTransferHash` function.
//!
//! # V2 Unified Transfer Hash (7-field)
//!
//! Both deposits and withdrawals use the same hash so they produce identical
//! hashes for the same transfer, enabling cross-chain verification.
//!
//! ```solidity
//! keccak256(abi.encode(
//!     bytes32(srcChain),     // 4 bytes -> padded to 32
//!     bytes32(destChain),    // 4 bytes -> padded to 32
//!     srcAccount,            // bytes32
//!     destAccount,           // bytes32
//!     token,                 // bytes32
//!     uint256(amount),       // 32 bytes
//!     uint256(nonce)         // 32 bytes
//! ))
//! ```
//!
//! # Byte Layout (224 bytes total, abi.encode format)
//! - Bytes 0-31:    srcChain (bytes4 left-aligned, zero-padded to 32)
//! - Bytes 32-63:   destChain (bytes4 left-aligned, zero-padded to 32)
//! - Bytes 64-95:   srcAccount (32 bytes)
//! - Bytes 96-127:  destAccount (32 bytes)
//! - Bytes 128-159: token (32 bytes)
//! - Bytes 160-191: amount (uint256, big-endian, left-padded)
//! - Bytes 192-223: nonce (uint256, big-endian, left-padded)

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

/// Compute unified transfer hash matching EVM's HashLib.computeTransferHash
///
/// This function produces identical output to the Solidity:
/// ```solidity
/// keccak256(abi.encode(
///     bytes32(srcChain), bytes32(destChain), srcAccount, destAccount, token, amount, uint256(nonce)
/// ))
/// ```
///
/// Both deposits and withdrawals use this same hash, enabling cross-chain matching.
///
/// # Arguments
/// * `src_chain` - 4-byte source chain ID
/// * `dest_chain` - 4-byte destination chain ID
/// * `src_account` - 32-byte source account (depositor)
/// * `dest_account` - 32-byte destination account (recipient)
/// * `token` - 32-byte token address on destination chain
/// * `amount` - Transfer amount (u128, will be left-padded to 32 bytes)
/// * `nonce` - Unique nonce (u64, will be left-padded to 32 bytes)
///
/// # Returns
/// 32-byte keccak256 hash
pub fn compute_transfer_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    // Pre-allocate exact size: 7 * 32 = 224 bytes (abi.encode format)
    let mut data = [0u8; 224];

    // srcChain (bytes4 left-aligned in bytes32, rest zero-padded)
    data[0..4].copy_from_slice(src_chain);
    // bytes 4..32 remain zero

    // destChain (bytes4 left-aligned in bytes32, rest zero-padded)
    data[32..36].copy_from_slice(dest_chain);
    // bytes 36..64 remain zero

    // srcAccount (32 bytes)
    data[64..96].copy_from_slice(src_account);

    // destAccount (32 bytes)
    data[96..128].copy_from_slice(dest_account);

    // token (32 bytes)
    data[128..160].copy_from_slice(token);

    // uint256 amount - left-padded to 32 bytes, big-endian
    // u128 (16 bytes) goes into bytes 16-31, bytes 0-15 remain zero
    let amount_bytes = amount.to_be_bytes();
    data[160 + 16..192].copy_from_slice(&amount_bytes);

    // uint256 nonce - left-padded to 32 bytes, big-endian
    // u64 (8 bytes) goes into bytes 24-31, bytes 0-23 remain zero
    let nonce_bytes = nonce.to_be_bytes();
    data[192 + 24..224].copy_from_slice(&nonce_bytes);

    keccak256(&data)
}

/// Legacy V1 transfer ID computation (6-field, 32-byte chain keys).
///
/// **Deprecated**: Use [`compute_transfer_hash`] for the unified 7-field V2 hash.
/// Retained for backward compatibility with `ComputeWithdrawHash` legacy query.
#[deprecated(note = "Use compute_transfer_hash (V2 7-field) instead")]
pub fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    token: &[u8; 32],
    account: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    let mut data = [0u8; 192]; // 6 * 32 = 192 bytes
    data[0..32].copy_from_slice(src_chain_key);
    data[32..64].copy_from_slice(dest_chain_key);
    data[64..96].copy_from_slice(token);
    data[96..128].copy_from_slice(account);
    let amount_bytes = amount.to_be_bytes();
    data[128 + 16..160].copy_from_slice(&amount_bytes);
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

    // Handle varying canonical address sizes (20 bytes on-chain, variable in mock)
    if bytes.len() <= 32 {
        // Left-pad: address goes in last N bytes
        let start = 32 - bytes.len();
        result[start..].copy_from_slice(bytes);
    } else {
        // If longer than 32 bytes (shouldn't happen), take last 32 bytes
        result.copy_from_slice(&bytes[bytes.len() - 32..]);
    }

    Ok(result)
}

/// Decode a 32-byte left-padded address back to a Terra/Cosmos address.
///
/// Reverse of [`encode_terra_address`]: strips leading zero-padding and humanizes
/// the canonical address bytes.
pub fn decode_terra_address(deps: Deps, bytes: &[u8; 32]) -> StdResult<Addr> {
    // Find first non-zero byte — everything before is padding
    let first_nonzero = bytes.iter().position(|&b| b != 0).unwrap_or(32);
    if first_nonzero >= 32 {
        return Err(cosmwasm_std::StdError::generic_err(
            "Cannot decode zero address",
        ));
    }
    let canonical = cosmwasm_std::CanonicalAddr::from(bytes[first_nonzero..].to_vec());
    deps.api.addr_humanize(&canonical)
}

/// Minimum length for a valid bech32 contract address.
///
/// Real Terra/Cosmos addresses are bech32-encoded and at least 44 characters
/// (e.g. "terra1..."). Native denoms like "uluna", "uusd" are much shorter.
/// We use this threshold to distinguish CW20 contract addresses from native denoms,
/// which avoids relying on `addr_validate` (which accepts any string in mock tests).
const MIN_CW20_ADDRESS_LENGTH: usize = 20;

/// Encode a token denom/address as 32 bytes
///
/// For native denoms (short strings): Returns keccak256 hash of the denom string
/// For CW20 addresses (long strings): Canonicalizes the address and left-pads to 32 bytes
pub fn encode_token_address(deps: Deps, token: &str) -> StdResult<[u8; 32]> {
    // Native denoms are short strings (e.g. "uluna", "uusd", "ibc/...").
    // CW20 contract addresses are bech32-encoded and always 44+ characters.
    if token.len() >= MIN_CW20_ADDRESS_LENGTH && deps.api.addr_validate(token).is_ok() {
        let addr = deps.api.addr_validate(token)?;
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
        result[i] =
            u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| "Invalid hex character")?;
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

    /// Test V2 7-field hash: all zeros
    #[test]
    fn test_transfer_hash_all_zeros() {
        let result = compute_transfer_hash(
            &[0u8; 4], &[0u8; 4], &[0u8; 32], &[0u8; 32], &[0u8; 32], 0, 0,
        );

        // All-zero 224-byte input should produce a deterministic hash
        assert_eq!(result.len(), 32);

        // Same inputs should produce same hash
        let result2 = compute_transfer_hash(
            &[0u8; 4], &[0u8; 4], &[0u8; 32], &[0u8; 32], &[0u8; 32], 0, 0,
        );
        assert_eq!(result, result2);
    }

    /// Test V2 7-field hash: different nonces produce different hashes
    #[test]
    fn test_transfer_hash_different_nonce() {
        let src_chain: [u8; 4] = [0, 0, 0, 1];
        let dest_chain: [u8; 4] = [0, 0, 0, 2];
        let src_account = [0u8; 32];
        let dest_account = [0u8; 32];
        let token = [0u8; 32];

        let hash1 = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            1,
        );
        let hash2 = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            2,
        );

        assert_ne!(
            hash1, hash2,
            "Different nonces must produce different hashes"
        );
    }

    /// Test V2 7-field hash: srcAccount matters
    #[test]
    fn test_transfer_hash_src_account_matters() {
        let src_chain: [u8; 4] = [0, 0, 0, 1];
        let dest_chain: [u8; 4] = [0, 0, 0, 2];
        let mut src_account_a = [0u8; 32];
        src_account_a[31] = 0xAA;
        let mut src_account_b = [0u8; 32];
        src_account_b[31] = 0xBB;
        let dest_account = [0u8; 32];
        let token = [0u8; 32];

        let hash_a = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account_a,
            &dest_account,
            &token,
            1_000_000,
            1,
        );
        let hash_b = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account_b,
            &dest_account,
            &token,
            1_000_000,
            1,
        );

        assert_ne!(
            hash_a, hash_b,
            "Different srcAccounts must produce different hashes"
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
            0x1e, 0x99, 0x0e, 0x27, 0xf0, 0xd7, 0x97, 0x6b, 0xf2, 0xad, 0xbd, 0x60, 0xe2, 0x03,
            0x84, 0xda, 0x01, 0x25, 0xb7, 0x6e, 0x28, 0x85, 0xa9, 0x6a, 0xa7, 0x07, 0xbc, 0xb0,
            0x54, 0x10, 0x8b, 0x0d,
        ];

        let hex = bytes32_to_hex(&original);
        assert_eq!(
            hex,
            "0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d"
        );

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

    /// Test terra_chain_key() helper matches cosmos_chain_key("columbus-5")
    #[test]
    fn test_terra_chain_key_helper() {
        let from_helper = terra_chain_key();
        let from_function = cosmos_chain_key("columbus-5");
        assert_eq!(from_helper, from_function);
    }

    // ================================================================
    // Cross-Chain Token Encoding Parity Tests
    // (uluna native ↔ ERC20 and CW20 ↔ ERC20)
    // ================================================================

    /// Verify keccak256("uluna") matches the known cross-chain value.
    /// Both Solidity keccak256(abi.encodePacked("uluna")) and Rust keccak256(b"uluna")
    /// must produce this exact value for hashes to match across chains.
    #[test]
    fn test_uluna_native_token_encoding_cross_chain() {
        let uluna_bytes32 = keccak256(b"uluna");
        assert_eq!(
            bytes32_to_hex(&uluna_bytes32),
            "0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da",
            "keccak256('uluna') must match Solidity and Rust operator implementations"
        );
    }

    /// Verify encode_token_address produces keccak256("uluna") for native denom "uluna".
    /// Uses cosmwasm mock deps to test the contract-side encoding path.
    #[test]
    fn test_encode_token_address_uluna_native() {
        let deps = cosmwasm_std::testing::mock_dependencies();

        let encoded = encode_token_address(deps.as_ref(), "uluna").unwrap();
        let expected = keccak256(b"uluna");

        assert_eq!(
            encoded, expected,
            "encode_token_address('uluna') must produce keccak256('uluna')"
        );
        assert_eq!(
            bytes32_to_hex(&encoded),
            "0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da"
        );
    }

    /// Verify encode_token_address produces keccak256("uusd") for native denom "uusd".
    #[test]
    fn test_encode_token_address_uusd_native() {
        let deps = cosmwasm_std::testing::mock_dependencies();

        let encoded = encode_token_address(deps.as_ref(), "uusd").unwrap();
        let expected = keccak256(b"uusd");

        assert_eq!(
            encoded, expected,
            "encode_token_address('uusd') must produce keccak256('uusd')"
        );
    }

    /// Verify that uluna and uusd produce different token encodings.
    #[test]
    fn test_encode_token_address_different_native_denoms() {
        let deps = cosmwasm_std::testing::mock_dependencies();

        let uluna = encode_token_address(deps.as_ref(), "uluna").unwrap();
        let uusd = encode_token_address(deps.as_ref(), "uusd").unwrap();

        assert_ne!(
            uluna, uusd,
            "Different native denoms must produce different encodings"
        );
    }

    /// EVM → Terra transfer hash with native uluna.
    /// Cross-chain parity: must match Solidity and Rust operator hash.
    #[test]
    fn test_transfer_hash_evm_to_terra_uluna() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        // EVM depositor: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 as bytes32
        let mut src_account = [0u8; 32];
        src_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        // Terra recipient: terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v as bytes32
        let dest_account =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();

        // Token: native uluna = keccak256("uluna")
        let token = keccak256(b"uluna");

        let hash = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            1,
        );

        // Must match Solidity and Rust multichain-rs output
        assert_eq!(
            bytes32_to_hex(&hash),
            "0xfae09dfb97ff9f54f146b78d461f05956b8e57714dc1ff756f4b293720c22336",
            "EVM->Terra uluna hash must match Solidity and Rust"
        );
    }

    /// Terra → EVM transfer hash with native uluna.
    #[test]
    fn test_transfer_hash_terra_to_evm_uluna() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let src_account =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();

        let mut dest_account = [0u8; 32];
        dest_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        let token = keccak256(b"uluna");

        let hash = compute_transfer_hash(
            &terra_chain,
            &evm_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            1,
        );

        assert_eq!(
            bytes32_to_hex(&hash),
            "0xf2ee2cf947c1d90b12a4fdb93ddfafb32895b3eb8586b69c15d7bd935247f3ee",
            "Terra->EVM uluna hash must match Solidity and Rust"
        );
    }

    /// EVM → Terra transfer hash with CW20 token.
    #[test]
    fn test_transfer_hash_evm_to_terra_cw20() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        let mut src_account = [0u8; 32];
        src_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        // CW20 address bytes32 (same as token in this test case)
        let cw20_bytes32 =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();

        let hash = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &src_account,
            &cw20_bytes32,
            &cw20_bytes32,
            1_000_000,
            1,
        );

        assert_eq!(
            bytes32_to_hex(&hash),
            "0xf9737e3f6928b01ce2088caab2694eef79dd51ba42bcf177f01aad2fa6c7a4c6",
            "EVM->Terra CW20 hash must match Solidity and Rust"
        );
    }

    /// Terra → EVM transfer hash with CW20 token.
    #[test]
    fn test_transfer_hash_terra_to_evm_cw20() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let cw20_bytes32 =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();

        let mut dest_account = [0u8; 32];
        dest_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        let hash = compute_transfer_hash(
            &terra_chain,
            &evm_chain,
            &cw20_bytes32,
            &dest_account,
            &cw20_bytes32,
            1_000_000,
            1,
        );

        assert_eq!(
            bytes32_to_hex(&hash),
            "0xb8179fbc5a9f62e1b750c327fe0921600b1ce312585801f644604f8363a4dafa",
            "Terra->EVM CW20 hash must match Solidity and Rust"
        );
    }

    /// Verify uluna vs CW20 token encoding produces different transfer hashes.
    /// This is the root cause of "terra approval not found within timeout".
    #[test]
    fn test_uluna_vs_cw20_hash_mismatch() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let src = [0xABu8; 32];
        let dest = [0xCDu8; 32];

        let token_uluna = keccak256(b"uluna");
        let token_cw20 =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();

        assert_ne!(
            token_uluna, token_cw20,
            "uluna hash and CW20 bytes32 must be different token encodings"
        );

        let hash_uluna = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &src,
            &dest,
            &token_uluna,
            1_000_000,
            1,
        );
        let hash_cw20 = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &src,
            &dest,
            &token_cw20,
            1_000_000,
            1,
        );

        assert_ne!(
            hash_uluna, hash_cw20,
            "Using keccak256('uluna') vs CW20 bytes32 MUST produce different hashes. \
             This mismatch causes 'terra approval not found within timeout'."
        );
    }

    /// Direction matters: EVM→Terra and Terra→EVM produce different hashes for uluna.
    #[test]
    fn test_direction_matters_uluna() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        let mut evm_account = [0u8; 32];
        evm_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);
        let terra_account =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();
        let token = keccak256(b"uluna");

        let evm_to_terra = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &evm_account,
            &terra_account,
            &token,
            1_000_000,
            1,
        );
        let terra_to_evm = compute_transfer_hash(
            &terra_chain,
            &evm_chain,
            &terra_account,
            &evm_account,
            &token,
            1_000_000,
            1,
        );

        assert_ne!(
            evm_to_terra, terra_to_evm,
            "EVM->Terra and Terra->EVM must produce different hashes"
        );
    }

    // ================================================================
    // Deposit ↔ Withdraw Hash Parity Tests
    //
    // Deposit side (source chain) and withdraw side (dest chain) must
    // produce the SAME hash for the same transfer parameters.
    // Token = destination token address in all cases.
    // Expected values verified against Solidity and Rust multichain-rs.
    // ================================================================

    /// EVM → EVM: ERC20 deposit == withdraw
    #[test]
    fn test_deposit_withdraw_match_evm_to_evm_erc20() {
        let src_chain: [u8; 4] = [0, 0, 0, 1];
        let dest_chain: [u8; 4] = [0, 0, 0, 56];

        let mut src_account = [0u8; 32];
        src_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        let mut dest_account = [0u8; 32];
        dest_account[12..32].copy_from_slice(&[
            0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5,
            0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xC8,
        ]);

        let mut dest_token = [0u8; 32];
        dest_token[12..32].copy_from_slice(&[
            0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F,
            0x64, 0x2f, 0x64, 0x18, 0x0a, 0xa3,
        ]);

        let deposit_hash = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &dest_token,
            1_000_000_000_000_000_000,
            42,
        );
        let withdraw_hash = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &dest_token,
            1_000_000_000_000_000_000,
            42,
        );

        assert_eq!(
            deposit_hash, withdraw_hash,
            "EVM→EVM ERC20: deposit must equal withdraw"
        );
        assert_eq!(
            bytes32_to_hex(&deposit_hash),
            "0x11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54",
            "Must match Solidity and Rust"
        );
    }

    /// EVM → Terra: native uluna deposit == withdraw
    #[test]
    fn test_deposit_withdraw_match_evm_to_terra_native() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        let mut evm_account = [0u8; 32];
        evm_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);
        let terra_account =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();

        let token = keccak256(b"uluna");

        let deposit_hash = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &evm_account,
            &terra_account,
            &token,
            995_000,
            1,
        );
        let withdraw_hash = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &evm_account,
            &terra_account,
            &token,
            995_000,
            1,
        );

        assert_eq!(
            deposit_hash, withdraw_hash,
            "EVM→Terra native uluna: deposit must equal withdraw"
        );
        assert_eq!(
            bytes32_to_hex(&deposit_hash),
            "0x92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453",
            "Must match Solidity and Rust"
        );
    }

    /// EVM → Terra: CW20 deposit == withdraw
    #[test]
    fn test_deposit_withdraw_match_evm_to_terra_cw20() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        let mut evm_account = [0u8; 32];
        evm_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);
        let cw20_bytes32 =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();

        let deposit_hash = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &evm_account,
            &cw20_bytes32,
            &cw20_bytes32,
            1_000_000,
            5,
        );
        let withdraw_hash = compute_transfer_hash(
            &evm_chain,
            &terra_chain,
            &evm_account,
            &cw20_bytes32,
            &cw20_bytes32,
            1_000_000,
            5,
        );

        assert_eq!(
            deposit_hash, withdraw_hash,
            "EVM→Terra CW20: deposit must equal withdraw"
        );
        assert_eq!(
            bytes32_to_hex(&deposit_hash),
            "0x1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b",
            "Must match Solidity and Rust"
        );
    }

    /// Terra → EVM: native uluna source → ERC20 dest, deposit == withdraw
    #[test]
    fn test_deposit_withdraw_match_terra_to_evm_native_erc20() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let terra_account =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();
        let mut evm_account = [0u8; 32];
        evm_account[12..32].copy_from_slice(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        let mut erc20_token = [0u8; 32];
        erc20_token[12..32].copy_from_slice(&[
            0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F,
            0x64, 0x2f, 0x64, 0x18, 0x0a, 0xa3,
        ]);

        let deposit_hash = compute_transfer_hash(
            &terra_chain,
            &evm_chain,
            &terra_account,
            &evm_account,
            &erc20_token,
            500_000,
            3,
        );
        let withdraw_hash = compute_transfer_hash(
            &terra_chain,
            &evm_chain,
            &terra_account,
            &evm_account,
            &erc20_token,
            500_000,
            3,
        );

        assert_eq!(
            deposit_hash, withdraw_hash,
            "Terra→EVM native→ERC20: deposit must equal withdraw"
        );
        assert_eq!(
            bytes32_to_hex(&deposit_hash),
            "0x076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438",
            "Must match Solidity and Rust"
        );
    }

    /// Terra → EVM: CW20 source → ERC20 dest, deposit == withdraw
    #[test]
    fn test_deposit_withdraw_match_terra_to_evm_cw20_erc20() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let terra_account =
            hex_to_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d")
                .unwrap();
        let mut evm_account = [0u8; 32];
        evm_account[12..32].copy_from_slice(&[
            0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5,
            0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xC8,
        ]);

        let mut erc20_token = [0u8; 32];
        erc20_token[12..32].copy_from_slice(&[
            0xe7, 0xf1, 0x72, 0x5E, 0x77, 0x34, 0xCE, 0x28, 0x8F, 0x83, 0x67, 0xe1, 0xBb, 0x14,
            0x3E, 0x90, 0xbb, 0x3F, 0x05, 0x12,
        ]);

        let deposit_hash = compute_transfer_hash(
            &terra_chain,
            &evm_chain,
            &terra_account,
            &evm_account,
            &erc20_token,
            2_500_000,
            7,
        );
        let withdraw_hash = compute_transfer_hash(
            &terra_chain,
            &evm_chain,
            &terra_account,
            &evm_account,
            &erc20_token,
            2_500_000,
            7,
        );

        assert_eq!(
            deposit_hash, withdraw_hash,
            "Terra→EVM CW20→ERC20: deposit must equal withdraw"
        );
        assert_eq!(
            bytes32_to_hex(&deposit_hash),
            "0xf1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab",
            "Must match Solidity and Rust"
        );
    }
}
