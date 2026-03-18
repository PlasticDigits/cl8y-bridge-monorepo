use solana_program::keccak;

/// Compute the cross-chain transfer hash matching HashLib.sol computeXchainHashId.
///
/// Layout: 7 x 32 = 224 bytes (abi.encode compatible)
///   - srcChain:    bytes4 left-aligned in 32-byte slot
///   - destChain:   bytes4 left-aligned in 32-byte slot
///   - srcAccount:  32 bytes
///   - destAccount: 32 bytes
///   - token:       32 bytes (destination token)
///   - amount:      uint256 big-endian (u128 right-aligned in 32-byte slot)
///   - nonce:       uint256 big-endian (u64 right-aligned in 32-byte slot)
pub fn compute_transfer_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    let mut buf = [0u8; 224];

    // srcChain: 4 bytes left-aligned in 32-byte slot
    buf[0..4].copy_from_slice(src_chain);

    // destChain: 4 bytes left-aligned in 32-byte slot
    buf[32..36].copy_from_slice(dest_chain);

    // srcAccount: 32 bytes
    buf[64..96].copy_from_slice(src_account);

    // destAccount: 32 bytes
    buf[96..128].copy_from_slice(dest_account);

    // token: 32 bytes (destination token)
    buf[128..160].copy_from_slice(token);

    // amount: uint256 big-endian, u128 right-aligned in slot (bytes 16..32 of the slot)
    buf[176..192].copy_from_slice(&amount.to_be_bytes());

    // nonce: uint256 big-endian, u64 right-aligned in slot (bytes 24..32 of the slot)
    buf[216..224].copy_from_slice(&nonce.to_be_bytes());

    keccak::hash(&buf).to_bytes()
}
