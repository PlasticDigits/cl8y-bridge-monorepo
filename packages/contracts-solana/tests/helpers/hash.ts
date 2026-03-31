/**
 * V2 xchain transfer hash (224-byte layout + keccak256) for Anchor TS tests.
 * Matches `cl8y_bridge::hash::compute_transfer_hash` — see docs/SOLANA_BRIDGE_INVARIANTS.md INV-H1.
 */
import pkg from "js-sha3";

const { keccak_256 } = pkg;

export function keccak256(data: Buffer): Buffer {
  return Buffer.from(keccak_256.arrayBuffer(data));
}

function chainSlice(chain: Buffer | number[]): Buffer {
  return Buffer.isBuffer(chain) ? chain : Buffer.from(chain);
}

/** u128 amount in low 16 bytes of the 32-byte ABI word; u64 nonce in low 8 bytes. */
export function computeTransferHash(
  srcChain: Buffer | number[],
  destChain: Buffer | number[],
  srcAccount: Buffer,
  destAccount: Buffer,
  token: Buffer,
  amount: bigint,
  nonce: bigint
): Buffer {
  const buf = Buffer.alloc(224);
  chainSlice(srcChain).copy(buf, 0);
  chainSlice(destChain).copy(buf, 32);
  srcAccount.copy(buf, 64);
  destAccount.copy(buf, 96);
  token.copy(buf, 128);

  const amountBuf = Buffer.alloc(16);
  amountBuf.writeBigUInt64BE(amount >> 64n, 0);
  amountBuf.writeBigUInt64BE(amount & 0xffffffffffffffffn, 8);
  amountBuf.copy(buf, 176);

  const nonceBuf = Buffer.alloc(8);
  nonceBuf.writeBigUInt64BE(nonce);
  nonceBuf.copy(buf, 216);

  return keccak256(buf);
}

/** Legacy/wrong layout test helper: u64 amount at bytes 184..192 (see hash_parity tests). */
export function computeTransferHashU64Amount(
  srcChain: Buffer | number[],
  destChain: Buffer | number[],
  srcAccount: Buffer,
  destAccount: Buffer,
  token: Buffer,
  amount: bigint,
  nonce: bigint
): Buffer {
  const buf = Buffer.alloc(224);
  chainSlice(srcChain).copy(buf, 0);
  chainSlice(destChain).copy(buf, 32);
  srcAccount.copy(buf, 64);
  destAccount.copy(buf, 96);
  token.copy(buf, 128);

  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64BE(amount);
  amountBuf.copy(buf, 184);

  const nonceBuf = Buffer.alloc(8);
  nonceBuf.writeBigUInt64BE(nonce);
  nonceBuf.copy(buf, 216);

  return keccak256(buf);
}
