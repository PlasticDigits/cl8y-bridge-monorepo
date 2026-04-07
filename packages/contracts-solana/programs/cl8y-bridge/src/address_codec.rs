use anchor_lang::prelude::Pubkey;

pub const CHAIN_TYPE_SOLANA: u32 = 3;

/// Convert a Solana Pubkey to a 32-byte representation for use in transfer hashes.
/// Solana pubkeys are already 32 bytes, so no padding needed.
pub fn pubkey_to_bytes32(pubkey: &Pubkey) -> [u8; 32] {
    pubkey.to_bytes()
}

/// Convert a 32-byte array back to a Solana Pubkey.
pub fn bytes32_to_pubkey(bytes: &[u8; 32]) -> Pubkey {
    Pubkey::new_from_array(*bytes)
}

/// Encode a Solana chain ID as bytes4 (big-endian u32).
pub fn encode_chain_id(chain_id: u32) -> [u8; 4] {
    chain_id.to_be_bytes()
}
