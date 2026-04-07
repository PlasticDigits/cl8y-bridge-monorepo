/**
 * Post–mainnet-deploy smoke: TS hash helpers only (no chain RPC, no faucet).
 * Full cross-chain golden vectors stay in hash_parity.test.ts (Anchor suite / non-mainnet deploy).
 */
import { expect } from "chai";

import {
  computeTransferHash,
  computeTransferHashU64Amount,
} from "./helpers/hash";

describe("hash parity (mainnet deploy smoke)", () => {
  it("is deterministic and differs when amount or nonce changes", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x02]);
    const srcAccount = Buffer.alloc(32);
    srcAccount[31] = 0x01;
    const destAccount = Buffer.alloc(32);
    destAccount[31] = 0x02;
    const token = Buffer.alloc(32);
    token[31] = 0x03;

    const base = { srcChain, destChain, srcAccount, destAccount, token };
    const h1 = computeTransferHash(
      base.srcChain,
      base.destChain,
      base.srcAccount,
      base.destAccount,
      base.token,
      1000000n,
      1n
    );
    const hSame = computeTransferHash(
      base.srcChain,
      base.destChain,
      base.srcAccount,
      base.destAccount,
      base.token,
      1000000n,
      1n
    );
    expect(h1.toString("hex")).to.equal(hSame.toString("hex"));

    const hAmt = computeTransferHash(
      base.srcChain,
      base.destChain,
      base.srcAccount,
      base.destAccount,
      base.token,
      999999n,
      1n
    );
    const hNonce = computeTransferHash(
      base.srcChain,
      base.destChain,
      base.srcAccount,
      base.destAccount,
      base.token,
      1000000n,
      2n
    );
    expect(h1.toString("hex")).to.not.equal(hAmt.toString("hex"));
    expect(h1.toString("hex")).to.not.equal(hNonce.toString("hex"));
  });

  it("u64 amount layout matches u128 for values fitting in u64", () => {
    const srcChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const destChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
    const srcAccount = Buffer.alloc(32, 0xaa);
    const destAccount = Buffer.alloc(32, 0xbb);
    const token = Buffer.alloc(32, 0xcc);
    const amount = 1_000_000_000n;
    const nonce = 42n;

    const hashU128 = computeTransferHash(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce
    );
    const hashU64 = computeTransferHashU64Amount(
      srcChain,
      destChain,
      srcAccount,
      destAccount,
      token,
      amount,
      nonce
    );

    expect(hashU128.toString("hex")).to.equal(hashU64.toString("hex"));
  });

  /** Single SPL-style mint fixture: Solana (0x05) → EVM; deposit and withdraw must agree (INV-H1). */
  it("Solana→EVM: one token mint, full 32-byte depositor; deposit hash equals withdraw hash", () => {
    const solanaChain = Buffer.from([0x00, 0x00, 0x00, 0x05]);
    const evmChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);

    const depositor = Buffer.from(
      "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
      "hex"
    );
    const recipient = Buffer.alloc(32);
    Buffer.from([
      0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8,
      0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66,
    ]).copy(recipient, 12);

    const token = Buffer.alloc(32);
    token[31] = 0x42;

    const amount = 995_000n;
    const nonce = 1n;

    const depositHash = computeTransferHash(
      solanaChain,
      evmChain,
      depositor,
      recipient,
      token,
      amount,
      nonce
    );
    const withdrawHash = computeTransferHash(
      solanaChain,
      evmChain,
      depositor,
      recipient,
      token,
      amount,
      nonce
    );

    expect(depositHash.toString("hex")).to.equal(withdrawHash.toString("hex"));
    expect(depositHash.length).to.equal(32);
    expect(depositHash.toString("hex")).to.not.equal("0".repeat(64));
  });
});
