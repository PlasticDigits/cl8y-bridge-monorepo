/** INV-W3 / canceler allowlist — see docs/SOLANA_BRIDGE_INVARIANTS.md */
import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import {
  setupTest,
  findWithdrawPda,
  findWithdrawRateLimitPda,
  findCancelerPda,
  findExecutedHashPda,
  findTokenPda,
  findNonceUsedPda,
  TestContext,
  initializeBridgeIfNeeded,
  registerChainIfNeeded,
  NATIVE_SOL_TOKEN,
} from "./helpers/setup";
import { computeTransferHash } from "./helpers/hash";

const SOLANA_CHAIN_ID = [0x00, 0x00, 0x00, 0x05];
const EVM_CHAIN_ID = [0x00, 0x00, 0x00, 0x01];
const WITHDRAW_DELAY_SECS = 15;

const EVM_REMOTE_NATIVE_TOKEN = Buffer.alloc(32);
EVM_REMOTE_NATIVE_TOKEN[31] = 0x37;

async function sleep(ms: number): Promise<void> {
  await new Promise((r) => setTimeout(r, ms));
}

describe("cancel blocks theft (full guarantee)", () => {
  let ctx: TestContext;
  let transferHash: Buffer;
  let withdrawPda: PublicKey;
  let executedHashPda: PublicKey;
  let cancelerPda: PublicKey;
  let evmChainPda: PublicKey;
  let withdrawNativeTokenMappingPda: PublicKey;

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

    evmChainPda = await registerChainIfNeeded(ctx, EVM_CHAIN_ID, "evm_1");

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
        srcToken: Array.from(EVM_REMOTE_NATIVE_TOKEN),
        destToken: NATIVE_SOL_TOKEN,
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
  });

  it("operator approve → cancel → past delay → execute blocked; reenable keeps approval; second cancel still blocks", async () => {
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
          withdrawRateLimit: findWithdrawRateLimitPda(ctx.program.programId, NATIVE_SOL_TOKEN)[0],
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
        authority: ctx.admin.publicKey,
      })
      .rpc();

    const pwAfterReenable = await ctx.program.account.pendingWithdraw.fetch(
      withdrawPda
    );
    expect(pwAfterReenable.cancelled).to.be.false;
    expect(pwAfterReenable.approved).to.be.true;

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
      expect.fail("execute should fail until delay after uncancel");
    } catch (err) {
      expect(err.toString()).to.contain("DelayNotElapsed");
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

    await sleep((WITHDRAW_DELAY_SECS + 1) * 1000);

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
      expect.fail("execute should fail after second cancel");
    } catch (err) {
      expect(err.toString()).to.match(/WithdrawalCancelled|6007|cancel/i);
    }
  });
});
