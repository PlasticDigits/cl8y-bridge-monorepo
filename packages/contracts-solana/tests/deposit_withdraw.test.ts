import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import { Cl8yBridge } from "../target/types/cl8y_bridge";
import {
  setupTest, findBridgePda, findDepositPda, findChainPda, findWithdrawPda,
  findExecutedHashPda, airdrop, TestContext,
  initializeBridgeIfNeeded, registerChainIfNeeded, getNextDepositNonce
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
  // amount as u128 big-endian, right-aligned in 32-byte slot (bytes 176..192)
  const amountBuf = Buffer.alloc(16);
  amountBuf.writeBigUInt64BE(amount >> 64n, 0);
  amountBuf.writeBigUInt64BE(amount & 0xFFFFFFFFFFFFFFFFn, 8);
  amountBuf.copy(buf, 176);
  // nonce as u64 big-endian, right-aligned in 32-byte slot (bytes 216..224)
  const nonceBuf = Buffer.alloc(8);
  nonceBuf.writeBigUInt64BE(nonce);
  nonceBuf.copy(buf, 216);
  return keccak256(buf);
}

describe("deposit and withdraw flow", () => {
  let ctx: TestContext;
  let evmChainPda: PublicKey;

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
        feeBps: 50,
        withdrawDelay: new anchor.BN(15),
        paused: false,
      })
      .accounts({
        bridge: ctx.bridgePda,
        admin: ctx.admin.publicKey,
      })
      .rpc();

    evmChainPda = await registerChainIfNeeded(ctx, EVM_CHAIN_ID, "evm_1");
  });

  describe("deposit_native", () => {
    let firstDepositNonce: number;
    let firstDepositPda: PublicKey;

    it("deposits SOL and creates deposit record", async () => {
      const amount = 1 * LAMPORTS_PER_SOL;
      const destAccount = Array.from(Buffer.alloc(32, 0xBB));
      const destToken = Array.from(Buffer.alloc(32, 0xCC));

      firstDepositNonce = await getNextDepositNonce(ctx);
      [firstDepositPda] = findDepositPda(ctx.program.programId, firstDepositNonce);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: destAccount,
          destToken: destToken,
          amount: new anchor.BN(amount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: firstDepositPda,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const deposit = await ctx.program.account.depositRecord.fetch(firstDepositPda);
      expect(deposit.nonce.toNumber()).to.equal(firstDepositNonce);
      expect(deposit.srcAccount.toString()).to.equal(ctx.user.publicKey.toString());

      const expectedNet = amount - Math.floor(amount * 50 / 10000);
      expect(Number(deposit.amount)).to.equal(expectedNet);

      const bridge = await ctx.program.account.bridgeConfig.fetch(ctx.bridgePda);
      expect(bridge.depositNonce.toNumber()).to.equal(firstDepositNonce);
    });

    it("deposit hash uses chain_id from bridge config", async () => {
      const deposit = await ctx.program.account.depositRecord.fetch(firstDepositPda);

      const amount = 1 * LAMPORTS_PER_SOL;
      const expectedNet = BigInt(amount - Math.floor(amount * 50 / 10000));

      const expectedHash = computeTransferHash(
        SOLANA_CHAIN_ID,
        EVM_CHAIN_ID,
        ctx.user.publicKey.toBuffer(),
        Buffer.alloc(32, 0xBB),
        Buffer.alloc(32, 0xCC),
        expectedNet,
        BigInt(firstDepositNonce),
      );

      expect(Buffer.from(deposit.transferHash).toString("hex"))
        .to.equal(expectedHash.toString("hex"));
    });

    it("rejects zero amount", async () => {
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      try {
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32)),
            destToken: Array.from(Buffer.alloc(32)),
            amount: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: evmChainPda,
            depositor: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("ZeroAmount");
      }
    });

    it("rejects deposit to unregistered chain", async () => {
      const unregisteredChain = [0x00, 0x00, 0x00, 0xFF];
      const [fakePda] = findChainPda(ctx.program.programId, Buffer.from(unregisteredChain));
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      try {
        await ctx.program.methods
          .depositNative({
            destChain: unregisteredChain,
            destAccount: Array.from(Buffer.alloc(32)),
            destToken: Array.from(Buffer.alloc(32)),
            amount: new anchor.BN(1000000),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: fakePda,
            depositor: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("AccountNotInitialized");
      }
    });

    it("rejects when paused", async () => {
      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: null, withdrawDelay: null, paused: true })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      try {
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32)),
            destToken: Array.from(Buffer.alloc(32)),
            amount: new anchor.BN(1000000),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: evmChainPda,
            depositor: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }

      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: null, withdrawDelay: null, paused: false })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });

    it("fee math: 0 bps means no fee", async () => {
      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: 0, withdrawDelay: null, paused: null })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const amount = 1 * LAMPORTS_PER_SOL;
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xBB)),
          destToken: Array.from(Buffer.alloc(32, 0xCC)),
          amount: new anchor.BN(amount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const deposit = await ctx.program.account.depositRecord.fetch(depositPda);
      expect(Number(deposit.amount)).to.equal(amount);

      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: 50, withdrawDelay: null, paused: null })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });

    it("fee math: 10000 bps (100%) leaves net = 0", async () => {
      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: 10000, withdrawDelay: null, paused: null })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const amount = 1 * LAMPORTS_PER_SOL;
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xBB)),
          destToken: Array.from(Buffer.alloc(32, 0xCC)),
          amount: new anchor.BN(amount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const deposit = await ctx.program.account.depositRecord.fetch(depositPda);
      expect(Number(deposit.amount)).to.equal(0);

      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: 50, withdrawDelay: null, paused: null })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });
  });

  describe("withdraw lifecycle: submit -> approve -> delay -> execute_native", () => {
    let transferHash: Buffer;
    const withdrawAmount = 500000000n; // 0.5 SOL
    const withdrawNonce = 100n;
    const srcChain = EVM_CHAIN_ID;

    it("submit withdrawal", async () => {
      const srcAccount = Buffer.alloc(32, 0xAA);
      const destToken = Keypair.generate().publicKey;

      transferHash = computeTransferHash(
        srcChain,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        withdrawAmount,
        withdrawNonce,
      );

      const [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);
      const [executedHashPda] = findExecutedHashPda(ctx.program.programId, transferHash);

      await ctx.program.methods
        .withdrawSubmit({
          srcChain: srcChain,
          srcAccount: Array.from(srcAccount),
          destToken: destToken,
          amount: new anchor.BN(Number(withdrawAmount)),
          nonce: new anchor.BN(Number(withdrawNonce)),
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

      const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      expect(pw.approved).to.be.false;
      expect(pw.cancelled).to.be.false;
      expect(pw.executed).to.be.false;
      expect(Number(pw.amount)).to.equal(Number(withdrawAmount));
    });

    it("non-operator cannot approve", async () => {
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(transferHash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            operator: ctx.user.publicKey,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedOperator");
      }
    });

    it("operator approves withdrawal", async () => {
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);

      await ctx.program.methods
        .withdrawApprove({ transferHash: Array.from(transferHash) })
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          operator: ctx.operator.publicKey,
        })
        .signers([ctx.operator])
        .rpc();

      const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      expect(pw.approved).to.be.true;
      expect(pw.approvedAt.toNumber()).to.be.greaterThan(0);
    });

    it("cannot approve twice", async () => {
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(transferHash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            operator: ctx.operator.publicKey,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("AlreadyApproved");
      }
    });

    it("execute native withdrawal after delay", async () => {
      await new Promise((r) => setTimeout(r, 16000));

      const [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);
      const [executedHashPda] = findExecutedHashPda(ctx.program.programId, transferHash);

      const balanceBefore = await ctx.provider.connection.getBalance(ctx.user.publicKey);

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

      const balanceAfter = await ctx.provider.connection.getBalance(ctx.user.publicKey);
      expect(balanceAfter).to.be.greaterThan(balanceBefore);

      const executed = await ctx.program.account.executedHash.fetch(executedHashPda);
      expect(executed).to.not.be.null;
    });

    it("cannot re-submit after execution (close-reinit protection)", async () => {
      const srcAccount = Buffer.alloc(32, 0xAA);
      const destToken = Keypair.generate().publicKey;

      const sameHash = transferHash;
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, sameHash);
      const [executedHashPda] = findExecutedHashPda(ctx.program.programId, sameHash);

      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: srcChain,
            srcAccount: Array.from(srcAccount),
            destToken: destToken,
            amount: new anchor.BN(Number(withdrawAmount)),
            nonce: new anchor.BN(Number(withdrawNonce)),
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
        expect.fail("Should have thrown - close-reinit should be blocked");
      } catch (err) {
        expect(err.toString()).to.satisfy(
          (s: string) => s.includes("AlreadyExecutedHash") || s.includes("already in use"),
          "Expected AlreadyExecutedHash or account already in use error"
        );
      }
    });
  });

  describe("withdraw_approve rejects when paused", () => {
    it("operator cannot approve when bridge is paused", async () => {
      const srcAccount = Buffer.alloc(32, 0xDD);
      const destToken = Keypair.generate().publicKey;
      const amount = 100000n;
      const nonce = 200n;

      const hash = computeTransferHash(
        EVM_CHAIN_ID, SOLANA_CHAIN_ID,
        srcAccount, ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(), amount, nonce,
      );
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, hash);
      const [executedHashPda] = findExecutedHashPda(ctx.program.programId, hash);

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
        .setConfig({ newAdmin: null, operator: null, feeBps: null, withdrawDelay: null, paused: true })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(hash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            operator: ctx.operator.publicKey,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }

      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: null, withdrawDelay: null, paused: false })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });
  });
});
