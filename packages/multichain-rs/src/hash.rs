//! Hash computation for cross-chain transfer IDs
//!
//! This module provides hash computation functions that match the EVM and
//! Terra contract implementations for verifying transfer identities.
//!
//! ## V2 Format (7-field unified hash)
//!
//! V2 uses 4-byte chain IDs and a unified 7-field hash for both deposits and withdrawals:
//! ```text
//! transferHash = keccak256(abi.encode(
//!     bytes32(srcChain),   // 4 bytes -> padded to 32
//!     bytes32(destChain),  // 4 bytes -> padded to 32
//!     srcAccount,          // bytes32
//!     destAccount,         // bytes32
//!     token,               // bytes32
//!     uint256(amount),     // 32 bytes
//!     uint256(nonce)       // 32 bytes
//! ))
//! // Total: 7 * 32 = 224 bytes (abi.encode padding)
//! ```

use tiny_keccak::{Hasher, Keccak};

// Re-export address codec functions for convenience
pub use crate::address_codec::{
    decode_bech32_address, encode_bech32_address, encode_evm_address, parse_evm_address,
};

/// Compute keccak256 hash of data
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

// ============================================================================
// V2 Hash Functions (7-field unified, abi.encode compatible)
// ============================================================================

/// Compute unified transfer hash for V2 format (matches HashLib.sol computeTransferHash)
///
/// Both deposits and withdrawals use the same 7-field hash so they produce
/// identical hashes for the same transfer, enabling cross-chain verification.
///
/// ```solidity
/// keccak256(abi.encode(
///     bytes32(srcChain), bytes32(destChain), srcAccount, destAccount, token, amount, uint256(nonce)
/// ))
/// ```
///
/// Uses `abi.encode` padding (each field padded to 32 bytes = 224 bytes total).
///
/// On deposit (source chain):
///   srcChain = thisChainId, srcAccount = msg.sender, destChain/destAccount/token from params
/// On withdraw (dest chain):
///   srcChain/srcAccount from params, destChain = thisChainId, destAccount/token from params
pub fn compute_transfer_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    // abi.encode layout: 7 * 32 = 224 bytes (each field padded to 32 bytes)
    let mut data = [0u8; 224];

    // srcChain (bytes4 -> left-aligned in bytes32, rest zero-padded)
    data[0..4].copy_from_slice(src_chain);
    // bytes 4..32 remain zero

    // destChain (bytes4 -> left-aligned in bytes32, rest zero-padded)
    data[32..36].copy_from_slice(dest_chain);
    // bytes 36..64 remain zero

    // srcAccount (32 bytes)
    data[64..96].copy_from_slice(src_account);

    // destAccount (32 bytes)
    data[96..128].copy_from_slice(dest_account);

    // token (32 bytes)
    data[128..160].copy_from_slice(token);

    // amount (uint256 as 32 bytes, big-endian, left-padded)
    let amount_bytes = amount.to_be_bytes();
    data[160 + 16..192].copy_from_slice(&amount_bytes);

    // nonce (uint64 -> uint256 as 32 bytes, big-endian, left-padded)
    let nonce_bytes = nonce.to_be_bytes();
    data[192 + 24..224].copy_from_slice(&nonce_bytes);

    keccak256(&data)
}

/// Compute deposit hash (alias for compute_transfer_hash)
///
/// On the source chain, call with:
/// - src_chain = this chain's bytes4 ID
/// - src_account = depositor's address (msg.sender encoded as bytes32)
/// - dest_chain, dest_account, token = from deposit parameters
pub fn compute_deposit_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    compute_transfer_hash(
        src_chain,
        dest_chain,
        src_account,
        dest_account,
        token,
        amount,
        nonce,
    )
}

/// Compute withdraw hash (alias for compute_transfer_hash)
///
/// On the destination chain, call with:
/// - src_chain, src_account = from withdraw parameters (source chain info)
/// - dest_chain = this chain's bytes4 ID
/// - dest_account = recipient's address (from withdraw parameters)
/// - token = local token address
pub fn compute_withdraw_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    compute_transfer_hash(
        src_chain,
        dest_chain,
        src_account,
        dest_account,
        token,
        amount,
        nonce,
    )
}

/// Convert an EVM address to bytes32 (left-padded with zeros)
pub fn address_to_bytes32(addr: &[u8; 20]) -> [u8; 32] {
    let mut result = [0u8; 32];
    result[12..32].copy_from_slice(addr);
    result
}

/// Extract raw 20-byte address from bytes32
pub fn bytes32_to_address(bytes: &[u8; 32]) -> [u8; 20] {
    let mut result = [0u8; 20];
    result.copy_from_slice(&bytes[12..32]);
    result
}

/// Encode a Terra bech32 address to bytes32 for EVM contracts
///
/// Decodes the bech32 to raw 20 bytes, then left-pads with zeros to 32 bytes.
pub fn encode_terra_address_to_bytes32(addr: &str) -> Result<[u8; 32], String> {
    let (raw_bytes, hrp) = decode_bech32_address(addr).map_err(|e| e.to_string())?;

    if hrp != "terra" {
        return Err(format!("Expected 'terra' prefix, got '{}'", hrp));
    }

    Ok(address_to_bytes32(&raw_bytes))
}

/// Decode bytes32 to a Terra bech32 address
pub fn decode_bytes32_to_terra_address(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 20 {
        return Err(format!("Invalid bytes length: {}", bytes.len()));
    }

    let mut arr = [0u8; 32];
    if bytes.len() >= 32 {
        arr.copy_from_slice(&bytes[..32]);
    } else {
        arr[32 - bytes.len()..].copy_from_slice(bytes);
    }

    let raw = bytes32_to_address(&arr);
    encode_bech32_address(&raw, "terra").map_err(|e| e.to_string())
}

/// Convert bytes to hex string with 0x prefix
pub fn bytes32_to_hex(bytes: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(66);
    hex.push_str("0x");
    for byte in bytes {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

/// Convert 4-byte array to hex string with 0x prefix
pub fn bytes4_to_hex(bytes: &[u8; 4]) -> String {
    let mut hex = String::with_capacity(10);
    hex.push_str("0x");
    for byte in bytes {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keccak256() {
        let result = keccak256(b"hello");
        assert_eq!(
            bytes32_to_hex(&result),
            "0x1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8"
        );
    }

    #[test]
    fn test_compute_transfer_hash() {
        let src_chain: [u8; 4] = [0, 0, 0, 1]; // Chain ID 1
        let dest_chain: [u8; 4] = [0, 0, 0, 2]; // Chain ID 2
        let src_account = [0u8; 32];
        let dest_account = [0u8; 32];
        let token = [0u8; 32];
        let amount: u128 = 1_000_000;
        let nonce: u64 = 1;

        let hash = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            amount,
            nonce,
        );

        // Verify hash is computed
        assert_eq!(hash.len(), 32);

        // Same inputs should produce same hash
        let hash2 = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            amount,
            nonce,
        );
        assert_eq!(hash, hash2);

        // Different inputs should produce different hash
        let hash3 = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            amount,
            2, // Different nonce
        );
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_deposit_and_withdraw_hash_match() {
        // The deposit hash on the source chain and the withdraw hash on the dest chain
        // should produce the same hash for the same transfer.
        let src_chain: [u8; 4] = [0, 0, 0, 1];
        let dest_chain: [u8; 4] = [0, 0, 0, 2];
        let mut src_account = [0u8; 32];
        src_account[31] = 0xAA;
        let mut dest_account = [0u8; 32];
        dest_account[31] = 0xBB;
        let mut token = [0u8; 32];
        token[31] = 0xCC;
        let amount: u128 = 1_000_000;
        let nonce: u64 = 42;

        let deposit_hash = compute_deposit_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            amount,
            nonce,
        );
        let withdraw_hash = compute_withdraw_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            amount,
            nonce,
        );

        assert_eq!(
            deposit_hash, withdraw_hash,
            "Deposit and withdraw hashes must match for cross-chain verification"
        );
    }

    #[test]
    fn test_address_to_bytes32_roundtrip() {
        let addr: [u8; 20] = [
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ];

        let bytes32 = address_to_bytes32(&addr);
        let recovered = bytes32_to_address(&bytes32);

        assert_eq!(addr, recovered);
    }

    #[test]
    fn test_bytes4_to_hex() {
        let bytes: [u8; 4] = [0, 0, 0, 1];
        assert_eq!(bytes4_to_hex(&bytes), "0x00000001");

        let bytes2: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
        assert_eq!(bytes4_to_hex(&bytes2), "0x12345678");
    }
}
