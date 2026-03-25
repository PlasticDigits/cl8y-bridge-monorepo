import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  AuthorityType,
  TOKEN_PROGRAM_ID,
  createMint,
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
  findExecutedHashPda,
  findTokenPda,
  findWithdrawPda,
  initializeBridgeIfNeeded,
  registerChainIfNeeded,
  setupTest,
  NATIVE_SOL_TOKEN,
  findNonceUsedPda,
} from "./helpers/setup";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];
const WITHDRAW_DELAY_SECONDS = 15;

const EVM_REMOTE_NATIVE_TOKEN = Buffer.alloc(32);
EVM_REMOTE_NATIVE_TOKEN[31] = 0x37;
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

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

describe("hardening tests", () => {
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

    evmChainPda = await registerChainIfNeeded(ctx, EVM_CHAIN_ID, "evm_hard");

    [depositTokenMappingPda] = findTokenPda(
      ctx.program.programId,
      Buffer.from(EVM_CHAIN_ID),
      DEPOSIT_DEST_TOKEN
    );
    if (!(await ctx.provider.connection.getAccountInfo(depositTokenMappingPda))) {
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
    if (
      !(await ctx.provider.connection.getAccountInfo(withdrawNativeTokenMappingPda))
    ) {
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
    await ctx.program.methods
      .addCanceler({ canceler: ctx.canceler.publicKey, active: true })
      .accounts({
        bridge: ctx.bridgePda,
        cancelerEntry: cancelerPda,
        admin: ctx.admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    return cancelerPda;
  }

  // -----------------------------------------------------------------------
  // 1. Admin rotation boundary tests
  // -----------------------------------------------------------------------
  describe("admin rotation", () => {
    it("set_config({ newAdmin }) transfers admin, old admin is rejected, new admin can act", async () => {
      const newAdmin = Keypair.generate();
      await airdrop(ctx.provider.connection, newAdmin.publicKey);

      // Transfer admin
      await ctx.program.methods
        .setConfig({
          newAdmin: newAdmin.publicKey,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
        })
        .rpc();

      // Old admin cannot set_config
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
            admin: ctx.admin.publicKey,
          })
          .rpc();
        expect.fail("Old admin should be rejected");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }

      // New admin can set_config
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: 60,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: newAdmin.publicKey,
        })
        .signers([newAdmin])
        .rpc();

      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      expect(bridge.feeBps).to.equal(60);

      // Restore admin back for subsequent tests
      await ctx.program.methods
        .setConfig({
          newAdmin: ctx.admin.publicKey,
          operator: null,
          feeBps: 50,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: newAdmin.publicKey,
        })
        .signers([newAdmin])
        .rpc();
    });
  });

  // -----------------------------------------------------------------------
  // 2. Decimals validation tests
  // -----------------------------------------------------------------------
  describe("decimals validation", () => {
    it("register_token with decimals != mint.decimals is rejected (InvalidDecimals)", async () => {
      const mint = await createMint(
        ctx.provider.connection,
        ctx.admin,
        ctx.admin.publicKey,
        null,
        6 // actual decimals = 6
      );
      const destToken = Buffer.alloc(32);
      destToken[31] = 0x50;
      const [tokenPda] = findTokenPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        destToken
      );

      try {
        await ctx.program.methods
          .registerToken({
            localMint: mint,
            destChain: EVM_CHAIN_ID,
            destToken: Array.from(destToken),
            mode: { lockUnlock: {} },
            decimals: 9, // mismatched
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InvalidDecimals");
      }
    });

    it("register_token with correct decimals succeeds", async () => {
      const mint = await createMint(
        ctx.provider.connection,
        ctx.admin,
        ctx.admin.publicKey,
        null,
        6
      );
      const destToken = Buffer.alloc(32);
      destToken[31] = 0x51;
      const [tokenPda] = findTokenPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        destToken
      );

      await ctx.program.methods
        .registerToken({
          localMint: mint,
          destChain: EVM_CHAIN_ID,
          destToken: Array.from(destToken),
          mode: { lockUnlock: {} },
          decimals: 6,
          srcDecimals: 6,
        })
        .accounts({
          bridge: ctx.bridgePda,
          tokenMapping: tokenPda,
          mint,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const mapping = await ctx.program.account.tokenMapping.fetch(tokenPda);
      expect(mapping.decimals).to.equal(6);
    });
  });

  // -----------------------------------------------------------------------
  // 3. Zero chain_id registration rejected
  // -----------------------------------------------------------------------
  describe("zero chain_id rejection", () => {
    it("register_chain with chain_id [0,0,0,0] is rejected (InvalidChainId)", async () => {
      const zeroChain = [0, 0, 0, 0];
      const [chainPda] = findChainPda(
        ctx.program.programId,
        Buffer.from(zeroChain)
      );

      try {
        await ctx.program.methods
          .registerChain({ chainId: zeroChain, identifier: "zero" })
          .accounts({
            bridge: ctx.bridgePda,
            chainEntry: chainPda,
            admin: ctx.admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InvalidChainId");
      }
    });
  });

  // -----------------------------------------------------------------------
  // 4. MintBurn authority validation
  // -----------------------------------------------------------------------
  describe("MintBurn authority validation", () => {
    it("register_token in MintBurn mode where mint authority is NOT bridge PDA is rejected", async () => {
      const mint = await createMint(
        ctx.provider.connection,
        ctx.admin,
        ctx.admin.publicKey, // admin is mint authority, NOT bridge PDA
        null,
        9
      );
      const destToken = Buffer.alloc(32);
      destToken[31] = 0x52;
      const [tokenPda] = findTokenPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        destToken
      );

      try {
        await ctx.program.methods
          .registerToken({
            localMint: mint,
            destChain: EVM_CHAIN_ID,
            destToken: Array.from(destToken),
            mode: { mintBurn: {} },
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
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("MintAuthorityNotBridge");
      }
    });

    it("register_token in MintBurn mode where mint authority IS bridge PDA succeeds", async () => {
      const mint = await createMint(
        ctx.provider.connection,
        ctx.admin,
        ctx.admin.publicKey,
        null,
        9
      );
      // Transfer mint authority to bridge PDA
      await setAuthority(
        ctx.provider.connection,
        ctx.admin,
        mint,
        ctx.admin,
        AuthorityType.MintTokens,
        ctx.bridgePda
      );

      const destToken = Buffer.alloc(32);
      destToken[31] = 0x53;
      const [tokenPda] = findTokenPda(
        ctx.program.programId,
        Buffer.from(EVM_CHAIN_ID),
        destToken
      );

      await ctx.program.methods
        .registerToken({
          localMint: mint,
          destChain: EVM_CHAIN_ID,
          destToken: Array.from(destToken),
          mode: { mintBurn: {} },
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

      const mapping = await ctx.program.account.tokenMapping.fetch(tokenPda);
      expect(mapping.mode).to.deep.equal({ mintBurn: {} });
    });
  });

  // -----------------------------------------------------------------------
  // 5. Zero-amount withdraw_submit rejected
  // -----------------------------------------------------------------------
  describe("zero-amount withdraw_submit", () => {
    it("withdraw_submit with amount: 0 is rejected (ZeroAmount)", async () => {
      const srcAccount = Buffer.alloc(32, 0xaa);
      const destToken = NATIVE_SOL_TOKEN;
      const transferHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        destToken.toBuffer(),
        0n,
        9990n
      );
      const [withdrawPda] = findWithdrawPda(
        ctx.program.programId,
        transferHash
      );
      const [executedHashPda] = findExecutedHashPda(
        ctx.program.programId,
        transferHash
      );

      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: EVM_CHAIN_ID,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken,
            amount: toBn(0),
            nonce: toBn(9990),
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
        expect(err.toString()).to.contain("ZeroAmount");
      }
    });
  });

  // -----------------------------------------------------------------------
  // 6. Withdraw reenable while paused rejected
  // -----------------------------------------------------------------------
  describe("withdraw reenable while paused", () => {
    it("withdraw_reenable while bridge is paused is rejected (BridgePaused)", async () => {
      const cancelerPda = await ensureCanceler();
      const destToken = NATIVE_SOL_TOKEN;

      // Submit + approve + cancel a withdrawal
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        50_000n,
        8001n,
        0xf1,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, 8001n);
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

      // Pause the bridge
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: true,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
        })
        .rpc();

      // Try reenable while paused
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
        expect(err.toString()).to.contain("BridgePaused");
      }

      // Unpause
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: false,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
        })
        .rpc();
    });
  });

  // -----------------------------------------------------------------------
  // 7. Double execute after success
  // -----------------------------------------------------------------------
  describe("double execute after success", () => {
    it("calling withdraw_execute_native again after success fails", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const payoutLamports = 10_000n;
      const rawAmount = payoutLamports * 1_000_000_000n;
      const nonce = 8002n;

      // Fund the bridge PDA
      const tx = new anchor.web3.Transaction().add(
        SystemProgram.transfer({
          fromPubkey: ctx.admin.publicKey,
          toPubkey: ctx.bridgePda,
          lamports: LAMPORTS_PER_SOL,
        })
      );
      await ctx.provider.sendAndConfirm(tx);

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          destToken,
          rawAmount,
          nonce,
          0xf2,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );

      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      // Execute successfully
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

      // Try executing again - PDA is already closed
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
        expect.fail("Should have thrown");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("AccountNotInitialized") ||
            msg.includes("account not found") ||
            msg.includes("already in use") ||
            msg.includes("Error processing Instruction")
        ).to.be.true;
      }
    });
  });

  // -----------------------------------------------------------------------
  // 8. Wrong executed_hash PDA on execute
  // -----------------------------------------------------------------------
  describe("wrong executed_hash PDA", () => {
    it("passing executed_hash PDA derived from a different hash fails", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      const rawAmount = 10_000n * 1_000_000_000n;
      const nonce = 8003n;

      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        destToken,
        rawAmount,
        nonce,
        0xf3,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      // Derive executed_hash from a DIFFERENT hash
      const wrongHash = Buffer.alloc(32, 0xff);
      const [wrongExecutedPda] = findExecutedHashPda(
        ctx.program.programId,
        wrongHash
      );

      try {
        await ctx.program.methods
          .withdrawExecuteNative()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: wrongExecutedPda,
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("ConstraintSeeds") ||
            msg.includes("seeds constraint") ||
            msg.includes("Error processing Instruction")
        ).to.be.true;
      }
    });
  });

  // -----------------------------------------------------------------------
  // 9. Fee rounding / dust tests
  // -----------------------------------------------------------------------
  describe("fee rounding / dust", () => {
    async function depositAndCheck(
      amount: number,
      expectedNet: number,
      expectedFeeDelta: number,
      label: string
    ) {
      const bridgeBefore = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const feesBefore = bridgeBefore.accruedNativeFees.toNumber();
      const nonce = bridgeBefore.depositNonce.toNumber() + 1;
      const [depositPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("deposit"),
          Buffer.from(new anchor.BN(nonce).toArray("le", 8)),
        ],
        ctx.program.programId
      );

      const destAccount = Array.from(
        Buffer.alloc(32, 0x60 + (nonce & 0xff))
      );

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount,
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

      const deposit = await ctx.program.account.depositRecord.fetch(
        depositPda
      );
      expect(Number(deposit.amount)).to.equal(expectedNet);

      const bridgeAfter = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const feesAfter = bridgeAfter.accruedNativeFees.toNumber();
      expect(feesAfter - feesBefore).to.equal(expectedFeeDelta);
    }

    it("deposit 1 lamport with fee_bps=50: fee=0, net=1", async () => {
      await depositAndCheck(1, 1, 0, "1 lamport");
    });

    it("deposit 199 lamports with fee_bps=50: fee=0, net=199", async () => {
      await depositAndCheck(199, 199, 0, "199 lamports");
    });

    it("deposit 200 lamports with fee_bps=50: fee=1, net=199", async () => {
      await depositAndCheck(200, 199, 1, "200 lamports");
    });
  });

  // -----------------------------------------------------------------------
  // 10. Hash with amount > u64::MAX
  // -----------------------------------------------------------------------
  describe("u128 amount handling", () => {
    it("transfer hash with u128 amount > u64::MAX produces valid unique hash", () => {
      const largeAmount = (1n << 65n) + 12345n;
      const hash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        Buffer.alloc(32, 0x11),
        Buffer.alloc(32, 0x22),
        Buffer.alloc(32, 0x33),
        largeAmount,
        1n
      );
      expect(hash.length).to.equal(32);

      // Different amount produces different hash
      const hash2 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        Buffer.alloc(32, 0x11),
        Buffer.alloc(32, 0x22),
        Buffer.alloc(32, 0x33),
        largeAmount + 1n,
        1n
      );
      expect(hash.equals(hash2)).to.be.false;
    });

    it("withdraw_execute_native with amount > u64::MAX fails (AmountExceedsU64)", async () => {
      const destToken = NATIVE_SOL_TOKEN;
      // After 18→9 normalize, still must not fit u64 (native mapping uses src 18 / dest 9).
      const largeAmount = (BigInt(1) << 64n) * BigInt(1_000_000_000);
      const nonce = 8004n;

      // Fund bridge
      const fundTx = new anchor.web3.Transaction().add(
        SystemProgram.transfer({
          fromPubkey: ctx.admin.publicKey,
          toPubkey: ctx.bridgePda,
          lamports: LAMPORTS_PER_SOL,
        })
      );
      await ctx.provider.sendAndConfirm(fundTx);

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          destToken,
          largeAmount,
          nonce,
          0xf4,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

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
        expect.fail("Should have thrown");
      } catch (err) {
        const s = err.toString();
        expect(
          s.includes("AmountExceedsU64") ||
            s.includes("ArithmeticOverflow") ||
            s.includes("6008")
        ).to.be.true;
      }
    });
  });

  // -----------------------------------------------------------------------
  // 11. InvalidWithdrawDelay boundary values
  // -----------------------------------------------------------------------
  describe("withdraw delay boundary", () => {
    it("withdraw delay of 14 is rejected (InvalidWithdrawDelay)", async () => {
      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: null,
            feeBps: null,
            withdrawDelay: new anchor.BN(14),
            paused: null,
          })
          .accounts({
            bridge: ctx.bridgePda,
            admin: ctx.admin.publicKey,
          })
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InvalidWithdrawDelay");
      }
    });

    it("withdraw delay of 86401 is rejected (InvalidWithdrawDelay)", async () => {
      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: null,
            feeBps: null,
            withdrawDelay: new anchor.BN(86401),
            paused: null,
          })
          .accounts({
            bridge: ctx.bridgePda,
            admin: ctx.admin.publicKey,
          })
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InvalidWithdrawDelay");
      }
    });

    it("withdraw delay of 15 is accepted", async () => {
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: new anchor.BN(15),
          paused: null,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
        })
        .rpc();

      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      expect(bridge.withdrawDelay.toNumber()).to.equal(15);
    });

    it("withdraw delay of 86400 is accepted", async () => {
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: new anchor.BN(86400),
          paused: null,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
        })
        .rpc();

      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      expect(bridge.withdrawDelay.toNumber()).to.equal(86400);

      // Restore to 15 for other tests
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: null,
          withdrawDelay: new anchor.BN(WITHDRAW_DELAY_SECONDS),
          paused: null,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
        })
        .rpc();
    });
  });
});
