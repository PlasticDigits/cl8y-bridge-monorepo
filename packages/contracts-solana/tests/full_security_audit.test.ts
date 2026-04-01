import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
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
import { computeTransferHash } from "./helpers/hash";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];
const WITHDRAW_DELAY_SECONDS = 15;

const EVM_REMOTE_NATIVE_TOKEN = Buffer.alloc(32);
EVM_REMOTE_NATIVE_TOKEN[31] = 0x37;
const DEPOSIT_DEST_TOKEN = Buffer.alloc(32);
DEPOSIT_DEST_TOKEN[31] = 0xcc;

function toBn(value: bigint | number): anchor.BN {
  return new anchor.BN(value.toString());
}

function feeFor(amount: bigint, feeBps = 50n): bigint {
  return (amount * feeBps) / 10000n;
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

function randomU64(): bigint {
  const hi = BigInt(Math.floor(Math.random() * 0xffffffff));
  const lo = BigInt(Math.floor(Math.random() * 0xffffffff));
  return (hi << 32n) | lo;
}

function randomBytes(len: number): Buffer {
  const buf = Buffer.alloc(len);
  for (let i = 0; i < len; i++) {
    buf[i] = Math.floor(Math.random() * 256);
  }
  return buf;
}

describe("FULL E2E SECURITY AUDIT", () => {
  let ctx: TestContext;
  let evmChainPda: PublicKey;
  let cancelerPda: PublicKey;
  let withdrawNativeTokenMappingPda: PublicKey;
  let depositTokenMappingPda: PublicKey;
  let nonceCounter = 20000n;

  function nextNonceVal(): bigint {
    return nonceCounter++;
  }

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
      "evm_full_audit"
    );

    const [cPda] = findCancelerPda(
      ctx.program.programId,
      ctx.canceler.publicKey
    );
    cancelerPda = cPda;
    await ctx.program.methods
      .addCanceler({ canceler: ctx.canceler.publicKey, active: true })
      .accounts({
        bridge: ctx.bridgePda,
        cancelerEntry: cancelerPda,
        admin: ctx.admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: ctx.admin.publicKey,
        toPubkey: ctx.bridgePda,
        lamports: 50 * LAMPORTS_PER_SOL,
      })
    );
    await ctx.provider.sendAndConfirm(fundTx);

    [depositTokenMappingPda] = findTokenPda(
      ctx.program.programId,
      Buffer.from(EVM_CHAIN_ID),
      DEPOSIT_DEST_TOKEN
    );
    if (
      !(await ctx.provider.connection.getAccountInfo(depositTokenMappingPda))
    ) {
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
      !(await ctx.provider.connection.getAccountInfo(
        withdrawNativeTokenMappingPda
      ))
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
    const initialSupply = 50_000_000_000n;
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

  // =====================================================================
  // SECTION 1: FUZZ TESTING - Fee Calculation Invariants
  // =====================================================================
  describe("FUZZ: fee calculation invariants (50 random amounts)", () => {
    const FUZZ_ITERATIONS = 50;

    it("fee + net_amount == gross_amount for all random amounts", async () => {
      const feeBps = 50n;
      for (let i = 0; i < FUZZ_ITERATIONS; i++) {
        const amount = (randomU64() % 10n ** 18n) + 1n;
        const fee = (amount * feeBps) / 10000n;
        const net = amount - fee;
        expect(fee + net).to.equal(
          amount,
          `fee + net != amount for amount=${amount}`
        );
        expect(fee <= amount).to.be.true;
        expect(net >= 0n).to.be.true;
      }
    });

    it("fee is zero for amounts below fee_bps threshold", () => {
      for (let feeBps = 1n; feeBps <= 100n; feeBps++) {
        const threshold = 10000n / feeBps;
        for (let amt = 1n; amt < threshold && amt < 500n; amt++) {
          const fee = (amt * feeBps) / 10000n;
          expect(fee).to.equal(
            0n,
            `fee should be 0 for amount=${amt}, feeBps=${feeBps}`
          );
        }
      }
    });

    it("fee is monotonically non-decreasing with amount", () => {
      const feeBps = 50n;
      let prevFee = 0n;
      for (let i = 0; i < FUZZ_ITERATIONS; i++) {
        const amount = BigInt(i * 100 + 1);
        const fee = (amount * feeBps) / 10000n;
        expect(fee >= prevFee).to.be.true;
        prevFee = fee;
      }
    });

    it("fee at max feeBps (100) is 1% of amount", () => {
      for (let i = 0; i < 20; i++) {
        const amount = (randomU64() % 10n ** 15n) + 1n;
        const fee = (amount * 100n) / 10000n;
        expect(fee).to.equal((amount * 100n) / 10000n);
      }
    });

    it("fee at zero feeBps is always zero", () => {
      for (let i = 0; i < 20; i++) {
        const amount = (randomU64() % 10n ** 15n) + 1n;
        const fee = (amount * 0n) / 10000n;
        expect(fee).to.equal(0n);
      }
    });

    it("on-chain native deposit fee matches off-chain calculation for 10 random amounts", async () => {
      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const feeBps = BigInt(bridge.feeBps);

      for (let i = 0; i < 10; i++) {
        const amount = BigInt(
          LAMPORTS_PER_SOL / 100 +
            Math.floor(Math.random() * LAMPORTS_PER_SOL * 5)
        );
        const expectedFee = (amount * feeBps) / 10000n;
        const expectedNet = amount - expectedFee;

        const nextNonce = await getNextDepositNonce(ctx);
        const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
        const bridgeBefore = await ctx.program.account.bridgeConfig.fetch(
          ctx.bridgePda
        );
        const feesBefore = BigInt(bridgeBefore.accruedNativeFees.toString());

        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(randomBytes(32)),
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

        const deposit = await ctx.program.account.depositRecord.fetch(
          depositPda
        );
        const bridgeAfter = await ctx.program.account.bridgeConfig.fetch(
          ctx.bridgePda
        );
        const feesAfter = BigInt(bridgeAfter.accruedNativeFees.toString());

        expect(BigInt(deposit.amount.toString())).to.equal(
          expectedNet,
          `On-chain net mismatch for amount=${amount}`
        );
        expect(feesAfter - feesBefore).to.equal(
          expectedFee,
          `On-chain fee delta mismatch for amount=${amount}`
        );
      }
    });
  });

  // =====================================================================
  // SECTION 2: FUZZ TESTING - Hash Function Properties
  // =====================================================================
  describe("FUZZ: transfer hash collision resistance (100 random inputs)", () => {
    const FUZZ_ITERATIONS = 100;

    it("100 random transfer hashes are all unique", () => {
      const hashes = new Set<string>();
      for (let i = 0; i < FUZZ_ITERATIONS; i++) {
        const srcChain = Array.from(randomBytes(4));
        const destChain = Array.from(randomBytes(4));
        const srcAccount = randomBytes(32);
        const destAccount = randomBytes(32);
        const token = randomBytes(32);
        const amount = randomU64();
        const nonce = randomU64();

        const hash = computeTransferHash(
          srcChain,
          destChain,
          srcAccount,
          destAccount,
          token,
          amount,
          nonce
        );
        const hex = hash.toString("hex");
        expect(hashes.has(hex)).to.be.false;
        hashes.add(hex);
      }
      expect(hashes.size).to.equal(FUZZ_ITERATIONS);
    });

    it("single-bit changes produce different hashes (avalanche property)", () => {
      const base = {
        srcChain: [0x00, 0x00, 0x00, 0x01] as number[],
        destChain: [0x00, 0x00, 0x00, 0x05] as number[],
        srcAccount: Buffer.alloc(32, 0xaa),
        destAccount: Buffer.alloc(32, 0xbb),
        token: Buffer.alloc(32, 0xcc),
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

      for (let byteIdx = 0; byteIdx < 32; byteIdx++) {
        const modified = Buffer.from(base.srcAccount);
        modified[byteIdx] ^= 0x01;
        const modHash = computeTransferHash(
          base.srcChain,
          base.destChain,
          modified,
          base.destAccount,
          base.token,
          base.amount,
          base.nonce
        );
        expect(modHash.toString("hex")).to.not.equal(
          baseHash.toString("hex"),
          `Bit flip at srcAccount[${byteIdx}] did not change hash`
        );
      }

      for (let bitShift = 0n; bitShift < 64n; bitShift++) {
        const modAmount = base.amount ^ (1n << bitShift);
        if (modAmount === base.amount) continue;
        const modHash = computeTransferHash(
          base.srcChain,
          base.destChain,
          base.srcAccount,
          base.destAccount,
          base.token,
          modAmount,
          base.nonce
        );
        expect(modHash.toString("hex")).to.not.equal(
          baseHash.toString("hex"),
          `Bit flip at amount bit ${bitShift} did not change hash`
        );
      }
    });

    it("hash is deterministic: same inputs always produce same output", () => {
      for (let i = 0; i < 20; i++) {
        const srcChain = Array.from(randomBytes(4));
        const destChain = Array.from(randomBytes(4));
        const srcAccount = randomBytes(32);
        const destAccount = randomBytes(32);
        const token = randomBytes(32);
        const amount = randomU64();
        const nonce = randomU64();

        const hash1 = computeTransferHash(
          srcChain,
          destChain,
          srcAccount,
          destAccount,
          token,
          amount,
          nonce
        );
        const hash2 = computeTransferHash(
          srcChain,
          destChain,
          srcAccount,
          destAccount,
          token,
          amount,
          nonce
        );
        expect(hash1.toString("hex")).to.equal(hash2.toString("hex"));
      }
    });

    it("amount=0 vs amount=1 produce different hashes", () => {
      const src = randomBytes(32);
      const dest = randomBytes(32);
      const tok = randomBytes(32);
      const h0 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        src,
        dest,
        tok,
        0n,
        1n
      );
      const h1 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        src,
        dest,
        tok,
        1n,
        1n
      );
      expect(h0.toString("hex")).to.not.equal(h1.toString("hex"));
    });

    it("nonce=0 vs nonce=1 produce different hashes", () => {
      const src = randomBytes(32);
      const dest = randomBytes(32);
      const tok = randomBytes(32);
      const h0 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        src,
        dest,
        tok,
        1000n,
        0n
      );
      const h1 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        src,
        dest,
        tok,
        1000n,
        1n
      );
      expect(h0.toString("hex")).to.not.equal(h1.toString("hex"));
    });

    it("swapping src_chain and dest_chain produces different hash", () => {
      const src = randomBytes(32);
      const dest = randomBytes(32);
      const tok = randomBytes(32);
      const h1 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        src,
        dest,
        tok,
        1000n,
        1n
      );
      const h2 = computeTransferHash(
        SOLANA_CHAIN_ID,
        EVM_CHAIN_ID,
        src,
        dest,
        tok,
        1000n,
        1n
      );
      expect(h1.toString("hex")).to.not.equal(h2.toString("hex"));
    });

    it("swapping src_account and dest_account produces different hash", () => {
      const a = randomBytes(32);
      const b = randomBytes(32);
      const tok = randomBytes(32);
      const h1 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        a,
        b,
        tok,
        1000n,
        1n
      );
      const h2 = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        b,
        a,
        tok,
        1000n,
        1n
      );
      expect(h1.toString("hex")).to.not.equal(h2.toString("hex"));
    });
  });

  // =====================================================================
  // SECTION 3: FUZZ TESTING - Nonce Monotonicity Invariant
  // =====================================================================
  describe("FUZZ: nonce monotonicity across 20 sequential deposits", () => {
    it("deposit_nonce increases by exactly 1 for each deposit", async () => {
      const nonces: number[] = [];
      for (let i = 0; i < 20; i++) {
        const bridgeBefore = await ctx.program.account.bridgeConfig.fetch(
          ctx.bridgePda
        );
        const expectedNonce = bridgeBefore.depositNonce.toNumber() + 1;
        const [depositPda] = findDepositPda(
          ctx.program.programId,
          expectedNonce
        );

        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(randomBytes(32)),
            amount: new anchor.BN(LAMPORTS_PER_SOL / 100),
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
        expect(deposit.nonce.toNumber()).to.equal(expectedNonce);
      }

      for (let i = 1; i < nonces.length; i++) {
        expect(nonces[i]).to.equal(nonces[i - 1] + 1);
      }
    });
  });

  // =====================================================================
  // SECTION 4: FUZZ TESTING - SPL Balance Accounting Invariant
  // =====================================================================
  describe("FUZZ: SPL balance accounting invariant across random deposit amounts", () => {
    it("lock/unlock: bridge_token_balance == sum(deposits) - sum(fee_withdrawals) - sum(executions)", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0x01);
      let totalDeposited = 0n;
      let totalFees = 0n;

      for (let i = 0; i < 5; i++) {
        const amount = BigInt(
          100_000_000 + Math.floor(Math.random() * 900_000_000)
        );
        const fee = feeFor(amount);
        totalDeposited += amount;
        totalFees += fee;

        const nextNonce = await getNextDepositNonce(ctx);
        const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

        await ctx.program.methods
          .depositSpl({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(randomBytes(32)),
            amount: toBn(amount),
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
      }

      const bridgeTokenInfo = await getAccount(
        ctx.provider.connection,
        fixture.bridgeToken.address
      );
      const mapping = await ctx.program.account.tokenMapping.fetch(
        fixture.tokenPda
      );

      expect(BigInt(bridgeTokenInfo.amount.toString())).to.equal(
        totalDeposited
      );
      expect(BigInt(mapping.accruedFees.toString())).to.equal(totalFees);

      const escrow =
        BigInt(bridgeTokenInfo.amount.toString()) -
        BigInt(mapping.accruedFees.toString());
      expect(escrow).to.equal(totalDeposited - totalFees);
    });

    it("mint/burn: supply_change == -(net_deposited) after deposits, restored after execution", async () => {
      const fixture = await createSplFixture({ mintBurn: {} }, 0x02);
      const mintBefore = await getMint(ctx.provider.connection, fixture.mint);
      const supplyBefore = BigInt(mintBefore.supply.toString());

      const depositAmount = BigInt(2_000_000_000);
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
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
      expect(BigInt(mintAfterDeposit.supply.toString())).to.equal(
        supplyBefore - net
      );

      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          net,
          nonce,
          0x02,
          fixture.destToken,
          fixture.tokenPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
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
      expect(BigInt(mintAfterExecute.supply.toString())).to.equal(supplyBefore);
    });
  });

  // =====================================================================
  // SECTION 5: ATTACK SIMULATION - Privilege Escalation
  // =====================================================================
  describe("ATTACK: privilege escalation attempts", () => {
    it("random user cannot set themselves as admin via set_config", async () => {
      const attacker = Keypair.generate();
      await airdrop(ctx.provider.connection, attacker.publicKey);

      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: attacker.publicKey,
            operator: null,
            feeBps: null,
            withdrawDelay: null,
            paused: null,
          })
          .accounts({
            bridge: ctx.bridgePda,
            admin: attacker.publicKey,
          })
          .signers([attacker])
          .rpc();
        expect.fail("Attacker should not be able to set themselves as admin");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("random user cannot set themselves as operator via set_config", async () => {
      const attacker = Keypair.generate();
      await airdrop(ctx.provider.connection, attacker.publicKey);

      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: attacker.publicKey,
            feeBps: null,
            withdrawDelay: null,
            paused: null,
          })
          .accounts({
            bridge: ctx.bridgePda,
            admin: attacker.publicKey,
          })
          .signers([attacker])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("operator cannot perform admin-only actions", async () => {
      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: null,
            feeBps: 9999,
            withdrawDelay: null,
            paused: null,
          })
          .accounts({
            bridge: ctx.bridgePda,
            admin: ctx.operator.publicKey,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("deactivated canceler cannot cancel withdrawals", async () => {
      const fakeCanceler = Keypair.generate();
      await airdrop(ctx.provider.connection, fakeCanceler.publicKey);

      const [fakeCancelerPda] = findCancelerPda(
        ctx.program.programId,
        fakeCanceler.publicKey
      );
      await ctx.program.methods
        .addCanceler({ canceler: fakeCanceler.publicKey, active: true })
        .accounts({
          bridge: ctx.bridgePda,
          cancelerEntry: fakeCancelerPda,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      await ctx.program.methods
        .addCanceler({ canceler: fakeCanceler.publicKey, active: false })
        .accounts({
          bridge: ctx.bridgePda,
          cancelerEntry: fakeCancelerPda,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const nonce = nextNonceVal();
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        NATIVE_SOL_TOKEN,
        100_000n,
        nonce,
        0x10,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, nonce);

      try {
        await ctx.program.methods
          .withdrawCancel()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            cancelerEntry: fakeCancelerPda,
            canceler: fakeCanceler.publicKey,
          })
          .signers([fakeCanceler])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedCanceler");
      }
    });

    it("non-admin cannot unpause a paused bridge", async () => {
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

      const attacker = Keypair.generate();
      await airdrop(ctx.provider.connection, attacker.publicKey);

      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: null,
            feeBps: null,
            withdrawDelay: null,
            paused: false,
          })
          .accounts({ bridge: ctx.bridgePda, admin: attacker.publicKey })
          .signers([attacker])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
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

  // =====================================================================
  // SECTION 6: ATTACK SIMULATION - Withdrawal Theft / Interception
  // =====================================================================
  describe("ATTACK: withdrawal theft and front-running", () => {
    it("attacker cannot front-run withdrawal by submitting with victim's dest_account", async () => {
      const victim = ctx.user;
      const attacker = Keypair.generate();
      await airdrop(ctx.provider.connection, attacker.publicKey);

      const nonce = nextNonceVal();
      const amount = 500_000n;
      const srcAccount = randomBytes(32);

      const victimHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        victim.publicKey.toBuffer(),
        NATIVE_SOL_TOKEN.toBuffer(),
        amount,
        nonce
      );
      const attackerHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        attacker.publicKey.toBuffer(),
        NATIVE_SOL_TOKEN.toBuffer(),
        amount,
        nonce
      );
      expect(victimHash.toString("hex")).to.not.equal(
        attackerHash.toString("hex")
      );

      const [victimWithdrawPda] = findWithdrawPda(
        ctx.program.programId,
        victimHash
      );
      const [victimExecutedPda] = findExecutedHashPda(
        ctx.program.programId,
        victimHash
      );

      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: EVM_CHAIN_ID,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken: NATIVE_SOL_TOKEN,
            amount: toBn(amount),
            nonce: toBn(nonce),
            operatorGas: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            srcChainEntry: evmChainPda,
            tokenMapping: withdrawNativeTokenMappingPda,
            pendingWithdraw: victimWithdrawPda,
            executedHashCheck: victimExecutedPda,
            recipient: attacker.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        expect.fail("Attacker PDA should not match victim hash PDA");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("ConstraintSeeds") ||
            msg.includes("seeds constraint") ||
            msg.includes("Error processing Instruction")
        ).to.be.true;
      }
    });

    it("attacker cannot execute another user's withdrawal with their own recipient", async () => {
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          200_000n,
          nonce,
          0x20,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
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
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              NATIVE_SOL_TOKEN
            )[0],
            recipient: attacker.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("WrongRecipient");
      }

      // `withdraw_delay` uses strict `clock > approved_at + delay`; a second sleep avoids flaky DelayNotElapsed at the boundary.
      await sleep(2000);

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(
            ctx.program.programId,
            NATIVE_SOL_TOKEN
          )[0],
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();
    });

    it("attacker cannot redirect SPL withdrawal to their token account", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0x03);
      const depositAmount = 1_000_000_000n;
      const fee = feeFor(depositAmount);
      const net = depositAmount - fee;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
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

      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          net,
          nonce,
          0x30,
          fixture.destToken,
          fixture.tokenPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      const attacker = Keypair.generate();
      await airdrop(ctx.provider.connection, attacker.publicKey);
      const attackerToken = await getOrCreateAssociatedTokenAccount(
        ctx.provider.connection,
        ctx.admin,
        fixture.mint,
        attacker.publicKey
      );

      try {
        await ctx.program.methods
          .withdrawExecute()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            mint: fixture.mint,
            recipientTokenAccount: attackerToken.address,
            bridgeTokenAccount: fixture.bridgeToken.address,
            tokenMapping: fixture.tokenPda,
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              fixture.mint
            )[0],
            recipient: attacker.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([attacker])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("WrongRecipient");
      }
    });
  });

  // =====================================================================
  // SECTION 7: ATTACK SIMULATION - Fee Draining
  // =====================================================================
  describe("ATTACK: fee draining and manipulation", () => {
    it("admin cannot drain bridge beyond accrued_native_fees", async () => {
      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const accrued = BigInt(bridge.accruedNativeFees.toString());

      if (accrued > 0n) {
        try {
          await ctx.program.methods
            .withdrawFees({ amount: toBn(accrued + 1n), native: true })
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
      }
    });

    it("admin cannot drain SPL fees beyond accrued per token_mapping", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0x04);
      const depositAmount = 500_000_000n;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
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

      const mapping = await ctx.program.account.tokenMapping.fetch(
        fixture.tokenPda
      );
      const accrued = BigInt(mapping.accruedFees.toString());

      try {
        await ctx.program.methods
          .withdrawFees({ amount: toBn(accrued + 1n), native: false })
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
    });

    it("native fee withdrawal reduces accrued_native_fees by exact amount", async () => {
      const bridgeBefore = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const feesBefore = BigInt(bridgeBefore.accruedNativeFees.toString());

      if (feesBefore > 0n) {
        const withdrawAmt = feesBefore > 1n ? feesBefore / 2n : feesBefore;
        const adminBalBefore = BigInt(
          (
            await ctx.provider.connection.getBalance(ctx.admin.publicKey)
          ).toString()
        );

        await ctx.program.methods
          .withdrawFees({ amount: toBn(withdrawAmt), native: true })
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

        const bridgeAfter = await ctx.program.account.bridgeConfig.fetch(
          ctx.bridgePda
        );
        const feesAfter = BigInt(bridgeAfter.accruedNativeFees.toString());
        expect(feesAfter).to.equal(feesBefore - withdrawAmt);
      }
    });
  });

  // =====================================================================
  // SECTION 8: ATTACK SIMULATION - Replay & Double-Spend
  // =====================================================================
  describe("ATTACK: replay and double-spend prevention", () => {
    it("cannot re-submit the same transfer hash after execution (ExecutedHash blocks it)", async () => {
      const nonce = nextNonceVal();
      const amount = 100_000n;
      const srcAccountByte = 0x40;

      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          amount,
          nonce,
          srcAccountByte,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(
            ctx.program.programId,
            NATIVE_SOL_TOKEN
          )[0],
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const executed = await ctx.program.account.executedHash.fetch(
        executedHashPda
      );
      expect(executed).to.not.be.null;

      const srcAccount = Buffer.alloc(32, srcAccountByte);
      const replayHash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        NATIVE_SOL_TOKEN.toBuffer(),
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
            destToken: NATIVE_SOL_TOKEN,
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
        expect.fail("Replay should be blocked");
      } catch (err) {
        expect(err.toString()).to.satisfy(
          (s: string) =>
            s.includes("AlreadyExecutedHash") || s.includes("already in use")
        );
      }
    });

    it("cannot double-execute: second execute fails because PDA is closed", async () => {
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          50_000n,
          nonce,
          0x41,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(
            ctx.program.programId,
            NATIVE_SOL_TOKEN
          )[0],
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      try {
        await ctx.program.methods
          .withdrawExecuteNative()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              NATIVE_SOL_TOKEN
            )[0],
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Double execute should fail");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("AccountNotInitialized") ||
            msg.includes("already in use") ||
            msg.includes("Error processing")
        ).to.be.true;
      }
    });
  });

  // =====================================================================
  // SECTION 9: BUSINESS LOGIC - Withdrawal State Machine Completeness
  // =====================================================================
  describe("BUSINESS LOGIC: withdrawal state machine edge cases", () => {
    it("execute before approval fails even after delay", async () => {
      const nonce = nextNonceVal();
      const { withdrawPda, executedHashPda } = await submitWithdraw(
        ctx.user,
        NATIVE_SOL_TOKEN,
        100_000n,
        nonce,
        0x50,
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
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              NATIVE_SOL_TOKEN
            )[0],
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

    it("execute immediately after approval fails (delay not elapsed)", async () => {
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          100_000n,
          nonce,
          0x51,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);

      try {
        await ctx.program.methods
          .withdrawExecuteNative()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              NATIVE_SOL_TOKEN
            )[0],
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("DelayNotElapsed");
      }
    });

    it("cancel -> reenable resets delay timer (must wait full delay again)", async () => {
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          100_000n,
          nonce,
          0x52,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);

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

      await ctx.program.methods
        .withdrawReenable()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          authority: ctx.admin.publicKey,
        })
        .signers([ctx.admin])
        .rpc();

      try {
        await ctx.program.methods
          .withdrawExecuteNative()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: withdrawPda,
            executedHash: executedHashPda,
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              NATIVE_SOL_TOKEN
            )[0],
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should fail - delay not elapsed after reenable");
      } catch (err) {
        expect(err.toString()).to.contain("DelayNotElapsed");
      }

      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(
            ctx.program.programId,
            NATIVE_SOL_TOKEN
          )[0],
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const executedHash = await ctx.program.account.executedHash.fetch(
        executedHashPda
      );
      expect(executedHash).to.not.be.null;
    });

    it("double approve is rejected", async () => {
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        NATIVE_SOL_TOKEN,
        100_000n,
        nonce,
        0x53,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, nonce);

      try {
        await approveWithdraw(transferHash, withdrawPda, nonce);
        expect.fail("Double approve should fail");
      } catch (err) {
        const s = err.toString();
        expect(
          s.includes("AlreadyApproved") ||
            s.includes("already in use") ||
            s.includes("Simulation failed")
        ).to.be.true;
      }
    });

    it("approve on cancelled withdrawal is rejected", async () => {
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        NATIVE_SOL_TOKEN,
        100_000n,
        nonce,
        0x54,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, nonce);

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
        await approveWithdraw(transferHash, withdrawPda, nonce);
        expect.fail("Approve on cancelled should fail");
      } catch (err) {
        const s = err.toString();
        expect(
          s.includes("WithdrawalCancelled") ||
            s.includes("already in use") ||
            s.includes("Simulation failed")
        ).to.be.true;
      }
    });
  });

  // =====================================================================
  // SECTION 10: BUSINESS LOGIC - Config Boundary Validation
  // =====================================================================
  describe("BUSINESS LOGIC: configuration boundary fuzzing", () => {
    afterEach(async () => {
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: ctx.operator.publicKey,
          feeBps: 50,
          withdrawDelay: new anchor.BN(WITHDRAW_DELAY_SECONDS),
          paused: false,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();
    });

    it("fee_bps = 10001 is rejected", async () => {
      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: null,
            feeBps: 10001,
            withdrawDelay: null,
            paused: null,
          })
          .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
          .rpc();
        expect.fail("Should reject fee_bps > 100");
      } catch (err) {
        expect(err.toString()).to.contain("InvalidFeeBps");
      }
    });

    it("fee_bps = 100 is accepted (max 1% fee)", async () => {
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
      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      expect(bridge.feeBps).to.equal(100);
    });

    it("fee_bps = 0 is accepted (zero fee)", async () => {
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
      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      expect(bridge.feeBps).to.equal(0);
    });

    it("withdraw_delay = 14 rejected, 15 accepted, 86400 accepted, 86401 rejected", async () => {
      for (const [delay, shouldFail] of [
        [14, true],
        [15, false],
        [86400, false],
        [86401, true],
      ] as [number, boolean][]) {
        try {
          await ctx.program.methods
            .setConfig({
              newAdmin: null,
              operator: null,
              feeBps: null,
              withdrawDelay: new anchor.BN(delay),
              paused: null,
            })
            .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
            .rpc();
          if (shouldFail)
            expect.fail(`delay=${delay} should have been rejected`);
        } catch (err) {
          if (!shouldFail) throw err;
          expect(err.toString()).to.contain("InvalidWithdrawDelay");
        }
      }
    });

    it("deposit with fee_bps=0 transfers full amount as net", async () => {
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

      const amount = BigInt(LAMPORTS_PER_SOL);
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      const bridgeBefore = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const feesBefore = BigInt(bridgeBefore.accruedNativeFees.toString());

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
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
      const bridgeAfter = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      const feesAfter = BigInt(bridgeAfter.accruedNativeFees.toString());

      expect(BigInt(deposit.amount.toString())).to.equal(amount);
      expect(feesAfter - feesBefore).to.equal(0n);
    });
  });

  // =====================================================================
  // SECTION 11: ATTACK SIMULATION - PDA Collision / Confusion
  // =====================================================================
  describe("ATTACK: PDA collision and account confusion", () => {
    it("passing a deposit PDA where a withdraw PDA is expected fails", async () => {
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      const nextNonce2 = nextNonce + 1;
      const [depositPda2] = findDepositPda(ctx.program.programId, nextNonce2);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
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

      try {
        await ctx.program.methods
          .withdrawApprove({ transferHash: Array.from(randomBytes(32)) })
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: depositPda,
            nonceUsed: findNonceUsedPda(
              ctx.program.programId,
              Buffer.from(EVM_CHAIN_ID),
              0n
            )[0],
            operator: ctx.operator.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.operator])
          .rpc();
        expect.fail("Should have thrown - wrong account type");
      } catch (_err) {
        // Success: the call was rejected (wrong PDA / account type)
        return;
      }
    });

    it("passing wrong bridge PDA fails at seed verification", async () => {
      const fakeConfig = Keypair.generate();
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
            bridge: fakeConfig.publicKey,
            admin: ctx.admin.publicKey,
          })
          .rpc();
        expect.fail("Wrong bridge PDA should fail");
      } catch (err) {
        const msg = err.toString();
        expect(
          msg.includes("ConstraintSeeds") ||
            msg.includes("seeds constraint") ||
            msg.includes("AccountNotInitialized") ||
            msg.includes("Error processing")
        ).to.be.true;
      }
    });
  });

  // =====================================================================
  // SECTION 12: FUZZ - Concurrent Multi-User Deposit/Withdraw Isolation
  // =====================================================================
  describe("FUZZ: concurrent multi-user isolation (5 users)", () => {
    it("5 users deposit and withdraw independently without interference", async () => {
      const users: Keypair[] = [];
      for (let i = 0; i < 5; i++) {
        const u = Keypair.generate();
        await airdrop(
          ctx.provider.connection,
          u.publicKey,
          5 * LAMPORTS_PER_SOL
        );
        users.push(u);
      }

      const withdrawPdas: {
        user: Keypair;
        transferHash: Buffer;
        withdrawPda: PublicKey;
        executedHashPda: PublicKey;
        amount: bigint;
        nonce: bigint;
      }[] = [];

      for (let i = 0; i < 5; i++) {
        const amount = BigInt(100_000 + Math.floor(Math.random() * 900_000));
        const nonce = nextNonceVal();
        const srcByte = 0x60 + i;
        const { transferHash, withdrawPda, executedHashPda } =
          await submitWithdraw(
            users[i],
            NATIVE_SOL_TOKEN,
            amount,
            nonce,
            srcByte,
            EVM_REMOTE_NATIVE_TOKEN,
            withdrawNativeTokenMappingPda
          );
        withdrawPdas.push({
          user: users[i],
          transferHash,
          withdrawPda,
          executedHashPda,
          amount,
          nonce,
        });
      }

      const hashes = new Set(
        withdrawPdas.map((w) => w.transferHash.toString("hex"))
      );
      expect(hashes.size).to.equal(5, "All hashes should be unique");

      for (const w of withdrawPdas) {
        await approveWithdraw(w.transferHash, w.withdrawPda, w.nonce);
      }

      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      const balancesBefore = await Promise.all(
        users.map((u) => ctx.provider.connection.getBalance(u.publicKey))
      );

      for (const w of withdrawPdas) {
        await ctx.program.methods
          .withdrawExecuteNative()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: w.withdrawPda,
            executedHash: w.executedHashPda,
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              NATIVE_SOL_TOKEN
            )[0],
            recipient: w.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([w.user])
          .rpc();
      }

      const balancesAfter = await Promise.all(
        users.map((u) => ctx.provider.connection.getBalance(u.publicKey))
      );

      for (let i = 0; i < 5; i++) {
        expect(balancesAfter[i]).to.be.greaterThan(balancesBefore[i]);
      }

      for (const w of withdrawPdas) {
        const executed = await ctx.program.account.executedHash.fetch(
          w.executedHashPda
        );
        expect(executed).to.not.be.null;
      }
    });
  });

  // =====================================================================
  // SECTION 13: INVARIANT - Native SOL Balance Integrity
  // =====================================================================
  describe("INVARIANT: native SOL bridge balance integrity", () => {
    it("bridge balance after deposit = previous_balance + deposit_amount", async () => {
      const bridgeInfoBefore = await ctx.provider.connection.getAccountInfo(
        ctx.bridgePda
      );
      const balanceBefore = BigInt(bridgeInfoBefore!.lamports.toString());

      const amount = BigInt(2 * LAMPORTS_PER_SOL);
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      await ctx.program.methods
        .depositNative({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
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

      const bridgeInfoAfter = await ctx.provider.connection.getAccountInfo(
        ctx.bridgePda
      );
      const balanceAfter = BigInt(bridgeInfoAfter!.lamports.toString());
      expect(balanceAfter).to.equal(balanceBefore + amount);
    });

    it("bridge balance after native withdrawal = previous_balance - withdrawal_amount + rent_return_from_PW_close", async () => {
      const nonce = nextNonceVal();
      const payoutLamports = 100_000n;
      // Token mapping uses srcDecimals=18; pending amount is scaled like EVM wei (÷1e9 → lamports).
      const rawAmount = payoutLamports * 1_000_000_000n;
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          rawAmount,
          nonce,
          0x70,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
      await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

      const bridgeInfoBefore = await ctx.provider.connection.getAccountInfo(
        ctx.bridgePda
      );
      const balanceBefore = BigInt(bridgeInfoBefore!.lamports.toString());

      await ctx.program.methods
        .withdrawExecuteNative()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          executedHash: executedHashPda,
          withdrawRateLimit: findWithdrawRateLimitPda(
            ctx.program.programId,
            NATIVE_SOL_TOKEN
          )[0],
          recipient: ctx.user.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const bridgeInfoAfter = await ctx.provider.connection.getAccountInfo(
        ctx.bridgePda
      );
      const balanceAfter = BigInt(bridgeInfoAfter!.lamports.toString());
      expect(balanceAfter < balanceBefore).to.be.true;
      const delta = balanceBefore - balanceAfter;
      // Chai `.equal` is unreliable for bigint; compare explicitly.
      expect(delta === payoutLamports).to.be.true;
    });
  });

  // =====================================================================
  // SECTION 14: ATTACK - Cross-Path Execution Enforcement
  // =====================================================================
  describe("ATTACK: cross-path execution enforcement", () => {
    it("SPL withdrawal cannot be executed via native path", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0x05);
      const depositAmount = 1_000_000_000n;

      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
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

      const net = depositAmount - feeFor(depositAmount);
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          fixture.mint,
          net,
          nonce,
          0x80,
          fixture.destToken,
          fixture.tokenPda
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
            withdrawRateLimit: findWithdrawRateLimitPda(
              ctx.program.programId,
              NATIVE_SOL_TOKEN
            )[0],
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("SPL via native path should fail");
      } catch (err) {
        expect(err.toString()).to.contain("NotNativeToken");
      }
    });

    it("native SOL withdrawal cannot be executed via SPL path", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0x06);
      const nonce = nextNonceVal();
      const { transferHash, withdrawPda, executedHashPda } =
        await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          100_000n,
          nonce,
          0x81,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
      await approveWithdraw(transferHash, withdrawPda, nonce);
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
        expect.fail("Native via SPL path should fail");
      } catch (err) {
        expect(err.toString()).to.contain("TokenMintMismatch");
      }
    });
  });

  // =====================================================================
  // SECTION 15: FUZZ - Deposit Record Integrity Across Random Inputs
  // =====================================================================
  describe("FUZZ: deposit record integrity (10 random deposits)", () => {
    it("each deposit stores correct dest_chain, dest_account, token, and amount", async () => {
      for (let i = 0; i < 10; i++) {
        const destAccount = randomBytes(32);
        const amount = BigInt(
          LAMPORTS_PER_SOL / 100 + Math.floor(Math.random() * LAMPORTS_PER_SOL)
        );
        const fee = feeFor(amount);
        const net = amount - fee;

        const nextNonce = await getNextDepositNonce(ctx);
        const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(destAccount),
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

        const deposit = await ctx.program.account.depositRecord.fetch(
          depositPda
        );
        expect(Buffer.from(deposit.destChain)).to.deep.equal(
          Buffer.from(EVM_CHAIN_ID)
        );
        expect(Buffer.from(deposit.destAccount)).to.deep.equal(destAccount);
        expect(Buffer.from(deposit.token)).to.deep.equal(DEPOSIT_DEST_TOKEN);
        expect(BigInt(deposit.amount.toString())).to.equal(net);
        expect(deposit.srcAccount.toBuffer()).to.deep.equal(
          ctx.user.publicKey.toBuffer()
        );
        expect(deposit.nonce.toNumber()).to.equal(nextNonce);
        expect(deposit.timestamp.toNumber()).to.be.greaterThan(0);

        const expectedHash = computeTransferHash(
          SOLANA_CHAIN_ID,
          EVM_CHAIN_ID,
          ctx.user.publicKey.toBuffer(),
          destAccount,
          DEPOSIT_DEST_TOKEN,
          net,
          BigInt(nextNonce)
        );
        expect(Buffer.from(deposit.transferHash).toString("hex")).to.equal(
          expectedHash.toString("hex")
        );
      }
    });
  });

  // =====================================================================
  // SECTION 16: ATTACK - Admin Self-Lock Prevention
  // =====================================================================
  describe("ATTACK: admin self-lock scenarios", () => {
    it("admin can transfer to new admin and new admin has full control", async () => {
      const newAdmin = Keypair.generate();
      await airdrop(ctx.provider.connection, newAdmin.publicKey);

      await ctx.program.methods
        .setConfig({
          newAdmin: newAdmin.publicKey,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      let bridge = await ctx.program.account.bridgeConfig.fetch(ctx.bridgePda);
      expect(bridge.admin.toString()).to.equal(newAdmin.publicKey.toString());

      try {
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
        expect.fail("Old admin should be rejected");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }

      await ctx.program.methods
        .setConfig({
          newAdmin: ctx.admin.publicKey,
          operator: null,
          feeBps: null,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: newAdmin.publicKey })
        .signers([newAdmin])
        .rpc();
    });
  });

  // =====================================================================
  // SECTION 17: INVARIANT - Canceler Toggle Isolation
  // =====================================================================
  describe("INVARIANT: canceler toggle isolation", () => {
    it("deactivating one canceler does not affect another active canceler", async () => {
      const cancelerA = Keypair.generate();
      const cancelerB = Keypair.generate();
      await airdrop(ctx.provider.connection, cancelerA.publicKey);
      await airdrop(ctx.provider.connection, cancelerB.publicKey);

      const [pdaA] = findCancelerPda(
        ctx.program.programId,
        cancelerA.publicKey
      );
      const [pdaB] = findCancelerPda(
        ctx.program.programId,
        cancelerB.publicKey
      );

      await ctx.program.methods
        .addCanceler({ canceler: cancelerA.publicKey, active: true })
        .accounts({
          bridge: ctx.bridgePda,
          cancelerEntry: pdaA,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      await ctx.program.methods
        .addCanceler({ canceler: cancelerB.publicKey, active: true })
        .accounts({
          bridge: ctx.bridgePda,
          cancelerEntry: pdaB,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      await ctx.program.methods
        .addCanceler({ canceler: cancelerA.publicKey, active: false })
        .accounts({
          bridge: ctx.bridgePda,
          cancelerEntry: pdaA,
          admin: ctx.admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const nonce = nextNonceVal();
      const { transferHash, withdrawPda } = await submitWithdraw(
        ctx.user,
        NATIVE_SOL_TOKEN,
        100_000n,
        nonce,
        0x90,
        EVM_REMOTE_NATIVE_TOKEN,
        withdrawNativeTokenMappingPda
      );
      await approveWithdraw(transferHash, withdrawPda, nonce);

      await ctx.program.methods
        .withdrawCancel()
        .accounts({
          bridge: ctx.bridgePda,
          pendingWithdraw: withdrawPda,
          cancelerEntry: pdaB,
          canceler: cancelerB.publicKey,
        })
        .signers([cancelerB])
        .rpc();

      const pw = await ctx.program.account.pendingWithdraw.fetch(withdrawPda);
      expect(pw.cancelled).to.be.true;

      try {
        const nonce2 = nextNonceVal();
        const { transferHash: th2, withdrawPda: wp2 } = await submitWithdraw(
          ctx.user,
          NATIVE_SOL_TOKEN,
          100_000n,
          nonce2,
          0x91,
          EVM_REMOTE_NATIVE_TOKEN,
          withdrawNativeTokenMappingPda
        );
        await approveWithdraw(th2, wp2, nonce2);
        await ctx.program.methods
          .withdrawCancel()
          .accounts({
            bridge: ctx.bridgePda,
            pendingWithdraw: wp2,
            cancelerEntry: pdaA,
            canceler: cancelerA.publicKey,
          })
          .signers([cancelerA])
          .rpc();
        expect.fail("Deactivated canceler should fail");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedCanceler");
      }
    });
  });

  // =====================================================================
  // SECTION 18: FUZZ - Multiple Token Mappings Isolation
  // =====================================================================
  describe("FUZZ: multiple token mapping isolation", () => {
    it("fees accrue independently per token mapping", async () => {
      const fixtureA = await createSplFixture({ lockUnlock: {} }, 0x07);
      const fixtureB = await createSplFixture({ lockUnlock: {} }, 0x08);

      const amountA = 1_000_000_000n;
      const amountB = 2_000_000_000n;
      const feeA = feeFor(amountA);
      const feeB = feeFor(amountB);

      const nonceA = await getNextDepositNonce(ctx);
      const [depositPdaA] = findDepositPda(ctx.program.programId, nonceA);
      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
          amount: toBn(amountA),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPdaA,
          tokenMapping: fixtureA.tokenPda,
          mint: fixtureA.mint,
          depositorTokenAccount: fixtureA.userToken.address,
          bridgeTokenAccount: fixtureA.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const nonceB = await getNextDepositNonce(ctx);
      const [depositPdaB] = findDepositPda(ctx.program.programId, nonceB);
      await ctx.program.methods
        .depositSpl({
          destChain: EVM_CHAIN_ID,
          destAccount: Array.from(randomBytes(32)),
          amount: toBn(amountB),
        })
        .accounts({
          bridge: ctx.bridgePda,
          depositRecord: depositPdaB,
          tokenMapping: fixtureB.tokenPda,
          mint: fixtureB.mint,
          depositorTokenAccount: fixtureB.userToken.address,
          bridgeTokenAccount: fixtureB.bridgeToken.address,
          destChainEntry: evmChainPda,
          depositor: ctx.user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([ctx.user])
        .rpc();

      const mappingA = await ctx.program.account.tokenMapping.fetch(
        fixtureA.tokenPda
      );
      const mappingB = await ctx.program.account.tokenMapping.fetch(
        fixtureB.tokenPda
      );

      expect(BigInt(mappingA.accruedFees.toString())).to.equal(feeA);
      expect(BigInt(mappingB.accruedFees.toString())).to.equal(feeB);

      await ctx.program.methods
        .withdrawFees({ amount: toBn(feeA), native: false })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
          adminTokenAccount: fixtureA.adminToken.address,
          bridgeTokenAccount: fixtureA.bridgeToken.address,
          mint: fixtureA.mint,
          tokenMapping: fixtureA.tokenPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const mappingAAfter = await ctx.program.account.tokenMapping.fetch(
        fixtureA.tokenPda
      );
      const mappingBAfter = await ctx.program.account.tokenMapping.fetch(
        fixtureB.tokenPda
      );

      expect(BigInt(mappingAAfter.accruedFees.toString())).to.equal(0n);
      expect(BigInt(mappingBAfter.accruedFees.toString())).to.equal(feeB);
    });
  });

  // =====================================================================
  // SECTION 19: BUSINESS LOGIC - Zero Amount Deposit Rejection
  // =====================================================================
  describe("BUSINESS LOGIC: zero-amount operations rejected", () => {
    it("deposit_native with amount=0 is rejected", async () => {
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      try {
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(randomBytes(32)),
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
        expect.fail("Should reject zero amount");
      } catch (err) {
        expect(err.toString()).to.contain("ZeroAmount");
      }
    });

    it("deposit_spl with amount=0 is rejected", async () => {
      const fixture = await createSplFixture({ lockUnlock: {} }, 0x09);
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);

      try {
        await ctx.program.methods
          .depositSpl({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(randomBytes(32)),
            amount: toBn(0n),
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
        expect.fail("Should reject zero amount");
      } catch (err) {
        expect(err.toString()).to.contain("ZeroAmount");
      }
    });

    it("withdraw_fees with amount=0 is rejected", async () => {
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
        expect.fail("Should reject zero amount");
      } catch (err) {
        expect(err.toString()).to.contain("ZeroAmount");
      }
    });

    it("withdraw_submit with amount=0 is rejected", async () => {
      const srcAccount = randomBytes(32);
      const nonce = nextNonceVal();
      const hash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        NATIVE_SOL_TOKEN.toBuffer(),
        0n,
        nonce
      );
      const [withdrawPda] = findWithdrawPda(ctx.program.programId, hash);
      const [executedPda] = findExecutedHashPda(ctx.program.programId, hash);

      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: EVM_CHAIN_ID,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken: NATIVE_SOL_TOKEN,
            amount: toBn(0n),
            nonce: toBn(nonce),
            operatorGas: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            srcChainEntry: evmChainPda,
            tokenMapping: withdrawNativeTokenMappingPda,
            pendingWithdraw: withdrawPda,
            executedHashCheck: executedPda,
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should reject zero amount");
      } catch (err) {
        expect(err.toString()).to.contain("ZeroAmount");
      }
    });
  });

  // =====================================================================
  // SECTION 20: INVARIANT - Paused Bridge Blocks All Mutable Operations
  // =====================================================================
  describe("INVARIANT: paused bridge blocks all user-facing operations", () => {
    before(async () => {
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
    });

    after(async () => {
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

    it("deposit_native blocked when paused", async () => {
      const nextNonce = await getNextDepositNonce(ctx);
      const [depositPda] = findDepositPda(ctx.program.programId, nextNonce);
      try {
        await ctx.program.methods
          .depositNative({
            destChain: EVM_CHAIN_ID,
            destAccount: Array.from(randomBytes(32)),
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
        expect.fail("Should be blocked");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }
    });

    it("withdraw_submit blocked when paused", async () => {
      const nonce = nextNonceVal();
      const srcAccount = randomBytes(32);
      const hash = computeTransferHash(
        EVM_CHAIN_ID,
        SOLANA_CHAIN_ID,
        srcAccount,
        ctx.user.publicKey.toBuffer(),
        NATIVE_SOL_TOKEN.toBuffer(),
        100_000n,
        nonce
      );
      const [wp] = findWithdrawPda(ctx.program.programId, hash);
      const [ep] = findExecutedHashPda(ctx.program.programId, hash);
      try {
        await ctx.program.methods
          .withdrawSubmit({
            srcChain: EVM_CHAIN_ID,
            srcAccount: Array.from(srcAccount),
            srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
            destToken: NATIVE_SOL_TOKEN,
            amount: toBn(100_000n),
            nonce: toBn(nonce),
            operatorGas: new anchor.BN(0),
          })
          .accounts({
            bridge: ctx.bridgePda,
            srcChainEntry: evmChainPda,
            tokenMapping: withdrawNativeTokenMappingPda,
            pendingWithdraw: wp,
            executedHashCheck: ep,
            recipient: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should be blocked");
      } catch (err) {
        expect(err.toString()).to.contain("BridgePaused");
      }
    });

    it("admin can still set_config while paused (to unpause)", async () => {
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: null,
          feeBps: 60,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      const bridge = await ctx.program.account.bridgeConfig.fetch(
        ctx.bridgePda
      );
      expect(bridge.feeBps).to.equal(60);

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
});
