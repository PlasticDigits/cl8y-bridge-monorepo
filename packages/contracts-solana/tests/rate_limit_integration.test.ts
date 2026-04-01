/**
 * Integration tests for `set_rate_limit` and on-chain withdraw rate limit PDAs.
 * Complements Rust unit tests in `rate_limit.rs` and `decimal.rs`.
 */
import * as anchor from "@coral-xyz/anchor";
import { SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import {
  TestContext,
  findWithdrawRateLimitPda,
  initializeBridgeIfNeeded,
  setupTest,
  NATIVE_SOL_TOKEN,
} from "./helpers/setup";

describe("rate limit admin + account layout", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupTest();
    await initializeBridgeIfNeeded(ctx, {
      operator: ctx.operator.publicKey,
      feeBps: 50,
      withdrawDelay: new anchor.BN(15),
      chainId: [0x00, 0x00, 0x00, 0x05],
    });
  });

  it("admin can set explicit rate limits for native SOL key", async () => {
    const [pda] = findWithdrawRateLimitPda(
      ctx.program.programId,
      NATIVE_SOL_TOKEN
    );
    await ctx.program.methods
      .setRateLimit({
        localMint: NATIVE_SOL_TOKEN,
        minPerTransaction: new anchor.BN(0),
        maxPerTransaction: new anchor.BN(0),
        maxPerPeriod: new anchor.BN(0),
      })
      .accounts({
        bridge: ctx.bridgePda,
        withdrawRateLimit: pda,
        admin: ctx.admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([ctx.admin])
      .rpc();

    const acc = await ctx.program.account.withdrawRateLimit.fetch(pda);
    expect(acc.explicitConfig).to.be.true;
    expect(acc.minPerTransaction.toString()).to.equal("0");
    expect(acc.maxPerTransaction.toString()).to.equal("0");
    expect(acc.maxPerPeriod.toString()).to.equal("0");
  });
});
