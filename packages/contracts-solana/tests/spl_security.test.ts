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
  findDepositPda,
  findExecutedHashPda,
  findTokenPda,
  findWithdrawPda,
  findWithdrawRateLimitPda,
  findNonceUsedPda,
  getNextDepositNonce,
  initializeBridgeIfNeeded,
  registerChainIfNeeded,
  setExplicitUnlimitedWithdrawRateLimit,
  setupTest,
  NATIVE_SOL_TOKEN,
} from "./helpers/setup";
import { computeTransferHash } from "./helpers/hash";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];
const WITHDRAW_DELAY_SECONDS = 15;

const EVM_REMOTE_NATIVE_TOKEN = Buffer.alloc(32);
EVM_REMOTE_NATIVE_TOKEN[31] = 0x37;

function toBn(value: bigint | number): anchor.BN {
  return new anchor.BN(value.toString());
}

function feeFor(amount: bigint, feeBps = 50n): bigint {
  return (amount * feeBps) / 10000n;
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

describe("bridge SPL security and multi-user coverage", () => {
  let ctx: TestContext;
  let evmChainPda: PublicKey;

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
      "evm_security"
    );

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
          // Match SPL test amounts (9-decimal minimal units); avoids execute-time normalize 18→9 truncating to 0.
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

    const initialSupply = 3_000_000_000n;
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
    mint: PublicKey,
    amount: bigint,
    nonce: bigint,
    srcAccountByte: number,
    remoteDestToken: Buffer,
    tokenPda: PublicKey
  ) {
    const srcAccount = Buffer.alloc(32, srcAccountByte);
    const transferHash = computeTransferHash(
      EVM_CHAIN_ID,
      SOLANA_CHAIN_ID,
      srcAccount,
      recipient.publicKey.toBuffer(),
      mint.toBuffer(),
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
        destToken: mint,
        amount: toBn(amount),
        nonce: toBn(nonce),
        operatorGas: new anchor.BN(0),
      })
      .accounts({
        bridge: ctx.bridgePda,
        srcChainEntry: evmChainPda,
        tokenMapping: tokenPda,
        pendingWithdraw: withdrawPda,
        executedHashCheck: executedHashPda,
        recipient: recipient.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([recipient])
      .rpc();

    return { transferHash, withdrawPda, executedHashPda };
  }

  describe("SPL fee isolation and e2e", () => {
    it("keeps lock/unlock SPL fees separate from escrow and executes full SPL flow", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0x21);
      const depositAmount = 1_000_000_000n;
      const expectedFee = feeFor(depositAmount);
      const expectedNet = depositAmount - expectedFee;
      const destAccount = Buffer.alloc(32, 0xab);

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
      const mapping = await ctx.program.account.tokenMapping.fetch(
        fixture.tokenPda
      );
      const bridgeTokenBalance = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );

      const expectedHash = computeTransferHash(
        SOLANA_CHAIN_ID,
        EVM_CHAIN_ID,
        ctx.user.publicKey.toBuffer(),
        destAccount,
        fixture.destToken,
        expectedNet,
        BigInt(nextNonce)
      );

      expect(Buffer.from(deposit.transferHash).toString("hex")).to.equal(
        expectedHash.toString("hex")
      );
      expect(Number(deposit.amount)).to.equal(Number(expectedNet));
      expect(Number(mapping.accruedFees)).to.equal(Number(expectedFee));
      expect(Number(bridgeTokenBalance.amount)).to.equal(Number(depositAmount));

      try {
        await ctx.program.methods
          .withdrawFees({
            amount: toBn(expectedFee + 1n),
            native: false,
          })
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InsufficientAccruedFees");
      }

      await ctx.program.methods
        .withdrawFees({
          amount: toBn(expectedFee),
          native: false,
        })
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

      const mappingAfterFeeWithdraw =
        await ctx.program.account.tokenMapping.fetch(fixture.tokenPda);
      const bridgeAfterFeeWithdraw = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      const adminAfterFeeWithdraw = await getAccount(
        ctx.provider.connection,
        fixture.adminToken.address
      );

      expect(Number(mappingAfterFeeWithdraw.accruedFees)).to.equal(0);
      expect(Number(bridgeAfterFeeWithdraw.amount)).to.equal(
        Number(expectedNet)
      );
      expect(Number(adminAfterFeeWithdraw.amount)).to.equal(
        Number(expectedFee)
      );

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          expectedNet,
          5001n,
          0xcd,
          fixture.destToken,
          fixture.tokenPda
        );

      const [nonceUsedPda] = findNonceUsedPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        5001n
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
          withdrawRateLimit: findWithdrawRateLimitPda(
            ctx.program.programId,
            fixture.mint
          )[0],
          recipient: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const bridgeAfterExecute = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      expect(Number(bridgeAfterExecute.amount)).to.equal(0);
    });

    it("handles mint/burn SPL deposits and remints exact net amount on execution", async () => {
      const fixture = await createSplFixture({ mintBurn: {} }, 0x31);
      const depositAmount = 1_000_000_000n;
      const expectedFee = feeFor(depositAmount);
      const expectedNet = depositAmount - expectedFee;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xee)),
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

      const bridgeTokenBalance = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      const mintAfterDeposit = await getMint(
        ctx.provider.connection,
        fixture.mint
      );
      expect(Number(bridgeTokenBalance.amount)).to.equal(Number(expectedFee));
      expect(Number(mintAfterDeposit.supply)).to.equal(
        Number(fixture.initialSupply - expectedNet)
      );

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          expectedNet,
          6001n,
          0xef,
          fixture.destToken,
          fixture.tokenPda
        );

      const [nonceUsedMb] = findNonceUsedPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        6001n
      );

      await ctx.program.methods
        .withdrawApprove({ transferHash: Array.from(transferHash) })
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          nonceUsed: nonceUsedMb,
          operator: ctx.operator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.operator])
        .rpc();

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
          withdrawRateLimit: findWithdrawRateLimitPda(
            ctx.program.programId,
            fixture.mint
          )[0],
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
      expect(Number(mintAfterExecute.supply)).to.equal(
        Number(fixture.initialSupply)
      );

      await ctx.program.methods
        .withdrawFees({
          amount: toBn(expectedFee),
          native: false,
        })
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

      const bridgeAfterFeeWithdraw = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      expect(Number(bridgeAfterFeeWithdraw.amount)).to.equal(0);
    });
  });

  describe("bad-path execution hardening", () => {
    it("rejects not-approved, early, wrong-recipient, wrong-mint, and cancelled SPL executions", async () => {
      const fixture = await createSplFixture({ mintBurn: {} }, 0x41);
      const wrongMintFixture = await createSplFixture({ mintBurn: {} }, 0x42);
      const rogueRecipient = Keypair.generate();
      await airdrop(
        ctx.provider.connection,
        rogueRecipient.publicKey,
        LAMPORTS_PER_SOL
      );
      const rogueToken = await getOrCreateAssociatedTokenAccount(
        ctx.provider.connection,
        ctx.admin,
        fixture.mint,
        rogueRecipient.publicKey
      );

      const amount = 750_000_000n;
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          amount,
          7001n,
          0x91,
          fixture.destToken,
          fixture.tokenPda
        );

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
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              fixture.mint
            )[0],
            recipient: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("NotApproved");
      }

      const [nonceUsed7001] = findNonceUsedPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        7001n
      );

      await ctx.program.methods
        .withdrawApprove({ transferHash: Array.from(transferHash) })
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          nonceUsed: nonceUsed7001,
          operator: ctx.operator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.operator])
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
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              fixture.mint
            )[0],
            recipient: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("DelayNotElapsed");
      }

      try {
        await ctx.program.methods
          .withdrawExecute()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            mint: fixture.mint,
            recipientTokenAccount: rogueToken.address,
            bridgeTokenAccount: fixture.bridgeToken.address,
            tokenMapping: fixture.tokenPda,
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              fixture.mint
            )[0],
            recipient: rogueRecipient.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([rogueRecipient])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("WrongRecipient");
      }

      try {
        await ctx.program.methods
          .withdrawExecute()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            mint: wrongMintFixture.mint,
            recipientTokenAccount: wrongMintFixture.userToken.address,
            bridgeTokenAccount: wrongMintFixture.bridgeToken.address,
            tokenMapping: wrongMintFixture.tokenPda,
            recipient: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("TokenMintMismatch") ||
            msg.includes("TokenNotRegistered")
        ).to.be.true;
      }

      const [cancelerPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("canceler"), ctx.canceler.publicKey.toBuffer()],
        ctx.program.programId
      );
      const cancelerInfo = await ctx.provider.connection.getAccountInfo(
        cancelerPda
      );
      if (!cancelerInfo) {
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
          .withdrawExecute()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            mint: fixture.mint,
            recipientTokenAccount: fixture.userToken.address,
            bridgeTokenAccount: fixture.bridgeToken.address,
            tokenMapping: fixture.tokenPda,
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              fixture.mint
            )[0],
            recipient: ctx.user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("WithdrawalCancelled");
      }
    });
  });

  describe("native fee safety and multi-user isolation", () => {
    it("tracks native fees across multiple users without letting admin drain principal", async () => {
      const secondUser = Keypair.generate();
      await airdrop(
        ctx.provider.connection,
        secondUser.publicKey,
        3 * LAMPORTS_PER_SOL
      );

      const destTokB1 = Buffer.alloc(32);
      destTokB1[31] = 0xb1;
      const destTokB2 = Buffer.alloc(32);
      destTokB2[31] = 0xb2;

      for (const dt of [destTokB1, destTokB2]) {
        const [tmap] = findTokenPda(
          ctx.program.programId,
          Buffer.from(EVM_CHAIN_ID),
          dt
        );
        const ex = await ctx.provider.connection.getAccountInfo(tmap);
        if (!ex) {
          await ctx.program.methods
            .registerToken({
              localMint: PublicKey.default,
              destChain: EVM_CHAIN_ID,
              destToken: Array.from(dt),
              mode: { lockUnlock: {} },
              decimals: 9,
              srcDecimals: 18,
            })
            .accounts({
              bridge: ctx.bridgePda,
              tokenMapping: tmap,
              mint: null,
              admin: ctx.admin.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .rpc();
        }
      }

      const [mapB1] = findTokenPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        destTokB1
      );
      const [mapB2] = findTokenPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        destTokB2
      );

      const amountA = 1_000_000_000n;
      const amountB = 2_000_000_000n;
      const feeA = feeFor(amountA);
      const feeB = feeFor(amountB);
      const totalFee = feeA + feeB;

      const nonceA = await getNextDepositNonce(ctx);
      const [depositPdaA] = findDepositPda(ctx.program.programId, nonceA);
      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xa1)),
          amount: toBn(amountA),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPdaA,
          destChainEntry: evmChainPda,
          tokenMapping: mapB1,
          depositor: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const nonceB = await getNextDepositNonce(ctx);
      const [depositPdaB] = findDepositPda(ctx.program.programId, nonceB);
      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(Buffer.alloc(32, 0xa2)),
          amount: toBn(amountB),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPdaB,
          destChainEntry: evmChainPda,
          tokenMapping: mapB2,
          depositor: secondUser.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([secondUser])
        .rpc();

      const depositA = await ctx.program.account.depositRecord.fetch(
        depositPdaA
      );
      const depositB = await ctx.program.account.depositRecord.fetch(
        depositPdaB
      );
      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );

      expect(depositA.nonce.toNumber()).to.equal(nonceA);
      expect(depositB.nonce.toNumber()).to.equal(nonceB);
      expect(Buffer.from(depositA.transferHash).toString("hex")).to.not.equal(
        Buffer.from(depositB.transferHash).toString("hex")
      );
      expect(Number(bridge.accruedNativeFees)).to.be.at.least(Number(totalFee));

      try {
        await ctx.program.methods
          .withdrawFees({
            amount: new anchor.BN(Number(bridge.accruedNativeFees) + 1),
            native: true,
          })
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
  });

  describe("auth and pause hardening", () => {
    it("rejects withdraw_submit when bridge is paused", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const srcAccount = Buffer.alloc(32, 0x77);
      const amount = 100_000n;
      const nonce = 8888n;

      const [withdrawNativeMap] = findTokenPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        EVM_REMOTE_NATIVE_TOKEN
      );
      const wInfo = await ctx.provider.connection.getAccountInfo(
        withdrawNativeMap
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
            tokenMapping: withdrawNativeMap,
            mint: null,
            admin: ctx.admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
      }

      const transferHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        amount,
        nonce
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
            amount: toBn(amount),
            nonce: toBn(nonce),
            operatorGas: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            srcChainEntry: evmChainPda,
            tokenMapping: withdrawNativeMap,
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

    it("rejects withdraw_fees from non-admin", async () => {
      const rogue = Keypair.generate();
      await airdrop(ctx.provider.connection, rogue.publicKey, LAMPORTS_PER_SOL);

      try {
        await ctx.program.methods
          .withdrawFees({
            amount: new anchor.BN(1),
            native: true,
          })
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
  });
});
