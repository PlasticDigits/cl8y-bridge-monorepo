/**
 * Mainnet: set explicit withdraw rate limits per SPL mint (admin `set_rate_limit`).
 *
 * Use when implicit caps (mint supply / 10_000 per tx) block noneconomic test withdrawals.
 *
 * Prerequisites: `anchor build` (target/idl/cl8y_bridge.json). Signer must be bridge admin.
 *
 * Wallet: `ANCHOR_WALLET` or `SOLANA_KEYPAIR`; default `~/.config/solana/id-deployer.json`.
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import type { Cl8yBridge } from "../target/types/cl8y_bridge";
import { findBridgePda, findWithdrawRateLimitPda } from "../tests/helpers/setup";

const MINT_TESTA = new PublicKey(
  "6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E"
);
const MINT_TESTB = new PublicKey(
  "EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX"
);
const MINT_TDEC = new PublicKey(
  "765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR"
);

/** min, max/tx, max/24h in Solana raw units (generous noneconomic policy). */
const LIMITS: {
  mint: PublicKey;
  min: string;
  maxTx: string;
  maxPeriod: string;
}[] = [
  {
    mint: MINT_TESTA,
    min: "1000000000",
    maxTx: "1000000000000",
    maxPeriod: "5000000000000",
  },
  {
    mint: MINT_TESTB,
    min: "1000000000",
    maxTx: "1000000000000",
    maxPeriod: "5000000000000",
  },
  {
    mint: MINT_TDEC,
    min: "1000000",
    maxTx: "1000000000",
    maxPeriod: "5000000000",
  },
];

async function main(): Promise<void> {
  const idlPath = path.join(__dirname, "../target/idl/cl8y_bridge.json");
  if (!fs.existsSync(idlPath)) {
    throw new Error(
      `Missing ${idlPath} — run anchor build in packages/contracts-solana`
    );
  }
  const idlRaw = JSON.parse(fs.readFileSync(idlPath, "utf8")) as anchor.Idl;
  const programIdStr = process.env.SOLANA_PROGRAM_ID?.trim();
  if (!programIdStr) {
    throw new Error("Set SOLANA_PROGRAM_ID");
  }
  const idl: anchor.Idl = { ...idlRaw, address: programIdStr };

  const rpc = (
    process.env.ANCHOR_PROVIDER_URL ||
    process.env.SOLANA_RPC_URL ||
    "http://127.0.0.1:8899"
  )
    .split(",")[0]
    .trim();
  const walletPath =
    process.env.ANCHOR_WALLET ||
    process.env.SOLANA_KEYPAIR ||
    `${process.env.HOME}/.config/solana/id-deployer.json`;
  const kp = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf8")))
  );
  const wallet = new Wallet(kp);
  const connection = new Connection(rpc, "confirmed");
  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  const program = new Program(idl, provider) as Program<Cl8yBridge>;
  const [bridgePda] = findBridgePda(program.programId);

  const bridge = await program.account.bridgeConfig.fetch(bridgePda);
  if (!bridge.admin.equals(kp.publicKey)) {
    throw new Error(
      `Wallet ${kp.publicKey.toBase58()} is not bridge admin (${bridge.admin.toBase58()}).`
    );
  }

  for (const { mint, min, maxTx, maxPeriod } of LIMITS) {
    const [withdrawRateLimit] = findWithdrawRateLimitPda(program.programId, mint);
    const sig = await program.methods
      .setRateLimit({
        localMint: mint,
        minPerTransaction: new anchor.BN(min),
        maxPerTransaction: new anchor.BN(maxTx),
        maxPerPeriod: new anchor.BN(maxPeriod),
      })
      .accounts({
        bridge: bridgePda,
        withdrawRateLimit,
        admin: kp.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log(`[set-mainnet-withdraw-rate-limits] ${mint.toBase58()} → ${sig}`);
  }
  console.log("[set-mainnet-withdraw-rate-limits] done");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
