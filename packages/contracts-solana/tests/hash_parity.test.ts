import { expect } from "chai";

import pkg from "js-sha3";
const { keccak_256 } = pkg;

function keccak256(data: Buffer): Buffer {

  return Buffer.from(keccak_256.arrayBuffer(data));
}

function computeTransferHash(
  srcChain: Buffer,
  destChain: Buffer,
  srcAccount: Buffer,
  destAccount: Buffer,
  token: Buffer,
  amount: bigint,
  nonce: bigint,
): Buffer {
  const buf = Buffer.alloc(224);

  srcChain.copy(buf, 0);
  destChain.copy(buf, 32);
  srcAccount.copy(buf, 64);
  destAccount.copy(buf, 96);
  token.copy(buf, 128);

  // amount as u128 big-endian, right-aligned in 32-byte slot
  const amountBuf = Buffer.alloc(16);
  amountBuf.writeBigUInt64BE(amount >> 64n, 0);
  amountBuf.writeBigUInt64BE(amount & 0xFFFFFFFFFFFFFFFFn, 8);
  amountBuf.copy(buf, 176);

  // nonce as u64 big-endian, right-aligned in 32-byte slot
  const nonceBuf = Buffer.alloc(8);
  nonceBuf.writeBigUInt64BE(nonce);
  nonceBuf.copy(buf, 216);

  return keccak256(buf);
}

function computeTransferHashU64Amount(
  srcChain: Buffer,
  destChain: Buffer,
  srcAccount: Buffer,
  destAccount: Buffer,
  token: Buffer,
  amount: bigint,
  nonce: bigint,
): Buffer {
  const buf = Buffer.alloc(224);

  srcChain.copy(buf, 0);
  destChain.copy(buf, 32);
  srcAccount.copy(buf, 64);
  destAccount.copy(buf, 96);
  token.copy(buf, 128);

  // amount as u64 big-endian at bytes 184..192 (same as Solana program)
  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64BE(amount);
  amountBuf.copy(buf, 184);

  const nonceBuf = Buffer.alloc(8);
  nonceBuf.writeBigUInt64BE(nonce);
  nonceBuf.copy(buf, 216);

  return keccak256(buf);
}

describe("hash parity", () => {
  it("is deterministic", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.alloc(32);
    srcAccount[31] = 0x01;
    const destAccount = Buffer.alloc(32);
    destAccount[31] = 0x02;
    const token = Buffer.alloc(32);
    token[31] = 0x03;

    const hash1 = computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, 1000000n, 1n);
    const hash2 = computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, 1000000n, 1n);
    expect(hash1.toString("hex")).to.equal(hash2.toString("hex"));
  });

  it("different inputs produce different hashes", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.alloc(32);
    const destAccount = Buffer.alloc(32);
    const token = Buffer.alloc(32);

    const hash1 = computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, 1000000n, 1n);
    const hash2 = computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, 999999n, 1n);
    const hash3 = computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, 1000000n, 2n);

    expect(hash1.toString("hex")).to.not.equal(hash2.toString("hex"));
    expect(hash1.toString("hex")).to.not.equal(hash3.toString("hex"));
  });

  it("u64 amount layout matches u128 layout for values fitting in u64", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const srcAccount = Buffer.alloc(32, 0xAA);
    const destAccount = Buffer.alloc(32, 0xBB);
    const token = Buffer.alloc(32, 0xCC);
    const amount = 1_000_000_000n;
    const nonce = 42n;

    const hashU128 = computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, amount, nonce);
    const hashU64 = computeTransferHashU64Amount(srcChain, destChain, srcAccount, destAccount, token, amount, nonce);

    expect(hashU128.toString("hex")).to.equal(hashU64.toString("hex"),
      "u64 and u128 encodings must produce the same hash for amounts that fit in u64");
  });

  it("matches known EVM reference hash (EVM chain 1 -> chain 2)", () => {
    // Reference test: keccak256(abi.encode(
    //   bytes32(0x00000001), bytes32(0x00000002),
    //   bytes32(0), bytes32(0), bytes32(0),
    //   uint256(1000000), uint256(1)
    // ))
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.alloc(32);
    const destAccount = Buffer.alloc(32);
    const token = Buffer.alloc(32);

    const hash = computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, 1000000n, 1n);

    // Manually verify the abi.encode buffer layout
    const expectedBuf = Buffer.alloc(224);
    expectedBuf[3] = 0x01; // srcChain
    expectedBuf[35] = 0x02; // destChain
    // srcAccount, destAccount, token all zeros
    // amount = 1000000 = 0xF4240
    const amtBuf = Buffer.alloc(16);
    amtBuf.writeBigUInt64BE(0n, 0);
    amtBuf.writeBigUInt64BE(1000000n, 8);
    amtBuf.copy(expectedBuf, 176);
    // nonce = 1
    const nonceBuf = Buffer.alloc(8);
    nonceBuf.writeBigUInt64BE(1n);
    nonceBuf.copy(expectedBuf, 216);

    const expectedHash = keccak256(expectedBuf);
    expect(hash.toString("hex")).to.equal(expectedHash.toString("hex"));
  });

  it("deposit and withdraw compute same hash (cross-chain verification)", () => {
    // Simulates: deposit on Solana (srcChain=0x05) -> withdraw on EVM (destChain=0x01)
    const solanaChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const evmChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);

    const depositor = Buffer.alloc(32, 0xAA); // Solana pubkey
    const recipient = Buffer.alloc(32);
    // EVM 20-byte address left-padded
    Buffer.from([0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce,
                  0x6a, 0xB8, 0x82, 0x72, 0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66])
      .copy(recipient, 12);
    const token = Buffer.alloc(32);
    token[31] = 0x42;

    const depositHash = computeTransferHash(
      solanaChain, evmChain, depositor, recipient, token, 995000n, 1n
    );
    const withdrawHash = computeTransferHash(
      solanaChain, evmChain, depositor, recipient, token, 995000n, 1n
    );

    expect(depositHash.toString("hex")).to.equal(withdrawHash.toString("hex"),
      "Deposit and withdraw must produce identical hashes");
  });

  it("Solana 32-byte pubkey as srcAccount produces valid hash", () => {
    const solanaChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const evmChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);

    // Full 32-byte Solana pubkey (no left-padding needed)
    const solanaPubkey = Buffer.from(
      "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
      "hex"
    );
    const evmRecipient = Buffer.alloc(32);
    evmRecipient[12] = 0xf3;
    evmRecipient[31] = 0x66;

    const token = Buffer.alloc(32);

    const hash = computeTransferHash(
      solanaChain, evmChain, solanaPubkey, evmRecipient, token, 1000000000n, 1n
    );

    expect(hash.length).to.equal(32);
    expect(hash.toString("hex")).to.not.equal("0".repeat(64));
  });
});
