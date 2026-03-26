import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { expect } from "chai";
import { Cl8yBridge } from "../target/types/cl8y_bridge";
import {
  setupTest,
  findBridgePda,
  findDepositPda,
  findChainPda,
  findWithdrawPda,
  findWithdrawRateLimitPda,
  findExecutedHashPda,
  findTokenPda,
  findNonceUsedPda,
  airdrop,
  TestContext,
  initializeBridgeIfNeeded,
  registerChainIfNeeded,
  getNextDepositNonce,
  NATIVE_SOL_TOKEN,
} from "./helpers/setup";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];

/** Must match `setConfig` withdraw delay in `before` (+ buffer for strict `>` execute boundary). */
const WITHDRAW_DELAY_SECONDS = 15;

/** bytes32 used in deposit_native token mapping (destination token on EVM). */
const DEPOSIT_DEST_TOKEN = Buffer.alloc(32, 0xcc);

/** Remote token id for incoming native SOL withdrawals (EVM side representation). */
const EVM_REMOTE_NATIVE_TOKEN = Buffer.alloc(32);
EVM_REMOTE_NATIVE_TOKEN[31] = 0x37;

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

describe("deposit and withdraw flow", () => {
  let ctx: TestContext;
  let evmChainPda: PublicKey;
  let depositTokenMappingPda: PublicKey;
  let withdrawNativeTokenMappingPda: PublicKey;

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

    [depositTokenMappingPda] = findTokenPda(
      ctx.program.programId,
      Buffer.from(EVM_CHAIN_ID),
      DEPOSIT_DEST_TOKEN
    );
    const depInfo = await ctx.provider.connection.getAccountInfo(
      depositTokenMappingPda
    );
    if (!depInfo) {
      await ctx.program.methods
        .registerToken({
          localMint: PublicKey.default,
          destChain: EVM_CHAIN_ID,
          destToken: Array.from(DEPOSIT_DEST_TOKEN),
          mode: { lockUnlock: {} },
          decimals: 9,
          srcDecimals: 18,
        })
        .accounts({
          bridge: ctx.bridgePda,
          tokenMapping: depositTokenMappingPda,
          mint: null,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    }

    [withdrawNativeTokenMappingPda] = findTokenPda(
      ctx.program.programId,
      Buffer.from(EVM_CHAIN_ID),
      EVM_REMOTE_NATIVE_TOKEN
    );
    const wInfo = await ctx.provider.connection.getAccountInfo(
      withdrawNativeTokenMappingPda
    );
    if (!wInfo) {
      await ctx.program.methods
        .registerToken({
          localMint: PublicKey.default,
          destChain: EVM_CHAIN_ID,
          destToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
          mode: { lockUnlock: {} },
          decimals: 9,
          srcDecimals: 18,
        })
        .accounts({
          bridge: ctx.bridgePda,
          tokenMapping: withdrawNativeTokenMappingPda,
          mint: null,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    }
  });

  describe("deposit_native", () => {
    let firstDepositNonce: number;
    let firstDepositPda: PublicKey;

    it("deposits SOL and creates deposit record", async () => {
      const amount = 1 * LAMPORTS_PER_SOL;
      const destAccount = Array.from(Buffer.alloc(32, 0xbb));

      firstDepositNonce = await getNextDepositNonce(ctx);
      [firstDepositPda] = findDepositPda(
        ctx.program.programId,
        firstDepositNonce
      );

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: destAccount,
          amount: new anchor.BN(amount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: firstDepositPda,
          destChainEntry: evmChainPda,
          tokenMapping: depositTokenMappingPda,
          depositor: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const deposit = await ctx.program.account.depositRecord.fetch(
        firstDepositPda
      );
      expect(deposit.nonce.toNumber()).to.equal(firstDepositNonce);
      expect(deposit.srcAccount.toString()).to.equal(
        ctx.user.publicKey.toString()
      );

      const expectedNet = amount - Math.floor((amount * 50) / 10000);
      expect(Number(deposit.amount)).to.equal(expectedNet);

      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      expect(bridge.depositNonce.toNumber()).to.equal(firstDepositNonce);
    });

    it("deposit hash uses chain_id from bridge config", async () => {
      const deposit = await ctx.program.account.depositRecord.fetch(
        firstDepositPda
      );

      const amount = 1 * LAMPORTS_PER_SOL;
      const expectedNet = BigInt(amount - Math.floor((amount * 50) / 10000));

      const expectedHash = computeTransferHash(
        SOLANA_CHAIN_ID,
        EVM_CHAIN_ID,
        ctx.user.publicKey.toBuffer(),
        Buffer.alloc(32, 0xbb),
        DEPOSIT_DEST_TOKEN,
        expectedNet,
        BigInt(firstDepositNonce)
      );

      expect(Buffer.from(deposit.transferHash).toString("hex")).to.equal(
        expectedHash.toString("hex")
      );
    });

    it("rejects zero amount", async () => {
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      try {
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32)),
            amount: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: evmChainPda,
            tokenMapping: depositTokenMappingPda,
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
      const unregisteredChain = [0x00, 0x00, 0x00, 0xff];
      const [fakePda] = findChainPda(
        ctx.program.programId,
        Buffer.from(unregisteredChain)
      );
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      try {
        await ctx.program.methods
          .depositNative({
            destChain: unregisteredChain,
            destAccount: Array.from(Buffer.alloc(32)),
            amount: new anchor.BN(1000000),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: fakePda,
            tokenMapping: depositTokenMappingPda,
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
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: true,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      try {
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32)),
            amount: new anchor.BN(1000000),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: evmChainPda,
            tokenMapping: depositTokenMappingPda,
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
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: false,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });

    it("fee math: 0 bps means no fee", async () => {
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: 0,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const amount = 1 * LAMPORTS_PER_SOL;
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xbb)),
          amount: new anchor.BN(amount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          destChainEntry: evmChainPda,
          tokenMapping: depositTokenMappingPda,
          depositor: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const deposit = await ctx.program.account.depositRecord.fetch(depositPda);
      expect(Number(deposit.amount)).to.equal(amount);

      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: 50,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });

    it("fee math: max 100 bps (1%) leaves net = 99% of gross", async () => {
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: 100,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const amount = 1 * LAMPORTS_PER_SOL;
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xbb)),
          amount: new anchor.BN(amount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          destChainEntry: evmChainPda,
          tokenMapping: depositTokenMappingPda,
          depositor: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const deposit = await ctx.program.account.depositRecord.fetch(depositPda);
      const expectedNet = amount - Math.floor((amount * 100) / 10000);
      expect(Number(deposit.amount)).to.equal(expectedNet);

      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: 50,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });
  });

  describe("withdraw lifecycle: submit -> approve -> delay -> execute_native", () => {
    let transferHash: Buffer;
    /** 0.5 SOL expressed with src_decimals=18 (matches EVM remote native mapping). */
    const withdrawAmount = 500_000_000_000_000_000n;
    const withdrawNonce = 100n;
    const srcChain = EVM_CHAIN_ID;
    const srcAccount = Buffer.alloc(32, 0xaa);
    const destToken = NATIVE_SOL_TOKEN;

    it("submit withdrawal", async () => {
      transferHash = computeTransferHash(
        srcChain,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        withdrawAmount,
        withdrawNonce
      );

      const [withdrawPda] = findWithdrawPda(
        ctx.program.programId,
        transferHash
      );
      const [executedHashPda] = findExecutedHashPda(
        ctx.program.programId,
        transferHash
      );

      await ctx.program.methods
        .withdrawSubmit({
          srcChain: srcChain,
          srcAccount: Array.from(srcAccount),
          srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
          destToken: destToken,
          amount: new anchor.BN(withdrawAmount.toString()),
          nonce: new anchor.BN(Number(withdrawNonce)),
          operatorGas: new anchor.BN(0),
        })
        .accounts({
          bridge: ctx.bridgePda,
          srcChainEntry: evmChainPda,
          tokenMapping: withdrawNativeTokenMappingPda,
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
      expect(pw.amount.toString()).to.equal(withdrawAmount.toString());
    });

    it("non-operator cannot approve", async () => {
      const [withdrawPda] = findWithdrawPda(
        ctx.program.programId,
        transferHash
      );
      const [nonceUsedPda] = findNonceUsedPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        withdrawNonce
      );

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(transferHash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            nonceUsed: nonceUsedPda,
            operator: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedOperator");
      }
    });

    it("operator approves withdrawal", async () => {
      const [withdrawPda] = findWithdrawPda(
        ctx.program.programId,
        transferHash
      );
      const [nonceUsedPda] = findNonceUsedPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        withdrawNonce
      );

      await ctx.program.methods
        .withdrawApprove({ transferHash: Array.from(transferHash) })
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          nonceUsed: nonceUsedPda,
          operator: ctx.operator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.operator])
        .rpc();

      const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      expect(pw.approved).to.be.true;
      expect(pw.approvedAt.toNumber()).to.be.greaterThan(0);
    });

    it("cannot approve twice", async () => {
      const [withdrawPda] = findWithdrawPda(
        ctx.program.programId,
        transferHash
      );
      const [nonceUsedPda] = findNonceUsedPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        withdrawNonce
      );

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(transferHash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            nonceUsed: nonceUsedPda,
            operator: ctx.operator.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.satisfy(
          (s: string) =>
            s.includes("AlreadyApproved") || s.includes("already in use")
        );
      }
    });

    it("execute native withdrawal after delay", async () => {
      await new Promise((r) =>
        setTimeout(r, (WITHDRAW_DELAY_SECONDS + 3) * 1000)
      );

      const [withdrawPda] = findWithdrawPda(
        ctx.program.programId,
        transferHash
      );
      const [executedHashPda] = findExecutedHashPda(
        ctx.program.programId,
        transferHash
      );

      const balanceBefore = await ctx.provider.connection.getBalance(
        ctx.user.publicKey
      );

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, NATIVE_SOL_TOKEN)[0],
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const balanceAfter = await ctx.provider.connection.getBalance(
        ctx.user.publicKey
      );
      expect(balanceAfter).to.be.greaterThan(balanceBefore);

      const executed = await ctx.program.account.executedHash.fetch(
        executedHashPda
      );
      expect(executed).to.not.be.null;
    });

    it("cannot re-submit after execution (close-reinit protection)", async () => {
      const sameHash = transferHash;
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, sameHash);
      const [executedHashPda] = findExecutedHashPda(
        ctx.program.programId,
        sameHash
      );

      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: srcChain,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken: destToken,
            amount: new anchor.BN(withdrawAmount.toString()),
            nonce: new anchor.BN(Number(withdrawNonce)),
            operatorGas: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            srcChainEntry: evmChainPda,
            tokenMapping: withdrawNativeTokenMappingPda,
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
          (s: string) =>
            s.includes("AlreadyExecutedHash") || s.includes("already in use"),
          "Expected AlreadyExecutedHash or account already in use error"
        );
      }
    });
  });

  describe("withdraw_approve rejects when paused", () => {
    it("operator cannot approve when bridge is paused", async () => {
      const srcAccount = Buffer.alloc(32, 0xdd);
      const destToken = NATIVE_SOL_TOKEN;
      const amount = 100000n;
      const nonce = 200n;

      const hash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        amount,
        nonce
      );
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, hash);
      const [executedHashPda] = findExecutedHashPda(
        ctx.program.programId,
        hash
      );

      await ctx.program.methods
        .withdrawSubmit({
          srcChain: EVM_CHAIN_ID,
          srcAccount: Array.from(srcAccount),
          srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
          destToken: destToken,
          amount: new anchor.BN(amount.toString()),
          nonce: new anchor.BN(Number(nonce)),
          operatorGas: new anchor.BN(0),
        })
        .accounts({
          bridge: ctx.bridgePda,
          srcChainEntry: evmChainPda,
          tokenMapping: withdrawNativeTokenMappingPda,
          pendingWithdraw: withdrawPda,
          executedHashCheck: executedHashPda,
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: true,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const [nonceUsedPda] = findNonceUsedPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        nonce
      );

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(hash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            nonceUsed: nonceUsedPda,
            operator: ctx.operator.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }

      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: false,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });
  });
});
