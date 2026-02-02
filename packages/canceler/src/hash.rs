//! Hash computation for verification
//!
//! Reuses the same hash functions as the operator for consistency.

use tiny_keccak::{Hasher, Keccak};

/// Compute keccak256 hash
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Compute transfer ID for verification
pub fn compute_transfer_id(
    src_chain_key: &[u8; 32],
    dest_chain_key: &[u8; 32],
    dest_token_address: &[u8; 32],
    dest_account: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    let mut data = [0u8; 192];

    data[0..32].copy_from_slice(src_chain_key);
    data[32..64].copy_from_slice(dest_chain_key);
    data[64..96].copy_from_slice(dest_token_address);
    data[96..128].copy_from_slice(dest_account);

    let amount_bytes = amount.to_be_bytes();
    data[128 + 16..160].copy_from_slice(&amount_bytes);

    let nonce_bytes = nonce.to_be_bytes();
    data[160 + 24..192].copy_from_slice(&nonce_bytes);

    keccak256(&data)
}

/// EVM chain key computation
pub fn evm_chain_key(chain_id: u64) -> [u8; 32] {
    let mut data = [0u8; 128];
    data[31] = 64;
    let chain_id_bytes = chain_id.to_be_bytes();
    data[32 + 24..64].copy_from_slice(&chain_id_bytes);
    data[64 + 31] = 3;
    data[96..99].copy_from_slice(b"EVM");
    keccak256(&data)
}

/// Cosmos chain key computation
pub fn cosmos_chain_key(chain_id: &str) -> [u8; 32] {
    let inner_hash = abi_encode_string_hash(chain_id);
    abi_encode_chain_key("COSMW", &inner_hash)
}

fn abi_encode_string_hash(s: &str) -> [u8; 32] {
    let str_bytes = s.as_bytes();
    let len = str_bytes.len();
    let padded_len = ((len + 31) / 32) * 32;

    let total_size = 32 + 32 + padded_len;
    let mut data = vec![0u8; total_size];

    data[31] = 32;
    data[32 + 24..64].copy_from_slice(&(len as u64).to_be_bytes());
    data[64..64 + len].copy_from_slice(str_bytes);

    keccak256(&data)
}

fn abi_encode_chain_key(chain_type: &str, raw_key: &[u8; 32]) -> [u8; 32] {
    let type_bytes = chain_type.as_bytes();
    let type_len = type_bytes.len();
    let padded_type_len = ((type_len + 31) / 32) * 32;

    let total_size = 64 + 32 + padded_type_len;
    let mut data = vec![0u8; total_size];

    data[31] = 64;
    data[32..64].copy_from_slice(raw_key);
    data[64 + 24..96].copy_from_slice(&(type_len as u64).to_be_bytes());
    data[96..96 + type_len].copy_from_slice(type_bytes);

    keccak256(&data)
}

/// Convert bytes to hex string
pub fn bytes32_to_hex(bytes: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(66);
    hex.push_str("0x");
    for byte in bytes {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}
