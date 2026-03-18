/**
 * Initialize the Solana bridge program
 * Usage: npx ts-node scripts/solana/initialize-bridge.ts
 *
 * Env vars:
 *   SOLANA_RPC_URL - Solana RPC endpoint
 *   SOLANA_KEYPAIR_PATH - Path to admin keypair JSON
 *   SOLANA_PROGRAM_ID - Deployed program ID
 *   OPERATOR_PUBKEY - Operator public key
 *   FEE_BPS - Fee in basis points (default: 50 = 0.5%)
 *   WITHDRAW_DELAY - Withdrawal delay in seconds (default: 300 = 5 minutes)
 */

import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";

async function main() {
  const rpcUrl = process.env.SOLANA_RPC_URL || "http://localhost:8899";
  const keypairPath = process.env.SOLANA_KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`;
  const programIdStr = process.env.SOLANA_PROGRAM_ID;
  const operatorPubkey = process.env.OPERATOR_PUBKEY;
  const feeBps = parseInt(process.env.FEE_BPS || "50");
  const withdrawDelay = parseInt(process.env.WITHDRAW_DELAY || "300");

  if (!programIdStr) throw new Error("SOLANA_PROGRAM_ID is required");
  if (!operatorPubkey) throw new Error("OPERATOR_PUBKEY is required");

  const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
  const admin = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  const programId = new PublicKey(programIdStr);
  const operator = new PublicKey(operatorPubkey);

  const connection = new Connection(rpcUrl, "confirmed");

  const [bridgePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("bridge")],
    programId
  );

  console.log("Initializing CL8Y Bridge on Solana");
  console.log(`  Program: ${programId.toBase58()}`);
  console.log(`  Admin: ${admin.publicKey.toBase58()}`);
  console.log(`  Operator: ${operator.toBase58()}`);
  console.log(`  Fee: ${feeBps} bps`);
  console.log(`  Withdraw Delay: ${withdrawDelay}s`);
  console.log(`  Bridge PDA: ${bridgePda.toBase58()}`);

  // Check if already initialized
  const existing = await connection.getAccountInfo(bridgePda);
  if (existing) {
    console.log("\nBridge PDA already exists - skipping initialization");
    return;
  }

  console.log("\nSending initialize transaction...");

  // Build and send via Anchor
  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(admin),
    { commitment: "confirmed" }
  );
  anchor.setProvider(provider);

  // Note: In production, use the IDL from target/
  console.log("  (Use anchor CLI or IDL-based client for actual initialization)");
}

main().catch(console.error);
