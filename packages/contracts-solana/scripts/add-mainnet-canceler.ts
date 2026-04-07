/**
 * Mainnet: register the production canceler pubkey on the Solana bridge (`add_canceler`).
 * Idempotent — skips if a CancelerEntry already exists for that pubkey and is active.
 *
 * Prerequisites: anchor build, bridge initialized, signer = bridge admin (same as register-mainnet-chains).
 *
 * Env:
 *   SOLANA_CANCELER_PUBKEY — canceler to register (default: production pubkey from runbook)
 *   ANCHOR_PROVIDER_URL / SOLANA_RPC_URL, SOLANA_PROGRAM_ID, ANCHOR_WALLET / SOLANA_KEYPAIR (see register-mainnet-chains.ts)
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import type { Cl8yBridge } from "../target/types/cl8y_bridge";
import { findBridgePda, findCancelerPda } from "../tests/helpers/setup";

/** docs/deployment-solana-mainnet.md Step 4.4 */
const DEFAULT_MAINNET_CANCELER =
  "EY7wuMnVByAcKW8BDT2KpieAwL5KatC8xJk4f7q3CurK";

async function main(): Promise<void> {
  const idlPath = path.join(__dirname, "../target/idl/cl8y_bridge.json");
  if (!fs.existsSync(idlPath)) {
    throw new Error(
      `Missing ${idlPath} — run anchor build in packages/contracts-solana`
    );
  }
  const idlRaw = JSON.parse(fs.readFileSync(idlPath, "utf8")) as anchor.Idl;
  const programIdStr = process.env.SOLANA_PROGRAM_ID?.trim();
  const idl: anchor.Idl = programIdStr
    ? { ...idlRaw, address: programIdStr }
    : idlRaw;

  const cancelerStr =
    process.env.SOLANA_CANCELER_PUBKEY?.trim() || DEFAULT_MAINNET_CANCELER;
  const cancelerPubkey = new PublicKey(cancelerStr);

  const rpc =
    process.env.ANCHOR_PROVIDER_URL ||
    process.env.SOLANA_RPC_URL ||
    "http://127.0.0.1:8899";
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
  const adminPk = kp.publicKey;

  const bridgeInfo = await connection.getAccountInfo(bridgePda);
  if (!bridgeInfo) {
    throw new Error(
      `Bridge PDA not initialized at ${bridgePda.toBase58()} — run initialize-bridge first`
    );
  }
  const bridge = await program.account.bridgeConfig.fetch(bridgePda);
  if (!bridge.admin.equals(adminPk)) {
    throw new Error(
      `Wallet ${adminPk.toBase58()} is not bridge admin (${bridge.admin.toBase58()})`
    );
  }

  const [cancelerEntryPda] = findCancelerPda(
    program.programId,
    cancelerPubkey
  );

  const existing = await connection.getAccountInfo(cancelerEntryPda);
  if (existing) {
    const entry = await program.account.cancelerEntry.fetch(cancelerEntryPda);
    if (entry.active && entry.pubkey.equals(cancelerPubkey)) {
      console.log(
        `[add-mainnet-canceler] skip (already active): ${cancelerPubkey.toBase58()} → ${cancelerEntryPda.toBase58()}`
      );
      return;
    }
    console.log(
      `[add-mainnet-canceler] updating existing entry (active=${entry.active})`
    );
  }

  await program.methods
    .addCanceler({ canceler: cancelerPubkey, active: true })
    .accounts({
      bridge: bridgePda,
      cancelerEntry: cancelerEntryPda,
      admin: adminPk,
      systemProgram: SystemProgram.programId,
    } as never)
    .rpc();

  console.log(
    `[add-mainnet-canceler] registered canceler ${cancelerPubkey.toBase58()} → ${cancelerEntryPda.toBase58()}`
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
