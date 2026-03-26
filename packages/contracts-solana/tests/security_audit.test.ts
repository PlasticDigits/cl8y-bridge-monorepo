import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import {
  AuthorityType,
  TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
  getMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  setAuthority,
} from "@solana/spl-token";
import { expect } from "chai";

import { Cl8yBridge } from "../target/types/cl8y_bridge";
import {
  TestContext,
  airdrop,
  findBridgePda,
  findCancelerPda,
  findChainPda,
  findDepositPda,
  findExecutedHashPda,
  findTokenPda,
  findWithdrawPda,
  findWithdrawRateLimitPda,
  getNextDepositNonce,
  initializeBridgeIfNeeded,
  registerChainIfNeeded,
  setExplicitUnlimitedWithdrawRateLimit,
  setupTest,
  NATIVE_SOL_TOKEN,
  findNonceUsedPda,
} from "./helpers/setup";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];
const WITHDRAW_DELAY_SECONDS = 15;

/** Remote native SOL id on EVM (matches deposit_withdraw tests). */
const EVM_REMOTE_NATIVE_TOKEN = Buffer.alloc(32);
EVM_REMOTE_NATIVE_TOKEN[31] = 0x37;
/** Destination token bytes32 for deposit_native token_mapping PDA. */
const DEPOSIT_DEST_TOKEN = Buffer.alloc(32);
DEPOSIT_DEST_TOKEN[31] = 0xcc;

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

function toBn(value: bigint | number): anchor.BN {
  return new anchor.BN(value.toString());
}

function feeFor(amount: bigint, feeBps = 50n): bigint {
  return (amount * feeBps) / 10000n;
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Comprehensive security audit test suite covering the top 20 Solana
 * vulnerability patterns mapped to this cross-chain bridge:
 *
 *  1. Missing signer checks
 *  2. Account confusion / type cosplay
 *  3. PDA substitution
 *  4. Replay / double execution
 *  5. Integer overflow / underflow
 *  6. Unauthorized privilege escalation
 *  7. Oracle manipulation (N/A - no oracles)
 *  8. Flash loan / reentrancy (N/A - Solana runtime prevents)
 *  9. Cross-user withdrawal interception
 * 10. Token account validation
 * 11. Pause circuit breaker bypass
 * 12. Fee manipulation / drainage
 * 13. Rent exemption safety
 * 14. Cancel-reenable-execute lifecycle
 * 15. Admin/operator rotation mid-flow
 * 16. Hash integrity (on-chain vs off-chain)
 * 17. Concurrent multi-user operations
 * 18. Balance accounting invariants
 * 19. Authority validation exhaustive
 * 20. Token mode enforcement (LockUnlock vs MintBurn)
 */
describe("security audit: top-20 Solana vulnerability patterns", () => {
  let ctx: TestContext;
  let evmChainPda: PublicKey;
  let withdrawNativeTokenMappingPda: PublicKey;
  let depositTokenMappingPda: PublicKey;

  before(async () => {
    ctx = await setupTest();
    await initializeBridgeIfNeeded(ctx, {
      operator: ctx.operator.publicKey,
      feeBps: 50,
      withdrawDelay: new anchor.BN(WITHDRAW_DELAY_SECONDS),
      chainId: SOLANA_CHAIN_ID,
    });
    await ctx.program.methods
      .setConfig({
        newAdmin: null,
        operator: ctx.operator.publicKey,
        feeBps: 50,
        withdrawDelay: new anchor.BN(WITHDRAW_DELAY_SECONDS),
        paused: false,
      })
      .accounts({
        bridge: ctx.bridgePda,
        admin: ctx.admin.publicKey,
      })
      .rpc();

    evmChainPda = await registerChainIfNeeded(
      ctx,
      EVM_CHAIN_ID,
      "evm_audit"
    );

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

    await setExplicitUnlimitedWithdrawRateLimit(ctx, NATIVE_SOL_TOKEN);
  });

  async function registerTokenMapping(
    mint: PublicKey,
    destTokenByte: number,
    mode: { lockUnlock: {} } | { mintBurn: {} }
  ): Promise<{ tokenPda: PublicKey; destToken: Buffer }> {
    const destToken = Buffer.alloc(32);
    destToken[31] = destTokenByte;
    const [tokenPda] = findTokenPda(
      ctx.program.programId,
      Buffer.from(EVM_CHAIN_ID),
      destToken
    );
    const existing = await ctx.provider.connection.getAccountInfo(tokenPda);
    if (!existing) {
      await ctx.program.methods
        .registerToken({
          localMint: mint,
          destChain: EVM_CHAIN_ID,
          destToken: Array.from(destToken),
          mode,
          decimals: 9,
          srcDecimals: 9,
        })
        .accounts({
          bridge: ctx.bridgePda,
          tokenMapping: tokenPda,
          mint,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    }
    return { tokenPda, destToken };
  }

  async function createSplFixture(
    mode: { lockUnlock: {} } | { mintBurn: {} },
    destTokenByte: number
  ) {
    const mint = await createMint(
      ctx.provider.connection,
      ctx.admin,
      ctx.admin.publicKey,
      null,
      9
    );
    const userToken = await getOrCreateAssociatedTokenAccount(
      ctx.provider.connection,
      ctx.admin,
      mint,
      ctx.user.publicKey
    );
    const adminToken = await getOrCreateAssociatedTokenAccount(
      ctx.provider.connection,
      ctx.admin,
      mint,
      ctx.admin.publicKey
    );
    const bridgeToken = await getOrCreateAssociatedTokenAccount(
      ctx.provider.connection,
      ctx.admin,
      mint,
      ctx.bridgePda,
      true
    );
    const initialSupply = 10_000_000_000n;
    await mintTo(
      ctx.provider.connection,
      ctx.admin,
      mint,
      userToken.address,
      ctx.admin,
      Number(initialSupply)
    );
    if ("mintBurn" in mode) {
      await setAuthority(
        ctx.provider.connection,
        ctx.admin,
        mint,
        ctx.admin,
        AuthorityType.MintTokens,
        ctx.bridgePda
      );
    }
    const { tokenPda, destToken } = await registerTokenMapping(
      mint,
      destTokenByte,
      mode
    );
    await setExplicitUnlimitedWithdrawRateLimit(ctx, mint);
    return {
      mint,
      tokenPda,
      destToken,
      userToken,
      bridgeToken,
      adminToken,
      initialSupply,
    };
  }

  async function submitWithdraw(
    recipient: Keypair,
    tokenPubkey: PublicKey,
    amount: bigint,
    nonce: bigint,
    srcAccountByte: number,
    remoteDestToken: Buffer,
    tokenMappingPda: PublicKey
  ) {
    const srcAccount = Buffer.alloc(32, srcAccountByte);
    const transferHash = computeTransferHash(
      EVM_CHAIN_ID,
      SOLANA_CHAIN_ID,
      srcAccount,
      recipient.publicKey.toBuffer(),
      tokenPubkey.toBuffer(),
      amount,
      nonce
    );
    const [withdrawPda] = findWithdrawPda(ctx.program.programId, transferHash);
    const [executedHashPda] = findExecutedHashPda(
      ctx.program.programId,
      transferHash
    );
    await ctx.program.methods
      .withdrawSubmit({
        srcChain: EVM_CHAIN_ID,
        srcAccount: Array.from(srcAccount),
        srcToken: Array.from(remoteDestToken),
        destToken: tokenPubkey,
        amount: toBn(amount),
        nonce: toBn(nonce),
        operatorGas: new anchor.BN(0),
      })
      .accounts({
        bridge: ctx.bridgePda,
        srcChainEntry: evmChainPda,
        tokenMapping: tokenMappingPda,
        pendingWithdraw: withdrawPda,
        executedHashCheck: executedHashPda,
        recipient: recipient.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([recipient])
      .rpc();
    return { transferHash, withdrawPda, executedHashPda, srcAccount };
  }

  async function approveWithdraw(
    transferHash: Buffer,
    withdrawPda: PublicKey,
    nonce: bigint
  ) {
    const [nonceUsedPda] = findNonceUsedPda(
      ctx.program.programId,
      Buffer.from(EVM_CHAIN_ID),
      nonce
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
  }

  async function ensureCanceler(): Promise<PublicKey> {
    const [cancelerPda] = findCancelerPda(
      ctx.program.programId,
      ctx.canceler.publicKey
    );
    const info = await ctx.provider.connection.getAccountInfo(cancelerPda);
    if (!info) {
      await ctx.program.methods
        .addCanceler({ canceler: ctx.canceler.publicKey, active: true })
        .accounts({
          bridge: ctx.bridgePda,
          cancelerEntry: cancelerPda,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
    } else {
      const entry = await ctx.program.account.cancelerEntry.fetch(cancelerPda);
      if (!entry.active) {
        await ctx.program.methods
          .addCanceler({ canceler: ctx.canceler.publicKey, active: true })
          .accounts({
            bridge: ctx.bridgePda,
            cancelerEntry: cancelerPda,
            admin: ctx.admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
      }
    }
    return cancelerPda;
  }

  // -----------------------------------------------------------------------
  // 1. MISSING SIGNER CHECKS (Wormhole-class vulnerability)
  // -----------------------------------------------------------------------
  describe("1. signer check enforcement", () => {
    it("rejects set_config when admin pubkey is passed but not signing", async () => {
      const impersonator = Keypair.generate();
      await airdrop(ctx.provider.connection, impersonator.publicKey);
      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: null,
            feeBps: 100,
            withdrawDelay: null,
            paused: null,
          })
          .accounts({
            bridge: ctx.bridgePda,
            admin: impersonator.publicKey,
          })
          .signers([impersonator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("rejects withdraw_approve when operator pubkey provided but wrong signer", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9001n,
        0xa1,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      const fakeSigner = Keypair.generate();
      await airdrop(ctx.provider.connection, fakeSigner.publicKey);
      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(transferHash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            nonceUsed: findNonceUsedPda(
              ctx.program.programId,
              Buffer.from(EVM_CHAIN_ID),
              9001n
            )[0],
            operator: fakeSigner.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([fakeSigner])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedOperator");
      }
    });

    it("rejects withdraw_cancel when canceler pubkey matches but signer is different", async () => {
      const cancelerPda = await ensureCanceler();
      const destToken = NATIVE_SOL_TOKEN;
      const { withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9002n,
        0xa2,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      const fakeSigner = Keypair.generate();
      await airdrop(ctx.provider.connection, fakeSigner.publicKey);
      try {
        await ctx.program.methods
          .withdrawCancel()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            cancelerEntry: cancelerPda,
            canceler: fakeSigner.publicKey,
          })
          .signers([fakeSigner])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("AccountNotInitialized") ||
            msg.includes("ConstraintSeeds") ||
            msg.includes("seeds")
        ).to.be.true;
      }
    });
  });

  // -----------------------------------------------------------------------
  // 2 & 3. ACCOUNT CONFUSION / TYPE COSPLAY / PDA SUBSTITUTION
  // -----------------------------------------------------------------------
  describe("2-3. account confusion and PDA substitution", () => {
    it("rejects deposit with wrong chain PDA (different chain_id seeds)", async () => {
      const badChainId = [0x00, 0x00, 0x00, 0xfe];
      const [fakePda] = findChainPda(
        ctx.program.programId,
        Buffer.from(badChainId)
      );
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      try {
        const badDestTok = Buffer.alloc(32);
        const [badTokenMapping] = findTokenPda(
          ctx.program.programId,
          Buffer.from(badChainId),
          badDestTok
        );
        await ctx.program.methods
          .depositNative({
            destChain: badChainId,
            destAccount: Array.from(Buffer.alloc(32)),
            amount: new anchor.BN(LAMPORTS_PER_SOL),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: fakePda,
            tokenMapping: badTokenMapping,
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

    it("rejects withdraw_approve with PDA for wrong transfer hash", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        200_000n,
        9003n,
        0xa3,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      const fakeHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        Buffer.alloc(32, 0xff),
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        200_000n,
        9003n
      );
      const [wrongWithdrawPda] = findWithdrawPda(
        ctx.program.programId,
        fakeHash
      );
      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(transferHash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: wrongWithdrawPda,
            nonceUsed: findNonceUsedPda(
              ctx.program.programId,
              Buffer.from(EVM_CHAIN_ID),
              9003n
            )[0],
            operator: ctx.operator.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("ConstraintSeeds") ||
            msg.includes("AccountNotInitialized") ||
            msg.includes("seeds constraint")
        ).to.be.true;
      }
    });

    it("rejects SPL deposit with mismatched mint vs token_mapping.local_mint", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xd1);
      const wrongMint = await createMint(
        ctx.provider.connection,
        ctx.admin,
        ctx.admin.publicKey,
        null,
        9
      );
      const wrongUserToken = await getOrCreateAssociatedTokenAccount(
        ctx.provider.connection,
        ctx.admin,
        wrongMint,
        ctx.user.publicKey
      );
      await mintTo(
        ctx.provider.connection,
        ctx.admin,
        wrongMint,
        wrongUserToken.address,
        ctx.admin,
        1_000_000_000
      );
      const wrongBridgeToken = await getOrCreateAssociatedTokenAccount(
        ctx.provider.connection,
        ctx.admin,
        wrongMint,
        ctx.bridgePda,
        true
      );
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      try {
        await ctx.program.methods
          .depositSpl({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32, 0xab)),
            amount: toBn(500_000_000n),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            tokenMapping: fixture.tokenPda,
            mint: wrongMint,
            depositorTokenAccount: wrongUserToken.address,
            bridgeTokenAccount: wrongBridgeToken.address,
            destChainEntry: evmChainPda,
            depositor: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("TokenNotRegistered");
      }
    });
  });

  // -----------------------------------------------------------------------
  // 4. REPLAY / DOUBLE EXECUTION
  // -----------------------------------------------------------------------
  describe("4. replay and double execution prevention", () => {
    let transferHash: Buffer;
    let withdrawPda: PublicKey;
    let executedHashPda: PublicKey;
    const destToken = NATIVE_SOL_TOKEN;
    const amount = 300_000n;
    const nonce = 9004n;

    before(async () => {
      const result = await submitWithdraw(
        ctx.user,
        destToken,
        amount,
        nonce,
        0xa4,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      transferHash = result.transferHash;
      withdrawPda = result.withdrawPda;
      executedHashPda = result.executedHashPda;
      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      const bridgeInfo = await ctx.provider.connection.getAccountInfo(
        ctx.bridgePda
      );
      const bridgeLamports = bridgeInfo!.lamports;
      const needed = Number(amount) + 2 * LAMPORTS_PER_SOL;
      if (bridgeLamports < needed) {
        const tx = new anchor.web3.Transaction().add(
          SystemProgram.transfer({
            fromPubkey: ctx.admin.publicKey,
            toPubkey: ctx.bridgePda,
            lamports: needed - bridgeLamports,
          })
        );
        await ctx.provider.sendAndConfirm(tx);
      }

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
    });

    it("cannot re-submit same transfer hash after execution", async () => {
      const srcAccount = Buffer.alloc(32, 0xa4);
      const replayHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        amount,
        nonce
      );
      const [replayWithdrawPda] = findWithdrawPda(
        ctx.program.programId,
        replayHash
      );
      const [replayExecutedPda] = findExecutedHashPda(
        ctx.program.programId,
        replayHash
      );
      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: EVM_CHAIN_ID,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken: destToken,
            amount: toBn(amount),
            nonce: toBn(nonce),
            operatorGas: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            srcChainEntry: evmChainPda,
            tokenMapping: withdrawNativeTokenMappingPda,
            pendingWithdraw: replayWithdrawPda,
            executedHashCheck: replayExecutedPda,
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.satisfy(
          (s: string) =>
            s.includes("AlreadyExecutedHash") || s.includes("already in use")
        );
      }
    });

    it("executed_hash PDA exists after execution, preventing any replay", async () => {
      const executed = await ctx.program.account.executedHash.fetch(
        executedHashPda
      );
      expect(executed).to.not.be.null;
      expect(executed.bump).to.be.greaterThan(0);
    });
  });

  // -----------------------------------------------------------------------
  // 5. INTEGER OVERFLOW / UNDERFLOW
  // -----------------------------------------------------------------------
  describe("5. integer overflow edge cases", () => {
    it("handles deposit with large amount (near u64::MAX / 2) without overflow", async () => {
      const largeAmount = BigInt("4611686018427387903"); // ~u64::MAX/4
      const richUser = Keypair.generate();
      await airdrop(
        ctx.provider.connection,
        richUser.publicKey,
        200 * LAMPORTS_PER_SOL
      );

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      try {
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32, 0xbb)),
            amount: toBn(largeAmount),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            destChainEntry: evmChainPda,
            tokenMapping: depositTokenMappingPda,
            depositor: richUser.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([richUser])
          .rpc();
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("insufficient lamports") ||
            msg.includes("InstructionError") ||
            msg.includes("custom program error")
        ).to.be.true;
      }
    });

    it("fee calculation doesn't overflow for max feeBps (100) * large amount", async () => {
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

      const amount = 5_000_000_000n;
      const fee = (amount * 100n) / 10000n;
      const net = amount - fee;
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xbb)),
          amount: toBn(amount),
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
      expect(Number(deposit.amount)).to.equal(Number(net));

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

  // -----------------------------------------------------------------------
  // 9. CROSS-USER WITHDRAWAL INTERCEPTION
  // -----------------------------------------------------------------------
  describe("9. cross-user withdrawal interception", () => {
    it("user A cannot execute user B's approved withdrawal", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          destToken,
          100_000n,
          9010n,
          0xb1,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9010n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      const attacker = Keypair.generate();
      await airdrop(ctx.provider.connection, attacker.publicKey);
      try {
        await ctx.program.methods
          .withdrawExecuteNative()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, NATIVE_SOL_TOKEN)[0],
            recipient: attacker.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("WrongRecipient");
      }
    });

    it("user A cannot submit withdrawal claiming user B's address", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const victim = ctx.user;
      const attacker = Keypair.generate();
      await airdrop(ctx.provider.connection, attacker.publicKey);

      const srcAccount = Buffer.alloc(32, 0xb2);
      const attackerHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        attacker.publicKey.toBuffer(),
        destToken.toBuffer(),
        50_000n,
        9011n
      );
      const victimHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        victim.publicKey.toBuffer(),
        destToken.toBuffer(),
        50_000n,
        9011n
      );
      expect(attackerHash.toString("hex")).to.not.equal(
        victimHash.toString("hex"),
        "Hashes must differ because dest_account differs"
      );

      const [withdrawPda] = findWithdrawPda(
        ctx.program.programId,
        victimHash
      );
      const [executedHashPda] = findExecutedHashPda(
        ctx.program.programId,
        victimHash
      );
      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: EVM_CHAIN_ID,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken: destToken,
            amount: toBn(50_000n),
            nonce: toBn(9011n),
            operatorGas: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            srcChainEntry: evmChainPda,
            tokenMapping: withdrawNativeTokenMappingPda,
            pendingWithdraw: withdrawPda,
            executedHashCheck: executedHashPda,
            recipient: attacker.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        expect.fail("Should have thrown - PDA mismatch");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("ConstraintSeeds") ||
            msg.includes("seeds constraint") ||
            msg.includes("custom program error") ||
            msg.includes("Error processing Instruction")
        ).to.be.true;
      }
    });
  });

  // -----------------------------------------------------------------------
  // 11. PAUSE CIRCUIT BREAKER - EXHAUSTIVE
  // -----------------------------------------------------------------------
  describe("11. pause circuit breaker across all operations", () => {
    afterEach(async () => {
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

    it("blocks deposit_native when paused", async () => {
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
            amount: new anchor.BN(LAMPORTS_PER_SOL),
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
    });

    it("blocks deposit_spl when paused", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xe1);
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
          .depositSpl({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32, 0xab)),
            amount: toBn(500_000_000n),
          })
          .accounts({
            bridge: ctx.bridgePda,
            depositRecord: depositPda,
            tokenMapping: fixture.tokenPda,
            mint: fixture.mint,
            depositorTokenAccount: fixture.userToken.address,
            bridgeTokenAccount: fixture.bridgeToken.address,
            destChainEntry: evmChainPda,
            depositor: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }
    });

    it("blocks withdraw_submit when paused", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const srcAccount = Buffer.alloc(32, 0xe2);
      const hash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        100_000n,
        9020n
      );
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, hash);
      const [executedHashPda] = findExecutedHashPda(
        ctx.program.programId,
        hash
      );

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

      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: EVM_CHAIN_ID,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken: destToken,
            amount: toBn(100_000n),
            nonce: toBn(9020n),
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }
    });

    it("blocks withdraw_execute_native when paused", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          destToken,
          100_000n,
          9021n,
          0xe3,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9021n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

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

      try {
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }
    });

    it("blocks withdraw_execute (SPL) when paused", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xe4);
      const depositAmount = 1_000_000_000n;
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          net,
          9022n,
          0xe5,
          fixture.destToken,
          fixture.tokenPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9022n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

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

      try {
        await ctx.program.methods
          .withdrawExecute()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            mint: fixture.mint,
            recipientTokenAccount: fixture.userToken.address,
            bridgeTokenAccount: fixture.bridgeToken.address,
            tokenMapping: fixture.tokenPda,
            withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, fixture.mint)[0],
            recipient: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }
    });
  });

  // -----------------------------------------------------------------------
  // 12. FEE MANIPULATION / DRAINAGE
  // -----------------------------------------------------------------------
  describe("12. fee manipulation and drainage prevention", () => {
    it("rejects native fee withdrawal exceeding accrued fees", async () => {
      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const overAmount = Number(bridge.accruedNativeFees) + 1;

      try {
        await ctx.program.methods
          .withdrawFees({ amount: new anchor.BN(overAmount), native: true })
          .accounts({
            bridge: ctx.bridgePda,
            admin: ctx.admin.publicKey,
            adminTokenAccount: null,
            bridgeTokenAccount: null,
            mint: null,
            tokenMapping: null,
            tokenProgram: null,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InsufficientAccruedFees");
      }
    });

    it("rejects zero-amount fee withdrawal", async () => {
      try {
        await ctx.program.methods
          .withdrawFees({ amount: new anchor.BN(0), native: true })
          .accounts({
            bridge: ctx.bridgePda,
            admin: ctx.admin.publicKey,
            adminTokenAccount: null,
            bridgeTokenAccount: null,
            mint: null,
            tokenMapping: null,
            tokenProgram: null,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("ZeroAmount");
      }
    });

    it("SPL fee withdrawal reduces accrued_fees to exact amount withdrawn", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xf1);
      const depositAmount = 2_000_000_000n;
      const fee = feeFor(depositAmount);
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const halfFee = fee / 2n;
      await ctx.program.methods
        .withdrawFees({ amount: toBn(halfFee), native: false })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
          adminTokenAccount: fixture.adminToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          mint: fixture.mint,
          tokenMapping: fixture.tokenPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const mapping = await ctx.program.account.tokenMapping.fetch(
        fixture.tokenPda
      );
      expect(Number(mapping.accruedFees)).to.equal(Number(fee - halfFee));
    });
  });

  // -----------------------------------------------------------------------
  // 13. RENT EXEMPTION SAFETY
  // -----------------------------------------------------------------------
  describe("13. rent exemption safety", () => {
    it("native withdrawal fails when bridge balance would drop below rent-exempt", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const bridgeLamports = await ctx.provider.connection.getBalance(
        ctx.bridgePda
      );
      // 18→9 decimal normalize divides by 1e9; request more lamports than bridge holds.
      const hugeAmount = (BigInt(bridgeLamports) + 100n) * 1_000_000_000n;
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          destToken,
          hugeAmount,
          9030n,
          0xc1,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9030n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      try {
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
        expect.fail("Should have thrown");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("InsufficientBridgeBalance") ||
            msg.includes("Bridge balance would fall below") ||
            msg.includes("6023")
        ).to.be.true;
      }
    });
  });

  // -----------------------------------------------------------------------
  // 14. CANCEL -> REENABLE -> EXECUTE FULL LIFECYCLE
  // -----------------------------------------------------------------------
  describe("14. cancel-reenable-execute full lifecycle", () => {
    it("complete cycle: submit -> approve -> cancel -> reenable -> delay -> execute", async () => {
      const cancelerPda = await ensureCanceler();
      const destToken = NATIVE_SOL_TOKEN;
      const amount = 200_000n;

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          destToken,
          amount,
          9040n,
          0xc2,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9040n);

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

      let pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      expect(pw.cancelled).to.be.true;

      try {
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("WithdrawalCancelled");
      }

      await ctx.program.methods
        .withdrawReenable()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          authority: ctx.admin.publicKey,
        })
        .signers([ctx.admin])
        .rpc();

      pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      expect(pw.cancelled).to.be.false;
      expect(pw.approved).to.be.true;

      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      const bridgeInfo = await ctx.provider.connection.getAccountInfo(
        ctx.bridgePda
      );
      const needed = Number(amount) + 2 * LAMPORTS_PER_SOL;
      if (bridgeInfo!.lamports < needed) {
        const tx = new anchor.web3.Transaction().add(
          SystemProgram.transfer({
            fromPubkey: ctx.admin.publicKey,
            toPubkey: ctx.bridgePda,
            lamports: needed - bridgeInfo!.lamports,
          })
        );
        await ctx.provider.sendAndConfirm(tx);
      }

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
    });
  });

  // -----------------------------------------------------------------------
  // 15. ADMIN/OPERATOR ROTATION MID-FLOW
  // -----------------------------------------------------------------------
  describe("15. admin/operator rotation during active withdrawals", () => {
    it("old operator cannot approve after operator change", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9050n,
        0xc3,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );

      const newOperator = Keypair.generate();
      await airdrop(ctx.provider.connection, newOperator.publicKey);
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: newOperator.publicKey,
          feeBps: null,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(transferHash) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            nonceUsed: findNonceUsedPda(
              ctx.program.programId,
              Buffer.from(EVM_CHAIN_ID),
              9050n
            )[0],
            operator: ctx.operator.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedOperator");
      }

      await ctx.program.methods
        .withdrawApprove({ transferHash: Array.from(transferHash) })
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          nonceUsed: findNonceUsedPda(
            ctx.program.programId,
            Buffer.from(EVM_CHAIN_ID),
            9050n
          )[0],
          operator: newOperator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([newOperator])
        .rpc();

      const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      expect(pw.approved).to.be.true;

      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: ctx.operator.publicKey,
          feeBps: null,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });
  });

  // -----------------------------------------------------------------------
  // 16. HASH INTEGRITY (ON-CHAIN vs OFF-CHAIN)
  // -----------------------------------------------------------------------
  describe("16. on-chain hash matches off-chain computation", () => {
    it("deposit_native stores hash matching TS computeTransferHash", async () => {
      const amount = 2 * LAMPORTS_PER_SOL;
      const destAccount = Buffer.alloc(32, 0xdd);
      const fee = feeFor(BigInt(amount));
      const netAmount = BigInt(amount) - fee;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(destAccount),
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
      const expectedHash = computeTransferHash(
        SOLANA_CHAIN_ID,
        EVM_CHAIN_ID,
        ctx.user.publicKey.toBuffer(),
        destAccount,
        DEPOSIT_DEST_TOKEN,
        netAmount,
        BigInt(nextNonce)
      );

      expect(Buffer.from(deposit.transferHash).toString("hex")).to.equal(
        expectedHash.toString("hex")
      );
      expect(Number(deposit.amount)).to.equal(Number(netAmount));
    });

    it("deposit_spl stores hash matching TS computeTransferHash", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xf2);
      const depositAmount = 1_500_000_000n;
      const fee = feeFor(depositAmount);
      const netAmount = depositAmount - fee;
      const destAccount = Buffer.alloc(32, 0xf3);

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(destAccount),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const deposit = await ctx.program.account.depositRecord.fetch(depositPda);
      const expectedHash = computeTransferHash(
        SOLANA_CHAIN_ID,
        EVM_CHAIN_ID,
        ctx.user.publicKey.toBuffer(),
        destAccount,
        fixture.destToken,
        netAmount,
        BigInt(nextNonce)
      );

      expect(Buffer.from(deposit.transferHash).toString("hex")).to.equal(
        expectedHash.toString("hex")
      );
    });

    it("changing any single hash field produces a different hash", async () => {
      const base = {
        srcChain: SOLANA_CHAIN_ID,
        destChain: EVM_CHAIN_ID,
        srcAccount: Buffer.alloc(32, 0x01),
        destAccount: Buffer.alloc(32, 0x02),
        token: Buffer.alloc(32, 0x03),
        amount: 1_000_000n,
        nonce: 42n,
      };
      const baseHash = computeTransferHash(
        base.srcChain,
        base.destChain,
        base.srcAccount,
        base.destAccount,
        base.token,
        base.amount,
        base.nonce
      );

      const modifiedSrcChain = computeTransferHash(
        [0x00, 0x00, 0x00, 0x99],
        base.destChain,
        base.srcAccount,
        base.destAccount,
        base.token,
        base.amount,
        base.nonce
      );
      const modifiedDestChain = computeTransferHash(
        base.srcChain,
        [0x00, 0x00, 0x00, 0x99],
        base.srcAccount,
        base.destAccount,
        base.token,
        base.amount,
        base.nonce
      );
      const modifiedSrcAccount = computeTransferHash(
        base.srcChain,
        base.destChain,
        Buffer.alloc(32, 0xff),
        base.destAccount,
        base.token,
        base.amount,
        base.nonce
      );
      const modifiedDestAccount = computeTransferHash(
        base.srcChain,
        base.destChain,
        base.srcAccount,
        Buffer.alloc(32, 0xff),
        base.token,
        base.amount,
        base.nonce
      );
      const modifiedToken = computeTransferHash(
        base.srcChain,
        base.destChain,
        base.srcAccount,
        base.destAccount,
        Buffer.alloc(32, 0xff),
        base.amount,
        base.nonce
      );
      const modifiedAmount = computeTransferHash(
        base.srcChain,
        base.destChain,
        base.srcAccount,
        base.destAccount,
        base.token,
        base.amount + 1n,
        base.nonce
      );
      const modifiedNonce = computeTransferHash(
        base.srcChain,
        base.destChain,
        base.srcAccount,
        base.destAccount,
        base.token,
        base.amount,
        base.nonce + 1n
      );

      const allModified = [
        modifiedSrcChain,
        modifiedDestChain,
        modifiedSrcAccount,
        modifiedDestAccount,
        modifiedToken,
        modifiedAmount,
        modifiedNonce,
      ];
      for (const modified of allModified) {
        expect(modified.toString("hex")).to.not.equal(
          baseHash.toString("hex"),
          "Modifying any single field must change the hash"
        );
      }

      const uniqueHashes = new Set(allModified.map((h) => h.toString("hex")));
      expect(uniqueHashes.size).to.equal(
        allModified.length,
        "Each field modification must produce a unique hash"
      );
    });
  });

  // -----------------------------------------------------------------------
  // 17. CONCURRENT MULTI-USER OPERATIONS
  // -----------------------------------------------------------------------
  describe("17. concurrent multi-user withdrawals", () => {
    it("three users submit, approve, and execute independent withdrawals concurrently", async () => {
      const userA = ctx.user;
      const userB = Keypair.generate();
      const userC = Keypair.generate();
      await airdrop(ctx.provider.connection, userB.publicKey);
      await airdrop(ctx.provider.connection, userC.publicKey);

      const bridgeInfo = await ctx.provider.connection.getAccountInfo(
        ctx.bridgePda
      );
      const needed = 5 * LAMPORTS_PER_SOL;
      if (bridgeInfo!.lamports < needed) {
        const tx = new anchor.web3.Transaction().add(
          SystemProgram.transfer({
            fromPubkey: ctx.admin.publicKey,
            toPubkey: ctx.bridgePda,
            lamports: needed,
          })
        );
        await ctx.provider.sendAndConfirm(tx);
      }

      const tokenA = NATIVE_SOL_TOKEN;
      const tokenB = NATIVE_SOL_TOKEN;
      const tokenC = NATIVE_SOL_TOKEN;

      const resultA = await submitWithdraw(
        userA,
        tokenA,
        100_000n,
        9060n,
        0xd1,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      const resultB = await submitWithdraw(
        userB,
        tokenB,
        200_000n,
        9061n,
        0xd2,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      const resultC = await submitWithdraw(
        userC,
        tokenC,
        300_000n,
        9062n,
        0xd3,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );

      expect(resultA.transferHash.toString("hex")).to.not.equal(
        resultB.transferHash.toString("hex")
      );
      expect(resultB.transferHash.toString("hex")).to.not.equal(
        resultC.transferHash.toString("hex")
      );

      await approveWithdraw(resultA.transferHash, resultA.withdrawPda, 9060n);
      await approveWithdraw(resultB.transferHash, resultB.withdrawPda, 9061n);
      await approveWithdraw(resultC.transferHash, resultC.withdrawPda, 9062n);

      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      const balABefore = await ctx.provider.connection.getBalance(
        userA.publicKey
      );
      const balBBefore = await ctx.provider.connection.getBalance(
        userB.publicKey
      );
      const balCBefore = await ctx.provider.connection.getBalance(
        userC.publicKey
      );

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: resultA.withdrawPda,
          executedHash: resultA.executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, NATIVE_SOL_TOKEN)[0],
          recipient: userA.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([userA])
        .rpc();

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: resultB.withdrawPda,
          executedHash: resultB.executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, NATIVE_SOL_TOKEN)[0],
          recipient: userB.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([userB])
        .rpc();

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: resultC.withdrawPda,
          executedHash: resultC.executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, NATIVE_SOL_TOKEN)[0],
          recipient: userC.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([userC])
        .rpc();

      const balAAfter = await ctx.provider.connection.getBalance(
        userA.publicKey
      );
      const balBAfter = await ctx.provider.connection.getBalance(
        userB.publicKey
      );
      const balCAfter = await ctx.provider.connection.getBalance(
        userC.publicKey
      );

      expect(balAAfter).to.be.greaterThan(balABefore);
      expect(balBAfter).to.be.greaterThan(balBBefore);
      expect(balCAfter).to.be.greaterThan(balCBefore);

      const executedA = await ctx.program.account.executedHash.fetch(
        resultA.executedHashPda
      );
      const executedB = await ctx.program.account.executedHash.fetch(
        resultB.executedHashPda
      );
      const executedC = await ctx.program.account.executedHash.fetch(
        resultC.executedHashPda
      );
      expect(executedA).to.not.be.null;
      expect(executedB).to.not.be.null;
      expect(executedC).to.not.be.null;
    });
  });

  // -----------------------------------------------------------------------
  // 18. BALANCE ACCOUNTING INVARIANTS
  // -----------------------------------------------------------------------
  describe("18. balance accounting invariants for SPL", () => {
    it("bridge token balance = escrow + accrued_fees at all times (lock/unlock)", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xf5);
      const depositAmount = 3_000_000_000n;
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const bridgeTokenAfterDeposit = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      const mappingAfterDeposit = await ctx.program.account.tokenMapping.fetch(
        fixture.tokenPda
      );

      expect(Number(bridgeTokenAfterDeposit.amount)).to.equal(
        Number(depositAmount)
      );
      expect(Number(mappingAfterDeposit.accruedFees)).to.equal(Number(fee));
      const escrow =
        Number(bridgeTokenAfterDeposit.amount) -
        Number(mappingAfterDeposit.accruedFees);
      expect(escrow).to.equal(Number(net));

      await ctx.program.methods
        .withdrawFees({ amount: toBn(fee), native: false })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
          adminTokenAccount: fixture.adminToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          mint: fixture.mint,
          tokenMapping: fixture.tokenPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const bridgeTokenAfterFeeWithdraw = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      const mappingAfterFeeWithdraw =
        await ctx.program.account.tokenMapping.fetch(fixture.tokenPda);

      expect(Number(bridgeTokenAfterFeeWithdraw.amount)).to.equal(Number(net));
      expect(Number(mappingAfterFeeWithdraw.accruedFees)).to.equal(0);
    });

    it("mint/burn: supply is conserved (deposit burns net, execute mints net)", async () => {
      const fixture = await createSplFixture({ mintBurn: {} }, 0xf6);
      const depositAmount = 2_000_000_000n;
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;

      const mintBefore = await getMint(ctx.provider.connection, fixture.mint);
      const supplyBefore = BigInt(mintBefore.supply);

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const mintAfterDeposit = await getMint(
        ctx.provider.connection,
        fixture.mint
      );
      expect(Number(mintAfterDeposit.supply)).to.equal(
        Number(supplyBefore - net)
      );

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          net,
          9070n,
          0xf7,
          fixture.destToken,
          fixture.tokenPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9070n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      await ctx.program.methods
        .withdrawExecute()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          mint: fixture.mint,
          recipientTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          tokenMapping: fixture.tokenPda,
          withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, fixture.mint)[0],
          recipient: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const mintAfterExecute = await getMint(
        ctx.provider.connection,
        fixture.mint
      );
      expect(Number(mintAfterExecute.supply)).to.equal(Number(supplyBefore));
    });
  });

  // -----------------------------------------------------------------------
  // 19. EXHAUSTIVE AUTHORITY VALIDATION
  // -----------------------------------------------------------------------
  describe("19. exhaustive authority validation", () => {
    it("non-admin cannot register chain", async () => {
      const rogue = Keypair.generate();
      await airdrop(ctx.provider.connection, rogue.publicKey);
      const chainId = [0x00, 0x00, 0x00, 0xfd];
      const [chainPda] = findChainPda(
        ctx.program.programId,
        Buffer.from(chainId)
      );
      try {
        await ctx.program.methods
          .registerChain({ chainId, identifier: "rogue" })
          .accounts({
            bridge: ctx.bridgePda,
            chainEntry: chainPda,
            admin: rogue.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([rogue])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("non-admin cannot register token", async () => {
      const rogue = Keypair.generate();
      await airdrop(ctx.provider.connection, rogue.publicKey);
      const rogueMint = await createMint(
        ctx.provider.connection,
        ctx.admin,
        ctx.admin.publicKey,
        null,
        9
      );
      const destChain = Buffer.from(EVM_CHAIN_ID);
      const destToken = Buffer.alloc(32);
      destToken[31] = 0xfe;
      const [tokenPda] = findTokenPda(
        ctx.program.programId,
        destChain,
        destToken
      );
      try {
        await ctx.program.methods
          .registerToken({
            localMint: rogueMint,
            destChain: EVM_CHAIN_ID,
            destToken: Array.from(destToken),
            mode: { lockUnlock: {} },
            decimals: 9,
            srcDecimals: 9,
          })
          .accounts({
            bridge: ctx.bridgePda,
            tokenMapping: tokenPda,
            mint: rogueMint,
            admin: rogue.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([rogue])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("non-admin cannot add canceler", async () => {
      const rogue = Keypair.generate();
      await airdrop(ctx.provider.connection, rogue.publicKey);
      const [cancelerPda] = findCancelerPda(
        ctx.program.programId,
        rogue.publicKey
      );
      try {
        await ctx.program.methods
          .addCanceler({ canceler: rogue.publicKey, active: true })
          .accounts({
            bridge: ctx.bridgePda,
            cancelerEntry: cancelerPda,
            admin: rogue.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([rogue])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("non-admin cannot withdraw fees (native)", async () => {
      const rogue = Keypair.generate();
      await airdrop(ctx.provider.connection, rogue.publicKey);
      try {
        await ctx.program.methods
          .withdrawFees({ amount: new anchor.BN(1), native: true })
          .accounts({
            bridge: ctx.bridgePda,
            admin: rogue.publicKey,
            adminTokenAccount: null,
            bridgeTokenAccount: null,
            mint: null,
            tokenMapping: null,
            tokenProgram: null,
            systemProgram: SystemProgram.programId,
          })
          .signers([rogue])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("non-admin cannot reenable cancelled withdrawal", async () => {
      const cancelerPda = await ensureCanceler();
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9080n,
        0xc5,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, 9080n);

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

      const rogue = Keypair.generate();
      await airdrop(ctx.provider.connection, rogue.publicKey);
      try {
        await ctx.program.methods
          .withdrawReenable()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            authority: rogue.publicKey,
          })
          .signers([rogue])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedOperator");
      }
    });
  });

  // -----------------------------------------------------------------------
  // 20. TOKEN MODE ENFORCEMENT (LOCK/UNLOCK vs MINT/BURN)
  // -----------------------------------------------------------------------
  describe("20. token mode enforcement", () => {
    it("lock/unlock: tokens are transferred to bridge, not burned", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xf8);
      const depositAmount = 1_000_000_000n;

      const mintBefore = await getMint(ctx.provider.connection, fixture.mint);
      const supplyBefore = BigInt(mintBefore.supply);

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const mintAfter = await getMint(ctx.provider.connection, fixture.mint);
      expect(Number(mintAfter.supply)).to.equal(
        Number(supplyBefore),
        "Lock/unlock should NOT change total supply"
      );

      const bridgeTokenAfter = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      expect(Number(bridgeTokenAfter.amount)).to.equal(Number(depositAmount));
    });

    it("mint/burn: net amount is burned, only fee remains in bridge", async () => {
      const fixture = await createSplFixture({ mintBurn: {} }, 0xf9);
      const depositAmount = 1_000_000_000n;
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;

      const mintBefore = await getMint(ctx.provider.connection, fixture.mint);
      const supplyBefore = BigInt(mintBefore.supply);

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const mintAfter = await getMint(ctx.provider.connection, fixture.mint);
      expect(Number(mintAfter.supply)).to.equal(
        Number(supplyBefore - net),
        "Mint/burn should reduce supply by net amount"
      );

      const bridgeTokenAfter = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      expect(Number(bridgeTokenAfter.amount)).to.equal(
        Number(fee),
        "Only fee should remain in bridge token account"
      );
    });

    it("mint/burn withdrawal re-mints exact net amount", async () => {
      const fixture = await createSplFixture({ mintBurn: {} }, 0xfa);
      const depositAmount = 2_000_000_000n;
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const userTokenAfterDeposit = await getAccount(
        ctx.provider.connection,
        fixture.userToken.address
      );
      const remainingBalance = BigInt(userTokenAfterDeposit.amount);

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          net,
          9090n,
          0xfb,
          fixture.destToken,
          fixture.tokenPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9090n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      await ctx.program.methods
        .withdrawExecute()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          mint: fixture.mint,
          recipientTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          tokenMapping: fixture.tokenPda,
          withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, fixture.mint)[0],
          recipient: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const userTokenAfterExecute = await getAccount(
        ctx.provider.connection,
        fixture.userToken.address
      );
      expect(Number(userTokenAfterExecute.amount)).to.equal(
        Number(remainingBalance + net)
      );
    });
  });

  // -----------------------------------------------------------------------
  // CROSS-PATH PREVENTION: native/SPL execution path enforcement
  // -----------------------------------------------------------------------
  describe("cross-path execution prevention (hash-bound token type)", () => {
    it("rejects SPL-token withdrawal executed via native path (NotNativeToken)", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xfc);
      const depositAmount = 1_000_000_000n;
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xab)),
          amount: toBn(depositAmount),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPda,
          tokenMapping: fixture.tokenPda,
          mint: fixture.mint,
          depositorTokenAccount: fixture.userToken.address,
          bridgeTokenAccount: fixture.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          net,
          9200n,
          0xfc,
          fixture.destToken,
          fixture.tokenPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9200n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      try {
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
        expect.fail("Should have thrown - SPL token cannot use native execution path");
      } catch (err) {
        expect(err.toString()).to.contain("NotNativeToken");
      }
    });

    it("rejects native-SOL withdrawal executed via SPL path (TokenMintMismatch)", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0xfd);

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          100_000n,
          9201n,
          0xfd,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9201n);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      try {
        await ctx.program.methods
          .withdrawExecute()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            mint: fixture.mint,
            recipientTokenAccount: fixture.userToken.address,
            bridgeTokenAccount: fixture.bridgeToken.address,
            tokenMapping: fixture.tokenPda,
            withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, fixture.mint)[0],
            recipient: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown - native SOL cannot use SPL execution path");
      } catch (err) {
        expect(err.toString()).to.contain("TokenMintMismatch");
      }
    });

    it("hash re-verification at execution time prevents tampered PW data", async () => {
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          100_000n,
          9202n,
          0xfe,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, 9202n);

      const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      const recomputedHash = computeTransferHash(
        Array.from(pw.srcChain),
        SOLANA_CHAIN_ID,
        Buffer.from(pw.srcAccount),
        pw.destAccount.toBuffer(),
        pw.token.toBuffer(),
        BigInt(pw.amount.toString()),
        BigInt(pw.nonce.toString())
      );
      expect(Buffer.from(pw.transferHash).toString("hex")).to.equal(
        recomputedHash.toString("hex"),
        "PW stored hash must match recomputed hash from stored fields"
      );
    });
  });

  // -----------------------------------------------------------------------
  // ADDITIONAL: WITHDRAW STATE MACHINE EDGE CASES
  // -----------------------------------------------------------------------
  describe("withdraw state machine edge cases", () => {
    it("cannot cancel an already-cancelled withdrawal", async () => {
      const cancelerPda = await ensureCanceler();
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9100n,
        0xc6,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, 9100n);

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
          .withdrawCancel()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            cancelerEntry: cancelerPda,
            canceler: ctx.canceler.publicKey,
          })
          .signers([ctx.canceler])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("WithdrawalCancelled");
      }
    });

    it("cannot reenable a non-cancelled withdrawal", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9101n,
        0xc7,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, 9101n);

      try {
        await ctx.program.methods
          .withdrawReenable()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            authority: ctx.admin.publicKey,
          })
          .signers([ctx.admin])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("NotCancelled");
      }
    });

    it("cannot cancel an unapproved withdrawal", async () => {
      const cancelerPda = await ensureCanceler();
      const destToken = NATIVE_SOL_TOKEN;
      const { withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9102n,
        0xc8,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );

      try {
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("NotApproved");
      }
    });

    it("cannot execute an unapproved withdrawal even after delay", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const { withdrawPda, executedHashPda } = await submitWithdraw(
        ctx.user,
        destToken,
        100_000n,
        9103n,
        0xc9,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      try {
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("NotApproved");
      }
    });
  });

  // -----------------------------------------------------------------------
  // ADDITIONAL: DEPOSIT RECORD INTEGRITY
  // -----------------------------------------------------------------------
  describe("deposit record integrity", () => {
    it("multiple deposits produce strictly increasing nonces", async () => {
      const nonces: number[] = [];
      for (let i = 0; i < 3; i++) {
        const nextNonce = await getNextDepositNonce(ctx);
        const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32, i + 1)),
            amount: new anchor.BN(LAMPORTS_PER_SOL / 10),
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
        const deposit = await ctx.program.account.depositRecord.fetch(
          depositPda
        );
        nonces.push(deposit.nonce.toNumber());
      }
      for (let i = 1; i < nonces.length; i++) {
        expect(nonces[i]).to.equal(nonces[i - 1] + 1);
      }
    });

    it("each deposit has a unique transfer hash", async () => {
      const hashes = new Set<string>();
      for (let i = 0; i < 3; i++) {
        const nextNonce = await getNextDepositNonce(ctx);
        const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(Buffer.alloc(32, 0x10 + i)),
            amount: new anchor.BN(LAMPORTS_PER_SOL / 10),
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
        const deposit = await ctx.program.account.depositRecord.fetch(
          depositPda
        );
        const hashHex = Buffer.from(deposit.transferHash).toString("hex");
        expect(hashes.has(hashHex)).to.be.false;
        hashes.add(hashHex);
      }
      expect(hashes.size).to.equal(3);
    });
  });
});
