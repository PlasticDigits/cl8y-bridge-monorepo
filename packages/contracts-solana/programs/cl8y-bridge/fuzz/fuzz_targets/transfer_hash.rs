#![no_main]

use cl8y_bridge::hash::compute_transfer_hash;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 + 4 + 32 * 3 + 16 + 8 {
        return;
    }
    let mut i = 0usize;
    let mut src_chain = [0u8; 4];
    src_chain.copy_from_slice(&data[i..i + 4]);
    i += 4;
    let mut dest_chain = [0u8; 4];
    dest_chain.copy_from_slice(&data[i..i + 4]);
    i += 4;
    let mut src_account = [0u8; 32];
    src_account.copy_from_slice(&data[i..i + 32]);
    i += 32;
    let mut dest_account = [0u8; 32];
    dest_account.copy_from_slice(&data[i..i + 32]);
    i += 32;
    let mut token = [0u8; 32];
    token.copy_from_slice(&data[i..i + 32]);
    i += 32;
    let amount = u128::from_be_bytes(data[i..i + 16].try_into().unwrap());
    i += 16;
    let nonce = u64::from_be_bytes(data[i..i + 8].try_into().unwrap());

    let h = compute_transfer_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );
    std::hint::black_box(h);
});
