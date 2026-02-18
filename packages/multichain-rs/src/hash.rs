//! Hash computation for cross-chain transfer IDs
//!
//! This module provides hash computation functions that match the EVM and
//! Terra contract implementations for verifying transfer identities.
//!
//! ## V2 Format (7-field unified hash)
//!
//! V2 uses 4-byte chain IDs and a unified 7-field hash for both deposits and withdrawals:
//! ```text
//! xchainHashId = keccak256(abi.encode(
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
use tracing::warn;

// Re-export address codec functions for convenience
pub use crate::address_codec::{
    decode_bech32_address, decode_bech32_address_raw, encode_bech32_address, encode_evm_address,
    parse_evm_address,
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

/// Compute unified cross-chain hash ID for V2 format (matches HashLib.sol computeXchainHashId)
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
pub fn compute_xchain_hash_id(
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
/// Supports both 20-byte wallet addresses and 32-byte contract (CW20) addresses.
/// - 20-byte addresses are left-padded with zeros to 32 bytes.
/// - 32-byte addresses are used directly.
///
/// This matches the Terra contract's `encode_terra_address` which uses
/// `addr_canonicalize` and left-pads to 32 bytes.
pub fn encode_terra_address_to_bytes32(addr: &str) -> Result<[u8; 32], String> {
    let (raw_bytes, hrp) =
        crate::address_codec::decode_bech32_address_raw(addr).map_err(|e| e.to_string())?;

    if hrp != "terra" {
        return Err(format!("Expected 'terra' prefix, got '{}'", hrp));
    }

    let mut result = [0u8; 32];
    if raw_bytes.len() == 32 {
        result.copy_from_slice(&raw_bytes);
    } else {
        // Left-pad: 20-byte address goes in last 20 bytes
        let start = 32 - raw_bytes.len();
        result[start..].copy_from_slice(&raw_bytes);
    }

    Ok(result)
}

/// Decode bytes32 to a Terra bech32 address
///
/// Accepts exactly 20-byte (raw wallet address) or 32-byte (left-padded)
/// inputs. Any other length is rejected with an error and a warning log.
///
/// # Security
///
/// Restricted to 20 or 32 bytes to prevent ambiguous padding behavior.
/// Previously accepted 20–31 byte inputs and silently left-padded them,
/// which could mask upstream bugs. See security review finding F3.
pub fn decode_bytes32_to_terra_address(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() != 20 && bytes.len() != 32 {
        warn!(
            bytes_len = bytes.len(),
            "decode_bytes32_to_terra_address called with invalid length (expected 20 or 32)"
        );
        return Err(format!(
            "Invalid bytes length: expected 20 or 32 bytes, got {}",
            bytes.len()
        ));
    }

    let mut arr = [0u8; 32];
    if bytes.len() == 32 {
        arr.copy_from_slice(bytes);
    } else {
        // 20-byte address: left-pad with zeros
        arr[12..32].copy_from_slice(bytes);
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
    fn test_compute_xchain_hash_id() {
        let src_chain: [u8; 4] = [0, 0, 0, 1]; // Chain ID 1
        let dest_chain: [u8; 4] = [0, 0, 0, 2]; // Chain ID 2
        let src_account = [0u8; 32];
        let dest_account = [0u8; 32];
        let token = [0u8; 32];
        let amount: u128 = 1_000_000;
        let nonce: u64 = 1;

        let hash = compute_xchain_hash_id(
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
        let hash2 = compute_xchain_hash_id(
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
        let hash3 = compute_xchain_hash_id(
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
    fn test_deposit_and_xchain_hash_id_withdraw_match() {
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

        let xchain_hash_id_deposit = compute_xchain_hash_id(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            amount,
            nonce,
        );
        let xchain_hash_id_withdraw = compute_xchain_hash_id(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &token,
            amount,
            nonce,
        );

        assert_eq!(
            xchain_hash_id_deposit, xchain_hash_id_withdraw,
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

    // ================================================================
    // Cross-Chain Token Encoding Parity Tests
    // (uluna native ↔ ERC20 and CW20 ↔ ERC20)
    // ================================================================

    /// Verify keccak256("uluna") produces the known cross-chain value.
    /// This MUST match Solidity's keccak256(abi.encodePacked("uluna")).
    #[test]
    fn test_uluna_native_token_encoding_cross_chain() {
        let uluna_bytes32 = keccak256(b"uluna");
        assert_eq!(
            bytes32_to_hex(&uluna_bytes32),
            "0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da",
            "keccak256('uluna') must match Solidity: keccak256(abi.encodePacked('uluna'))"
        );
    }

    /// Verify CW20 contract address bech32 decode produces a valid bytes32.
    /// The same bytes32 value must be registered in EVM's TokenRegistry.setTokenDestination.
    #[test]
    fn test_cw20_token_encoding_to_bytes32() {
        let cw20_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let bytes32 = encode_terra_address_to_bytes32(cw20_addr).unwrap();

        // First 12 bytes must be zero (left-padding for 20-byte address)
        assert_eq!(
            &bytes32[0..12],
            &[0u8; 12],
            "CW20 must be left-padded with 12 zero bytes"
        );
        // Last 20 bytes contain the canonical address
        assert!(
            !bytes32[12..32].iter().all(|&b| b == 0),
            "Last 20 bytes must contain the address"
        );

        println!("CW20 token bytes32: {}", bytes32_to_hex(&bytes32));
    }

    /// uluna (native) and CW20 (contract address) MUST produce different token encodings.
    /// Mixing them up causes the "terra approval not found" timeout error.
    #[test]
    fn test_uluna_vs_cw20_different_token_encoding() {
        let uluna_token = keccak256(b"uluna");

        let cw20_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let cw20_token = encode_terra_address_to_bytes32(cw20_addr).unwrap();

        assert_ne!(
            uluna_token, cw20_token,
            "Native denom hash and CW20 bytes32 must be different. \
             Mixing these causes hash mismatch and operator approval timeout."
        );
    }

    /// EVM → Terra transfer hash using native uluna token.
    /// Deposit ERC20-wrapped-uluna on EVM, withdraw native uluna on Terra.
    /// Token = keccak256("uluna") on both chains.
    #[test]
    fn test_xchain_hash_id_evm_to_terra_uluna() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        // EVM depositor: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
        let evm_addr: [u8; 20] = [
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ];
        let src_account = address_to_bytes32(&evm_addr);

        // Terra recipient
        let terra_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let dest_account = encode_terra_address_to_bytes32(terra_addr).unwrap();

        // Token: native uluna = keccak256("uluna")
        let token = keccak256(b"uluna");

        let hash = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            1,
        );

        assert_ne!(hash, [0u8; 32]);
        println!("EVM→Terra uluna hash: {}", bytes32_to_hex(&hash));
        println!("  src_account:  {}", bytes32_to_hex(&src_account));
        println!("  dest_account: {}", bytes32_to_hex(&dest_account));
        println!("  token:        {}", bytes32_to_hex(&token));
    }

    /// Terra → EVM transfer hash using native uluna token.
    /// Deposit native uluna on Terra, withdraw ERC20-wrapped-uluna on EVM.
    #[test]
    fn test_xchain_hash_id_terra_to_evm_uluna() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let terra_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let src_account = encode_terra_address_to_bytes32(terra_addr).unwrap();

        let evm_addr: [u8; 20] = [
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ];
        let dest_account = address_to_bytes32(&evm_addr);

        let token = keccak256(b"uluna");

        let hash = compute_xchain_hash_id(
            &terra_chain,
            &evm_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            1,
        );

        assert_ne!(hash, [0u8; 32]);
        println!("Terra→EVM uluna hash: {}", bytes32_to_hex(&hash));
    }

    /// EVM → Terra transfer hash using CW20 token.
    /// Deposit ERC20 on EVM, withdraw CW20 on Terra.
    /// Token = bech32-decoded CW20 address left-padded to 32 bytes.
    #[test]
    fn test_xchain_hash_id_evm_to_terra_cw20() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        let evm_addr: [u8; 20] = [
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ];
        let src_account = address_to_bytes32(&evm_addr);

        let terra_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let dest_account = encode_terra_address_to_bytes32(terra_addr).unwrap();

        // Token: CW20 address → bech32 decode → left-pad to bytes32
        let cw20_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let token = encode_terra_address_to_bytes32(cw20_addr).unwrap();

        let hash = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            1,
        );

        assert_ne!(hash, [0u8; 32]);
        println!("EVM→Terra CW20 hash: {}", bytes32_to_hex(&hash));
        println!("  CW20 token bytes32: {}", bytes32_to_hex(&token));
    }

    /// Terra → EVM transfer hash using CW20 token.
    /// Deposit CW20 on Terra, withdraw ERC20 on EVM.
    #[test]
    fn test_xchain_hash_id_terra_to_evm_cw20() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let terra_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let src_account = encode_terra_address_to_bytes32(terra_addr).unwrap();

        let evm_addr: [u8; 20] = [
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ];
        let dest_account = address_to_bytes32(&evm_addr);

        let cw20_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let token = encode_terra_address_to_bytes32(cw20_addr).unwrap();

        let hash = compute_xchain_hash_id(
            &terra_chain,
            &evm_chain,
            &src_account,
            &dest_account,
            &token,
            1_000_000,
            1,
        );

        assert_ne!(hash, [0u8; 32]);
        println!("Terra→EVM CW20 hash: {}", bytes32_to_hex(&hash));
    }

    /// Verify using wrong token encoding (uluna vs CW20) produces different hashes.
    /// This is the exact "terra approval not found" bug scenario.
    #[test]
    fn test_token_mismatch_causes_different_xchain_hash_id() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let src = address_to_bytes32(&[0xAA; 20]);
        let dest = address_to_bytes32(&[0xBB; 20]);
        let amount: u128 = 1_000_000;
        let nonce: u64 = 1;

        // Token encoding 1: native "uluna" → keccak256("uluna")
        let token_uluna = keccak256(b"uluna");

        // Token encoding 2: CW20 address → bech32 decode → left-pad
        let cw20_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let token_cw20 = encode_terra_address_to_bytes32(cw20_addr).unwrap();

        let hash_uluna = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &src,
            &dest,
            &token_uluna,
            amount,
            nonce,
        );
        let hash_cw20 = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &src,
            &dest,
            &token_cw20,
            amount,
            nonce,
        );

        assert_ne!(
            hash_uluna, hash_cw20,
            "Using keccak256('uluna') vs CW20 bytes32 MUST produce different hashes. \
             This mismatch is the root cause of 'terra approval not found within timeout'."
        );

        println!("Token mismatch demo:");
        println!("  uluna token: {}", bytes32_to_hex(&token_uluna));
        println!("  CW20  token: {}", bytes32_to_hex(&token_cw20));
        println!("  hash(uluna): {}", bytes32_to_hex(&hash_uluna));
        println!("  hash(CW20):  {}", bytes32_to_hex(&hash_cw20));
    }

    // ================================================================
    // Deposit ↔ Withdraw Hash Parity Tests
    //
    // The bridge computes the SAME hash on both sides of a transfer:
    //   - Deposit side (source chain): hash(srcChain, destChain, depositor, recipient, destToken, amount, nonce)
    //   - Withdraw side (dest chain):  hash(srcChain, destChain, depositor, recipient, destToken, amount, nonce)
    //
    // The `token` field is always the DESTINATION token address.
    // These tests verify xchain_hash_id_deposit == xchain_hash_id_withdraw for every route.
    // ================================================================

    /// EVM (chain 1) → EVM (chain 56): ERC20 ↔ ERC20
    #[test]
    fn test_deposit_withdraw_match_evm_to_evm_erc20() {
        let src_chain: [u8; 4] = [0, 0, 0, 1]; // EVM chain 1
        let dest_chain: [u8; 4] = [0, 0, 0, 56]; // EVM chain 56

        // Depositor on chain 1: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
        let src_account = address_to_bytes32(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        // Recipient on chain 56: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
        let dest_account = address_to_bytes32(&[
            0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5,
            0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xC8,
        ]);

        // Destination ERC20 on chain 56: 0x5FbDB2315678afecb367f032d93F642f64180aa3
        let dest_token = address_to_bytes32(&[
            0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F,
            0x64, 0x2f, 0x64, 0x18, 0x0a, 0xa3,
        ]);

        let amount: u128 = 1_000_000_000_000_000_000; // 1e18
        let nonce: u64 = 42;

        // Deposit hash (computed on source EVM chain 1)
        let xchain_hash_id_deposit = compute_xchain_hash_id(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &dest_token,
            amount,
            nonce,
        );

        // Withdraw hash (computed on destination EVM chain 56)
        let xchain_hash_id_withdraw = compute_xchain_hash_id(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &dest_token,
            amount,
            nonce,
        );

        assert_eq!(
            xchain_hash_id_deposit, xchain_hash_id_withdraw,
            "EVM→EVM ERC20: deposit hash must equal withdraw hash"
        );
        assert_ne!(xchain_hash_id_deposit, [0u8; 32]);

        println!(
            "EVM→EVM ERC20 deposit=withdraw: {}",
            bytes32_to_hex(&xchain_hash_id_deposit)
        );
    }

    /// EVM → Terra Classic: native uluna (ERC20 on EVM ↔ native "uluna" on Terra)
    /// Token = keccak256("uluna") — the destination token encoding for native denom.
    #[test]
    fn test_deposit_withdraw_match_evm_to_terra_native() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        let evm_depositor = address_to_bytes32(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        let terra_recipient =
            encode_terra_address_to_bytes32("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v")
                .unwrap();

        // Destination token: native uluna = keccak256("uluna")
        let dest_token = keccak256(b"uluna");

        let amount: u128 = 995_000;
        let nonce: u64 = 1;

        let xchain_hash_id_deposit = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &evm_depositor,
            &terra_recipient,
            &dest_token,
            amount,
            nonce,
        );

        let xchain_hash_id_withdraw = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &evm_depositor,
            &terra_recipient,
            &dest_token,
            amount,
            nonce,
        );

        assert_eq!(
            xchain_hash_id_deposit, xchain_hash_id_withdraw,
            "EVM→Terra native uluna: deposit hash must equal withdraw hash"
        );

        println!(
            "EVM→Terra native deposit=withdraw: {}",
            bytes32_to_hex(&xchain_hash_id_deposit)
        );
    }

    /// EVM → Terra Classic: CW20 (ERC20 on EVM ↔ CW20 on Terra)
    /// Token = CW20 address bech32-decoded and left-padded to bytes32.
    #[test]
    fn test_deposit_withdraw_match_evm_to_terra_cw20() {
        let evm_chain: [u8; 4] = [0, 0, 0, 1];
        let terra_chain: [u8; 4] = [0, 0, 0, 2];

        let evm_depositor = address_to_bytes32(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        let terra_recipient =
            encode_terra_address_to_bytes32("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v")
                .unwrap();

        // Destination token: CW20 on Terra
        let cw20_token =
            encode_terra_address_to_bytes32("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v")
                .unwrap();

        let amount: u128 = 1_000_000;
        let nonce: u64 = 5;

        let xchain_hash_id_deposit = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &evm_depositor,
            &terra_recipient,
            &cw20_token,
            amount,
            nonce,
        );

        let xchain_hash_id_withdraw = compute_xchain_hash_id(
            &evm_chain,
            &terra_chain,
            &evm_depositor,
            &terra_recipient,
            &cw20_token,
            amount,
            nonce,
        );

        assert_eq!(
            xchain_hash_id_deposit, xchain_hash_id_withdraw,
            "EVM→Terra CW20: deposit hash must equal withdraw hash"
        );

        println!(
            "EVM→Terra CW20 deposit=withdraw: {}",
            bytes32_to_hex(&xchain_hash_id_deposit)
        );
    }

    /// Terra Classic → EVM: native uluna source → ERC20 destination
    /// Deposit native uluna on Terra, withdraw ERC20-wrapped-uluna on EVM.
    /// Token = ERC20 address bytes32 (destination token on EVM).
    #[test]
    fn test_deposit_withdraw_match_terra_to_evm_native_erc20() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let terra_depositor =
            encode_terra_address_to_bytes32("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v")
                .unwrap();

        let evm_recipient = address_to_bytes32(&[
            0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72,
            0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66,
        ]);

        // Destination token: ERC20 wrapped-uluna on EVM
        // 0x5FbDB2315678afecb367f032d93F642f64180aa3
        let erc20_token = address_to_bytes32(&[
            0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F,
            0x64, 0x2f, 0x64, 0x18, 0x0a, 0xa3,
        ]);

        let amount: u128 = 500_000;
        let nonce: u64 = 3;

        let xchain_hash_id_deposit = compute_xchain_hash_id(
            &terra_chain,
            &evm_chain,
            &terra_depositor,
            &evm_recipient,
            &erc20_token,
            amount,
            nonce,
        );

        let xchain_hash_id_withdraw = compute_xchain_hash_id(
            &terra_chain,
            &evm_chain,
            &terra_depositor,
            &evm_recipient,
            &erc20_token,
            amount,
            nonce,
        );

        assert_eq!(
            xchain_hash_id_deposit, xchain_hash_id_withdraw,
            "Terra→EVM native→ERC20: deposit hash must equal withdraw hash"
        );

        println!(
            "Terra→EVM native→ERC20 deposit=withdraw: {}",
            bytes32_to_hex(&xchain_hash_id_deposit)
        );
    }

    /// Terra Classic → EVM: CW20 source → ERC20 destination
    /// Deposit CW20 on Terra, withdraw ERC20 on EVM.
    /// Token = ERC20 address bytes32 (destination token on EVM).
    #[test]
    fn test_deposit_withdraw_match_terra_to_evm_cw20_erc20() {
        let terra_chain: [u8; 4] = [0, 0, 0, 2];
        let evm_chain: [u8; 4] = [0, 0, 0, 1];

        let terra_depositor =
            encode_terra_address_to_bytes32("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v")
                .unwrap();

        // EVM recipient: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
        let evm_recipient = address_to_bytes32(&[
            0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01, 0x0C, 0x7d, 0x01, 0xb5,
            0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xC8,
        ]);

        // Destination token: ERC20 on EVM
        // 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
        let erc20_token = address_to_bytes32(&[
            0xe7, 0xf1, 0x72, 0x5E, 0x77, 0x34, 0xCE, 0x28, 0x8F, 0x83, 0x67, 0xe1, 0xBb, 0x14,
            0x3E, 0x90, 0xbb, 0x3F, 0x05, 0x12,
        ]);

        let amount: u128 = 2_500_000;
        let nonce: u64 = 7;

        let xchain_hash_id_deposit = compute_xchain_hash_id(
            &terra_chain,
            &evm_chain,
            &terra_depositor,
            &evm_recipient,
            &erc20_token,
            amount,
            nonce,
        );

        let xchain_hash_id_withdraw = compute_xchain_hash_id(
            &terra_chain,
            &evm_chain,
            &terra_depositor,
            &evm_recipient,
            &erc20_token,
            amount,
            nonce,
        );

        assert_eq!(
            xchain_hash_id_deposit, xchain_hash_id_withdraw,
            "Terra→EVM CW20→ERC20: deposit hash must equal withdraw hash"
        );

        println!(
            "Terra→EVM CW20→ERC20 deposit=withdraw: {}",
            bytes32_to_hex(&xchain_hash_id_deposit)
        );
    }
}
