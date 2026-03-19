import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { Cl8yBridge } from "../target/types/cl8y_bridge";
import {
  setupTest, findBridgePda, findChainPda, findWithdrawPda,
  findCancelerPda, findExecutedHashPda, airdrop, TestContext,
  initializeBridgeIfNeeded
} from "./helpers/setup";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];

function keccak256(data: Buffer): Buffer {
  const { keccak_256 } = require("js-sha3");
  return Buffer.from(keccak_256.arrayBuffer(data));
}

function computeTransferHash(
  srcChain: number[],
  destChain: number[],
  srcAccount: Buffer,
  destAccount: Buffer,
  token: Buffer,
  amount: bigint,
  nonce: bigint,
): Buffer {
  const buf = Buffer.alloc(224);
  Buffer.from(srcChain).copy(buf, 0);
  Buffer.from(destChain).copy(buf, 32);
  srcAccount.copy(buf, 64);
  destAccount.copy(buf, 96);
  token.copy(buf, 128);
  const amountBuf = Buffer.alloc(16);
  amountBuf.writeBigUInt64BE(amount >> 64n, 0);
  amountBuf.writeBigUInt64BE(amount & 0xFFFFFFFFFFFFFFFFn, 8);
  amountBuf.copy(buf, 176);
  const nonceBuf = Buffer.alloc(8);
  nonceBuf.writeBigUInt64BE(nonce);
  nonceBuf.copy(buf, 216);
  return keccak256(buf);
}

describe("cancel flow", () => {
  let ctx: TestContext;
  let transferHash: Buffer;
  let withdrawPda: PublicKey;
  let cancelerPda: PublicKey;

  const srcAccount = Buffer.alloc(32, 0xAA);
  const destToken = Keypair.generate().publicKey;
  const amount = 500000n;
  const nonce = 10n;

  before(async () => {
    ctx = await setupTest();

    await initializeBridgeIfNeeded(ctx, {
      operator: ctx.operator.publicKey,
      feeBps: 50,
      withdrawDelay: new anchor.BN(300),
      chainId: SOLANA_CHAIN_ID,
    });
    await ctx.program.methods
      .setConfig({
        newAdmin: null,
        operator: ctx.operator.publicKey,
        feeBps: null,
        withdrawDelay: null,
        paused: null,
      })
      .accounts({
        bridge: ctx.bridgePda,
        admin: ctx.admin.publicKey,
      })
      .rpc();

    [cancelerPda] = findCancelerPda(ctx.program.programId, ctx.canceler.publicKey);
    await ctx.program.methods
      .addCanceler({ canceler: ctx.canceler.publicKey, active: true })
      .accounts({
        bridge: ctx.bridgePda,
        cancelerEntry: cancelerPda,
        admin: ctx.admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    transferHash = computeTransferHash(
      EVM_CHAIN_ID, SOLANA_CHAIN_ID,
      srcAccount, ctx.user.publicKey.toBuffer(),
      destToken.toBuffer(), amount, nonce,
    );
    [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);
    const [executedHashPda] = findExecutedHashPda(ctx.program.programId, transferHash);

    await ctx.program.methods
      .withdrawSubmit({
        srcChain: EVM_CHAIN_ID,
        srcAccount: Array.from(srcAccount),
        destToken: destToken,
        amount: new anchor.BN(Number(amount)),
        nonce: new anchor.BN(Number(nonce)),
      })
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        executedHashCheck: executedHashPda,
        recipient: ctx.user.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([ctx.user])
      .rpc();

    await ctx.program.methods
      .withdrawApprove({ transferHash: Array.from(transferHash) })
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        operator: ctx.operator.publicKey,
      })
      .signers([ctx.operator])
      .rpc();
  });

  it("canceler entry is registered and active", async () => {
    const entry = await ctx.program.account.cancelerEntry.fetch(cancelerPda);
    expect(entry.active).to.be.true;
    expect(entry.pubkey.toString()).to.equal(ctx.canceler.publicKey.toString());
  });

  it("active canceler can cancel an approved withdrawal", async () => {
    await ctx.program.methods
      .withdrawCancel()
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        cancelerEntry: cancelerPda,
        canceler: ctx.canceler.publicKey,
      })
      .signers([ctx.canceler])
      .rpc();

    const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
    expect(pw.cancelled).to.be.true;
  });

  it("non-canceler cannot cancel (account does not exist)", async () => {
    const randomUser = Keypair.generate();
    await airdrop(ctx.provider.connection, randomUser.publicKey);

    const [fakePda] = findCancelerPda(ctx.program.programId, randomUser.publicKey);

    try {
      await ctx.program.methods
        .withdrawCancel()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          cancelerEntry: fakePda,
          canceler: randomUser.publicKey,
        })
        .signers([randomUser])
        .rpc();
      expect.fail("Should have thrown");
    } catch (err) {
      expect(err.toString()).to.contain("AccountNotInitialized");
    }
  });

  it("deactivated canceler cannot cancel", async () => {
    await ctx.program.methods
      .addCanceler({ canceler: ctx.canceler.publicKey, active: false })
      .accounts({
        bridge: ctx.bridgePda,
        cancelerEntry: cancelerPda,
        admin: ctx.admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const srcAccount2 = Buffer.alloc(32, 0xDD);
    const destToken2 = Keypair.generate().publicKey;
    const hash2 = computeTransferHash(
      EVM_CHAIN_ID, SOLANA_CHAIN_ID,
      srcAccount2, ctx.user.publicKey.toBuffer(),
      destToken2.toBuffer(), 1000n, 20n,
    );
    const [wp2] = findWithdrawPda(ctx.program.programId, hash2);
    const [eh2] = findExecutedHashPda(ctx.program.programId, hash2);

    await ctx.program.methods
      .withdrawSubmit({
        srcChain: EVM_CHAIN_ID,
        srcAccount: Array.from(srcAccount2),
        destToken: destToken2,
        amount: new anchor.BN(1000),
        nonce: new anchor.BN(20),
      })
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: wp2,
        executedHashCheck: eh2,
        recipient: ctx.user.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([ctx.user])
      .rpc();

    await ctx.program.methods
      .withdrawApprove({ transferHash: Array.from(hash2) })
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: wp2,
        operator: ctx.operator.publicKey,
      })
      .signers([ctx.operator])
      .rpc();

    try {
      await ctx.program.methods
        .withdrawCancel()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: wp2,
          cancelerEntry: cancelerPda,
          canceler: ctx.canceler.publicKey,
        })
        .signers([ctx.canceler])
        .rpc();
      expect.fail("Should have thrown");
    } catch (err) {
      expect(err.toString()).to.contain("UnauthorizedCanceler");
    }

    await ctx.program.methods
      .addCanceler({ canceler: ctx.canceler.publicKey, active: true })
      .accounts({
        bridge: ctx.bridgePda,
        cancelerEntry: cancelerPda,
        admin: ctx.admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
  });

  it("admin can reenable a cancelled withdrawal", async () => {
    await ctx.program.methods
      .withdrawReenable()
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        admin: ctx.admin.publicKey,
      })
      .rpc();

    const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
    expect(pw.cancelled).to.be.false;
    expect(pw.approved).to.be.true;
  });

  it("non-admin cannot reenable", async () => {
    await ctx.program.methods
      .withdrawCancel()
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        cancelerEntry: cancelerPda,
        canceler: ctx.canceler.publicKey,
      })
      .signers([ctx.canceler])
      .rpc();

    try {
      await ctx.program.methods
        .withdrawReenable()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          admin: ctx.user.publicKey,
        })
        .signers([ctx.user])
        .rpc();
      expect.fail("Should have thrown");
    } catch (err) {
      expect(err.toString()).to.contain("UnauthorizedAdmin");
    }
  });
});
