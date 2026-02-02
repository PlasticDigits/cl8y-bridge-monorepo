//! Hash computation for cross-chain transfer IDs
//!
//! This module provides hash computation functions that match the EVM and
//! Terra contract implementations for verifying transfer identities.

use tiny_keccak::{Hasher, Keccak};

/// Compute keccak256 hash of data
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Compute the transfer ID that matches EVM's _computeTransferId
///
/// This is the canonical hash used to identify a transfer across chains:
/// keccak256(abi.encode(srcChainKey, destChainKey, destTokenAddress, destAccount, amount, nonce))
///
/// All values are encoded as 32-byte words in big-endian format.
pub fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    dest_token_address: &[u8; 32],
    dest_account: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    // abi.encode layout: 6 words * 32 bytes = 192 bytes
    let mut data = [0u8; 192];

    // Word 0: srcChainKey (bytes32)
    data[0..32].copy_from_slice(src_chain_key);

    // Word 1: destChainKey (bytes32)
    data[32..64].copy_from_slice(dest_chain_key);

    // Word 2: destTokenAddress (bytes32)
    data[64..96].copy_from_slice(dest_token_address);

    // Word 3: destAccount (bytes32)
    data[96..128].copy_from_slice(dest_account);

    // Word 4: amount (uint256, but we only use u128)
    // Big-endian in last 16 bytes of the 32-byte word
    let amount_bytes = amount.to_be_bytes();
    data[128 + 16..160].copy_from_slice(&amount_bytes);

    // Word 5: nonce (uint256, but we only use u64)
    // Big-endian in last 8 bytes of the 32-byte word
    let nonce_bytes = nonce.to_be_bytes();
    data[160 + 24..192].copy_from_slice(&nonce_bytes);

    keccak256(&data)
}

/// Compute EVM chain key: keccak256(abi.encode("EVM", bytes32(chainId)))
pub fn evm_chain_key(chain_id: u64) -> [u8; 32] {
    // abi.encode("EVM", bytes32(chainId))
    // String "EVM" is encoded as:
    // - offset (32 bytes): 0x40 = 64 (pointing past the two words)
    // - chainId as bytes32 (32 bytes)
    // - string length (32 bytes): 3
    // - string data padded to 32 bytes: "EVM\0..."

    let mut data = vec![0u8; 128];

    // Word 0: offset to string data (0x40 = 64)
    data[31] = 0x40;

    // Word 1: chainId as bytes32 (big-endian u64 in last 8 bytes)
    let chain_id_bytes = chain_id.to_be_bytes();
    data[32 + 24..64].copy_from_slice(&chain_id_bytes);

    // Word 2: string length (3)
    data[64 + 31] = 3;

    // Word 3: string data "EVM" padded to 32 bytes
    data[96..99].copy_from_slice(b"EVM");

    keccak256(&data)
}

/// Compute Cosmos chain key: keccak256(abi.encode("COSMW", keccak256(abi.encode(chainId))))
///
/// This matches the EVM contract's getChainKeyCOSMW function.
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

/// Get Terra Classic chain key for columbus-5 mainnet
pub fn terra_chain_key() -> [u8; 32] {
    cosmos_chain_key("columbus-5")
}

/// Get Terra Classic chain key for localterra
pub fn localterra_chain_key() -> [u8; 32] {
    cosmos_chain_key("localterra")
}

/// Encode a Terra address to 32 bytes (left-padded)
pub fn encode_terra_address(addr: &str) -> [u8; 32] {
    let mut result = [0u8; 32];
    let bytes = addr.as_bytes();

    if bytes.len() <= 32 {
        let start = 32 - bytes.len();
        result[start..].copy_from_slice(bytes);
    } else {
        // Truncate if too long (shouldn't happen for valid addresses)
        result.copy_from_slice(&bytes[bytes.len() - 32..]);
    }

    result
}

/// Encode an EVM address (0x-prefixed hex) to 32 bytes (left-padded)
pub fn encode_evm_address(addr: &str) -> Result<[u8; 32], &'static str> {
    let hex_str = addr.strip_prefix("0x").unwrap_or(addr);

    if hex_str.len() != 40 {
        return Err("Invalid EVM address length");
    }

    let mut result = [0u8; 32];

    // Parse 20-byte address
    for i in 0..20 {
        result[12 + i] = u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16)
            .map_err(|_| "Invalid hex character")?;
    }

    Ok(result)
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
    fn test_evm_chain_key_bsc() {
        let key = evm_chain_key(56);
        // Matches EVM contract's getChainKeyEVM(56) and Terra hash.rs
        assert_eq!(
            bytes32_to_hex(&key),
            "0xe2debc38147727fd4c36e012d1d8335aebec2bcb98c3b1aae5dde65ddcd74367"
        );
    }

    #[test]
    fn test_terra_chain_key() {
        let key = terra_chain_key();
        // Matches EVM contract's getChainKeyCOSMW("columbus-5") and Terra hash.rs
        assert_eq!(
            bytes32_to_hex(&key),
            "0x0ece70814ff48c843659d2c2cfd2138d070b75d11f9fd81e424873e90a47d8b3"
        );
    }

    #[test]
    fn test_compute_transfer_id_all_zeros() {
        let src = [0u8; 32];
        let dest = [0u8; 32];
        let token = [0u8; 32];
        let account = [0u8; 32];

        let hash = compute_transfer_id(&src, &dest, &token, &account, 0, 0);

        // Matches Vector 1 from EVM HashVectors.t.sol and Terra hash.rs
        assert_eq!(
            bytes32_to_hex(&hash),
            "0x1e990e27f0d7976bf2adbd60e20384da0125b76e2885a96aa707bcb054108b0d"
        );
    }

    #[test]
    fn test_compute_transfer_id_simple_values() {
        // Match Vector 2 from Terra hash.rs
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

        let hash = compute_transfer_id(
            &src_chain_key,
            &dest_chain_key,
            &dest_token_address,
            &dest_account,
            amount,
            nonce,
        );

        assert_eq!(
            bytes32_to_hex(&hash),
            "0x7226dd6b664f0c50fb3e50adfa82057dab4819f592ef9d35c08b9c4531b05150"
        );
    }
}
