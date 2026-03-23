import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, createMint, createAssociatedTokenAccount, getAccount } from "@solana/spl-token";
import { expect } from "chai";
import { Cl8yFaucet } from "../target/types/cl8y_faucet";
import { airdrop } from "./helpers/setup";

const FAUCET_SEED = Buffer.from("faucet");
const CLAIM_SEED = Buffer.from("claim");

function findFaucetConfigPda(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([FAUCET_SEED], programId);
}

function findClaimRecordPda(
  programId: PublicKey,
  claimer: PublicKey,
  mintOrTag: PublicKey | Buffer,
): [PublicKey, number] {
  const key = mintOrTag instanceof PublicKey ? mintOrTag.toBuffer() : mintOrTag;
  return PublicKey.findProgramAddressSync([CLAIM_SEED, claimer.toBuffer(), key], programId);
}

describe("cl8y-faucet", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Cl8yFaucet as Program<Cl8yFaucet>;

  const admin = (provider.wallet as anchor.Wallet).payer;
  const claimer = Keypair.generate();

  const [faucetConfigPda] = findFaucetConfigPda(program.programId);

  const CLAIM_AMOUNT = 1_000_000_000; // 1 token (9 decimals)
  const COOLDOWN_SECONDS = 2;

  before(async () => {
    await airdrop(provider.connection, claimer.publicKey);
  });

  describe("initialize", () => {
    it("creates faucet config with correct fields", async () => {
      await program.methods
        .initialize({
          claimAmount: new anchor.BN(CLAIM_AMOUNT),
          cooldownSeconds: new anchor.BN(COOLDOWN_SECONDS),
        })
        .accounts({
          faucetConfig: faucetConfigPda,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const config = await program.account.faucetConfig.fetch(faucetConfigPda);
      expect(config.admin.toString()).to.equal(admin.publicKey.toString());
      expect(config.claimAmount.toNumber()).to.equal(CLAIM_AMOUNT);
      expect(config.cooldownSeconds.toNumber()).to.equal(COOLDOWN_SECONDS);
      expect(config.bump).to.be.greaterThan(0);
    });

    it("rejects re-initialization", async () => {
      try {
        await program.methods
          .initialize({
            claimAmount: new anchor.BN(999),
            cooldownSeconds: new anchor.BN(1),
          })
          .accounts({
            faucetConfig: faucetConfigPda,
            admin: admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.satisfy(
          (s: string) => s.includes("already in use") || s.includes("0x0"),
        );
      }
    });
  });

  describe("register_mint + claim (SPL)", () => {
    let mint: PublicKey;
    let claimerAta: PublicKey;

    before(async () => {
      mint = await createMint(
        provider.connection,
        admin,
        admin.publicKey, // initial mint authority = admin
        null,
        9,
      );
      claimerAta = await createAssociatedTokenAccount(
        provider.connection,
        admin,
        mint,
        claimer.publicKey,
      );
    });

    it("transfers mint authority to faucet PDA", async () => {
      await program.methods
        .registerMint()
        .accounts({
          faucetConfig: faucetConfigPda,
          mint,
          admin: admin.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
    });

    it("non-admin cannot register a mint", async () => {
      const rogue = Keypair.generate();
      await airdrop(provider.connection, rogue.publicKey, LAMPORTS_PER_SOL);

      const rogueMint = await createMint(
        provider.connection,
        rogue,
        rogue.publicKey,
        null,
        9,
      );

      try {
        await program.methods
          .registerMint()
          .accounts({
            faucetConfig: faucetConfigPda,
            mint: rogueMint,
            admin: rogue.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([rogue])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("Unauthorized");
      }
    });

    it("claims SPL tokens", async () => {
      const [claimRecord] = findClaimRecordPda(program.programId, claimer.publicKey, mint);

      await program.methods
        .claim()
        .accounts({
          faucetConfig: faucetConfigPda,
          claimRecord,
          mint,
          claimerTokenAccount: claimerAta,
          claimer: claimer.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([claimer])
        .rpc();

      const ata = await getAccount(provider.connection, claimerAta);
      expect(Number(ata.amount)).to.equal(CLAIM_AMOUNT);
    });

    it("rejects claim before cooldown elapses", async () => {
      const [claimRecord] = findClaimRecordPda(program.programId, claimer.publicKey, mint);

      try {
        await program.methods
          .claim()
          .accounts({
            faucetConfig: faucetConfigPda,
            claimRecord,
            mint,
            claimerTokenAccount: claimerAta,
            claimer: claimer.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([claimer])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("CooldownNotElapsed");
      }
    });

    it("allows claim after cooldown", async () => {
      await new Promise((r) => setTimeout(r, (COOLDOWN_SECONDS + 1) * 1000));

      const [claimRecord] = findClaimRecordPda(program.programId, claimer.publicKey, mint);

      await program.methods
        .claim()
        .accounts({
          faucetConfig: faucetConfigPda,
          claimRecord,
          mint,
          claimerTokenAccount: claimerAta,
          claimer: claimer.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([claimer])
        .rpc();

      const ata = await getAccount(provider.connection, claimerAta);
      expect(Number(ata.amount)).to.equal(CLAIM_AMOUNT * 2);
    });
  });

  describe("claim_sol", () => {
    const SOL_CLAIM_LAMPORTS = LAMPORTS_PER_SOL / 10; // 0.1 SOL
    const NATIVE_SOL_TAG = Buffer.from("native_sol");

    before(async () => {
      // Fund the faucet PDA with SOL so it can dispense
      const tx = new anchor.web3.Transaction().add(
        SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: faucetConfigPda,
          lamports: 2 * LAMPORTS_PER_SOL,
        }),
      );
      await provider.sendAndConfirm(tx);
    });

    it("claims SOL from faucet", async () => {
      const [claimRecord] = findClaimRecordPda(program.programId, claimer.publicKey, NATIVE_SOL_TAG);

      const balanceBefore = await provider.connection.getBalance(claimer.publicKey);

      await program.methods
        .claimSol(new anchor.BN(SOL_CLAIM_LAMPORTS))
        .accounts({
          faucetConfig: faucetConfigPda,
          claimRecord,
          claimer: claimer.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([claimer])
        .rpc();

      const balanceAfter = await provider.connection.getBalance(claimer.publicKey);
      // Balance increase ≈ SOL_CLAIM_LAMPORTS minus tx fee; at minimum it should go up
      expect(balanceAfter).to.be.greaterThan(balanceBefore);
    });

    it("rejects SOL claim before cooldown", async () => {
      const [claimRecord] = findClaimRecordPda(program.programId, claimer.publicKey, NATIVE_SOL_TAG);

      try {
        await program.methods
          .claimSol(new anchor.BN(SOL_CLAIM_LAMPORTS))
          .accounts({
            faucetConfig: faucetConfigPda,
            claimRecord,
            claimer: claimer.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([claimer])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("CooldownNotElapsed");
      }
    });

    it("rejects SOL claim exceeding available balance", async () => {
      await new Promise((r) => setTimeout(r, (COOLDOWN_SECONDS + 1) * 1000));

      const [claimRecord] = findClaimRecordPda(program.programId, claimer.publicKey, NATIVE_SOL_TAG);
      const hugeAmount = 999 * LAMPORTS_PER_SOL;

      try {
        await program.methods
          .claimSol(new anchor.BN(hugeAmount))
          .accounts({
            faucetConfig: faucetConfigPda,
            claimRecord,
            claimer: claimer.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([claimer])
          .rpc();
        expect.fail("Should have thrown");
      } catch (err) {
        expect(err.toString()).to.contain("InsufficientSol");
      }
    });
  });
});
