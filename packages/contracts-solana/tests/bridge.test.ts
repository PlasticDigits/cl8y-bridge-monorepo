import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { Cl8yBridge } from "../target/types/cl8y_bridge";
import { setupTest, findBridgePda, findTokenPda, airdrop, TestContext, initializeBridgeIfNeeded } from "./helpers/setup";

describe("cl8y-bridge", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupTest();
    await initializeBridgeIfNeeded(ctx, {
      operator: ctx.operator.publicKey,
      feeBps: 50,
      withdrawDelay: new anchor.BN(300),
      chainId: [0x00, 0x00, 0x00, 0x05],
    });
    await ctx.program.methods
      .setConfig({
        newAdmin: null,
        operator: ctx.operator.publicKey,
        feeBps: 50,
        withdrawDelay: new anchor.BN(300),
        paused: false,
      })
      .accounts({
        bridge: ctx.bridgePda,
        admin: ctx.admin.publicKey,
      })
      .rpc();
  });

  describe("initialize", () => {
    it("bridge config is initialized correctly", async () => {
      const bridge = await ctx.program.account.bridgeConfig.fetch(ctx.bridgePda);
      expect(bridge.admin.toString()).to.equal(ctx.admin.publicKey.toString());
      expect(bridge.operator.toString()).to.equal(ctx.operator.publicKey.toString());
      expect(bridge.feeBps).to.equal(50);
      expect(bridge.withdrawDelay.toNumber()).to.equal(300);
      expect(bridge.paused).to.be.false;
    });

    it("rejects invalid fee bps", async () => {
      try {
        await ctx.program.methods
          .setConfig({
            newAdmin: null,
            operator: null,
            feeBps: 10001,
            withdrawDelay: null,
            paused: null,
          })
          .accounts({
            bridge: ctx.bridgePda,
            admin: ctx.admin.publicKey,
          })
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InvalidFeeBps");
      }
    });
  });

  describe("set_config", () => {
    it("admin can update operator", async () => {
      const newOperator = Keypair.generate();
      await ctx.program.methods
        .setConfig({
          newAdmin: null,
          operator: newOperator.publicKey,
          feeBps: null,
          withdrawDelay: null,
          paused: null,
        })
        .accounts({
          bridge: ctx.bridgePda,
          admin: ctx.admin.publicKey,
        })
        .rpc();

      const bridge = await ctx.program.account.bridgeConfig.fetch(ctx.bridgePda);
      expect(bridge.operator.toString()).to.equal(newOperator.publicKey.toString());

      // Reset operator back
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
    });

    it("non-admin cannot set config", async () => {
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
            admin: ctx.user.publicKey,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });

    it("admin can pause and unpause", async () => {
      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: null, withdrawDelay: null, paused: true })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      let bridge = await ctx.program.account.bridgeConfig.fetch(ctx.bridgePda);
      expect(bridge.paused).to.be.true;

      await ctx.program.methods
        .setConfig({ newAdmin: null, operator: null, feeBps: null, withdrawDelay: null, paused: false })
        .accounts({ bridge: ctx.bridgePda, admin: ctx.admin.publicKey })
        .rpc();

      bridge = await ctx.program.account.bridgeConfig.fetch(ctx.bridgePda);
      expect(bridge.paused).to.be.false;
    });
  });

  describe("register_chain", () => {
    it("registers a chain", async () => {
      const chainId = Buffer.from([0x00, 0x00, 0x00, 0x01]);
      const [chainPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("chain"), chainId],
        ctx.program.programId
      );

      const existing = await ctx.provider.connection.getAccountInfo(chainPda);
      if (!existing) {
        await ctx.program.methods
          .registerChain({ chainId: Array.from(chainId), identifier: "evm_1" })
          .accounts({
            bridge: ctx.bridgePda,
            chainEntry: chainPda,
            admin: ctx.admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
      }

      const chain = await ctx.program.account.chainEntry.fetch(chainPda);
      expect(chain.identifier).to.equal("evm_1");
    });
  });

  describe("register_token", () => {
    it("registers a token mapping", async () => {
      const destChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
      const destToken = Buffer.alloc(32);
      destToken[31] = 0x42;
      const localMint = Keypair.generate().publicKey;

      const [tokenPda] = findTokenPda(ctx.program.programId, destChain, destToken);

      const existing = await ctx.provider.connection.getAccountInfo(tokenPda);
      if (!existing) {
        await ctx.program.methods
          .registerToken({
            localMint,
            destChain: Array.from(destChain),
            destToken: Array.from(destToken),
            mode: { lockUnlock: {} },
            decimals: 9,
          })
          .accounts({
            bridge: ctx.bridgePda,
            tokenMapping: tokenPda,
            admin: ctx.admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
      }

      const mapping = await ctx.program.account.tokenMapping.fetch(tokenPda);
      expect(mapping.localMint.toString()).to.equal(localMint.toString());
      expect(mapping.decimals).to.equal(9);
    });

    it("non-admin cannot register token", async () => {
      const destChain = Buffer.from([0x00, 0x00, 0x00, 0x01]);
      const destToken = Buffer.alloc(32);
      destToken[31] = 0xFF;
      const localMint = Keypair.generate().publicKey;

      const [tokenPda] = findTokenPda(ctx.program.programId, destChain, destToken);

      try {
        await ctx.program.methods
          .registerToken({
            localMint,
            destChain: Array.from(destChain),
            destToken: Array.from(destToken),
            mode: { lockUnlock: {} },
            decimals: 9,
          })
          .accounts({
            bridge: ctx.bridgePda,
            tokenMapping: tokenPda,
            admin: ctx.user.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([ctx.user])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("UnauthorizedAdmin");
      }
    });
  });

  describe("add_canceler", () => {
    it("registers a canceler", async () => {
      const [cancelerPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("canceler"), ctx.canceler.publicKey.toBuffer()],
        ctx.program.programId
      );

      const existing = await ctx.provider.connection.getAccountInfo(cancelerPda);
      if (!existing) {
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

      const entry = await ctx.program.account.cancelerEntry.fetch(cancelerPda);
      expect(entry.active).to.be.true;
    });
  });
});
