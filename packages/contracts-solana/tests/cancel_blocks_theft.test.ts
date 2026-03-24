import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import {
  setupTest,
  findWithdrawPda,
  findCancelerPda,
  findExecutedHashPda,
  TestContext,
  initializeBridgeIfNeeded,
  NATIVE_SOL_TOKEN,
} from "./helpers/setup";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];
const WITHDRAW_DELAY_SECS = 15;

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
  nonce: bigint
): Buffer {
  const buf = Buffer.alloc(224);
  Buffer.from(srcChain).copy(buf, 0);
  Buffer.from(destChain).copy(buf, 32);
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

async function sleep(ms: number): Promise<void> {
  await new Promise((r) => setTimeout(r, ms));
}

describe("cancel blocks theft (full guarantee)", () => {
  let ctx: TestContext;
  let transferHash: Buffer;
  let withdrawPda: PublicKey;
  let executedHashPda: PublicKey;
  let cancelerPda: PublicKey;

  const srcAccount = Buffer.alloc(32, 0x33);
  const amount = 400_000n;
  const nonce = 99n;

  before(async () => {
    ctx = await setupTest();

    await initializeBridgeIfNeeded(ctx, {
      operator: ctx.operator.publicKey,
      feeBps: 50,
      withdrawDelay: new anchor.BN(WITHDRAW_DELAY_SECS),
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

    [cancelerPda] = findCancelerPda(
      ctx.program.programId,
      ctx.canceler.publicKey
    );
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
      EVM_CHAIN_ID,
      SOLANA_CHAIN_ID,
      srcAccount,
      ctx.user.publicKey.toBuffer(),
      NATIVE_SOL_TOKEN.toBuffer(),
      amount,
      nonce
    );
    [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);
    [executedHashPda] = findExecutedHashPda(
      ctx.program.programId,
      transferHash
    );

    await ctx.program.methods
      .withdrawSubmit({
        srcChain: EVM_CHAIN_ID,
        srcAccount: Array.from(srcAccount),
        destToken: NATIVE_SOL_TOKEN,
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

  it("operator approve → cancel → past delay → execute blocked; reenable requires re-approval; second cancel still blocks", async () => {
    const bridgeBefore = (
      await ctx.provider.connection.getAccountInfo(ctx.bridgePda)
    )!.lamports;

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

    await sleep((WITHDRAW_DELAY_SECS + 1) * 1000);

    try {
      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();
      expect.fail("execute should fail while cancelled");
    } catch (err) {
      expect(err.toString()).to.match(/WithdrawalCancelled|6007|cancel/i);
    }

    const bridgeMid = (
      await ctx.provider.connection.getAccountInfo(ctx.bridgePda)
    )!.lamports;
    expect(bridgeMid).to.equal(bridgeBefore);

    await ctx.program.methods
      .withdrawReenable()
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        admin: ctx.admin.publicKey,
      })
      .rpc();

    const pwAfterReenable = await ctx.program.account.pendingWithdraw.fetch(
      withdrawPda
    );
    expect(pwAfterReenable.cancelled).to.be.false;
    expect(pwAfterReenable.approved).to.be.false;

    try {
      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();
      expect.fail("execute should fail without re-approval");
    } catch (err) {
      expect(err.toString()).to.contain("NotApproved");
    }

    await ctx.program.methods
      .withdrawApprove({ transferHash: Array.from(transferHash) })
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        operator: ctx.operator.publicKey,
      })
      .signers([ctx.operator])
      .rpc();

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
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();
      expect.fail("execute should fail after second cancel");
    } catch (err) {
      expect(err.toString()).to.match(/WithdrawalCancelled|6007|cancel/i);
    }
  });
});
