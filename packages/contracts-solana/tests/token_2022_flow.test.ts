/**
 * Plain Token-2022 mint (no transfer-fee extension, no rebasing / interest-bearing extensions).
 * Bridge uses `token_interface`; this file is the supported Token-2022 subset — see INV-D3 in
 * docs/SOLANA_BRIDGE_INVARIANTS.md.
 */
import * as anchor from "@coral-xyz/anchor";
import {
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  createMint,
  getAccount,
  getAssociatedTokenAddressSync,
  mintTo,
} from "@solana/spl-token";
import { expect } from "chai";

import { computeTransferHash } from "./helpers/hash";
import {
  TestContext,
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
  airdrop,
} from "./helpers/setup";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];
const WITHDRAW_DELAY_SECONDS = 15;
const TP2022 = TOKEN_2022_PROGRAM_ID;

function toBn(value: bigint | number): anchor.BN {
  return new anchor.BN(value.toString());
}

function feeFor(amount: bigint, feeBps = 50n): bigint {
  return (amount * feeBps) / 10000n;
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

describe("Token-2022 plain mint (lock/unlock deposit → withdraw execute)", () => {
  let ctx: TestContext;
  let evmChainPda: PublicKey;
  let mint: PublicKey;
  let tokenPda: PublicKey;
  let destToken: Buffer;
  let userToken: { address: PublicKey };
  let bridgeToken: { address: PublicKey };

  before(async () => {
    ctx = await setupTest();
    // Late in the full `anchor test` run the admin can be low on SOL for new ATAs.
    await airdrop(
      ctx.provider.connection,
      ctx.admin.publicKey,
      100 * LAMPORTS_PER_SOL
    );
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

    evmChainPda = await registerChainIfNeeded(ctx, EVM_CHAIN_ID, "evm_t2022");

    mint = await createMint(
      ctx.provider.connection,
      ctx.admin,
      ctx.admin.publicKey,
      null,
      9,
      undefined,
      undefined,
      TP2022
    );

    // Idempotent ATA creation surfaces CPI errors; getOrCreate swallows create failures and
    // then throws TokenAccountNotFoundError (misleading when Token-2022 / rent is the root cause).
    const userAta = getAssociatedTokenAddressSync(
      mint,
      ctx.user.publicKey,
      false,
      TP2022
    );
    const bridgeAta = getAssociatedTokenAddressSync(
      mint,
      ctx.bridgePda,
      true,
      TP2022
    );
    const ataTx = new Transaction().add(
      createAssociatedTokenAccountIdempotentInstruction(
        ctx.admin.publicKey,
        userAta,
        ctx.user.publicKey,
        mint,
        TP2022
      ),
      createAssociatedTokenAccountIdempotentInstruction(
        ctx.admin.publicKey,
        bridgeAta,
        ctx.bridgePda,
        mint,
        TP2022
      )
    );
    await sendAndConfirmTransaction(ctx.provider.connection, ataTx, [ctx.admin], {
      commitment: "confirmed",
    });
    userToken = { address: userAta };
    bridgeToken = { address: bridgeAta };

    await mintTo(
      ctx.provider.connection,
      ctx.admin,
      mint,
      userToken.address,
      ctx.admin,
      3_000_000_000,
      [],
      undefined,
      TP2022
    );

    destToken = Buffer.alloc(32, 0);
    destToken[31] = 0x88;
    [tokenPda] = findTokenPda(ctx.program.programId, EVM_CHAIN_ID, destToken);

    const existing = await ctx.provider.connection.getAccountInfo(tokenPda);
    if (!existing) {
      await ctx.program.methods
        .registerToken({
          localMint: mint,
          destChain: EVM_CHAIN_ID,
          destToken: Array.from(destToken),
          mode: { lockUnlock: {} },
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

    await setExplicitUnlimitedWithdrawRateLimit(ctx, mint);
  });

  it("depositSpl and withdrawExecute use Token-2022 program id and match transfer hash", async () => {
    const depositAmount = 1_000_000_000n;
    const expectedFee = feeFor(depositAmount);
    const expectedNet = depositAmount - expectedFee;
    const destAccount = Buffer.alloc(32, 0x42);

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
        tokenMapping: tokenPda,
        mint,
        depositorTokenAccount: userToken.address,
        bridgeTokenAccount: bridgeToken.address,
        destChainEntry: evmChainPda,
        depositor: ctx.user.publicKey,
        tokenProgram: TP2022,
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
      destToken,
      expectedNet,
      BigInt(nextNonce)
    );
    expect(Buffer.from(deposit.transferHash).toString("hex")).to.equal(
      expectedHash.toString("hex")
    );

    const bridgeBal = await getAccount(
      ctx.provider.connection,
      bridgeToken.address,
      undefined,
      TP2022
    );
    expect(Number(bridgeBal.amount)).to.equal(Number(depositAmount));

    const srcAccount = Buffer.alloc(32, 0x11);
    const withdrawNonce = 9001n;
    const transferHash = computeTransferHash(
      EVM_CHAIN_ID,
      SOLANA_CHAIN_ID,
      srcAccount,
      ctx.user.publicKey.toBuffer(),
      mint.toBuffer(),
      expectedNet,
      withdrawNonce
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
        srcToken: Array.from(destToken),
        destToken: mint,
        destAccount: ctx.user.publicKey,
        amount: toBn(expectedNet),
        nonce: toBn(withdrawNonce),
        operatorGas: new anchor.BN(0),
      })
      .accounts({
        bridge: ctx.bridgePda,
        srcChainEntry: evmChainPda,
        tokenMapping: tokenPda,
        pendingWithdraw: withdrawPda,
        executedHashCheck: executedHashPda,
        payer: ctx.user.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([ctx.user])
      .rpc();

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

    await sleep((WITHDRAW_DELAY_SECONDS + 3) * 1000);

    await ctx.program.methods
      .withdrawExecute()
      .accounts({
        bridge: ctx.bridgePda,
        pendingWithdraw: withdrawPda,
        executedHash: executedHashPda,
        mint,
        recipientTokenAccount: userToken.address,
        bridgeTokenAccount: bridgeToken.address,
        tokenMapping: tokenPda,
        withdrawRateLimit: findWithdrawRateLimitPda(
          ctx.program.programId,
          mint
        )[0],
        recipient: ctx.user.publicKey,
        tokenProgram: TP2022,
        systemProgram: SystemProgram.programId,
      })
      .signers([ctx.user])
      .rpc();

    const bridgeAfter = await getAccount(
      ctx.provider.connection,
      bridgeToken.address,
      undefined,
      TP2022
    );
    // Lock/unlock: full gross sat in bridge; execute unlocks net; fee remains until admin withdraw_fees.
    expect(Number(bridgeAfter.amount)).to.equal(Number(expectedFee));
  });
});
