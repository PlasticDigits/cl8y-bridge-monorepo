/**
 * Mainnet: register BSC, opBNB, Terra Classic, and MegaETH as peer chains on the Solana bridge.
 * Idempotent — skips each chain if its ChainEntry PDA already exists.
 *
 * Bytes4 IDs and identifiers must match EVM ChainRegistry / Terra bridge (see deploy-evm-full.sh,
 * docs/deployment-solana-mainnet.md §2.4).
 *
 * Prerequisites: `anchor build` (target/idl/cl8y_bridge.json). Signer must be bridge admin.
 *
 * Wallet: `ANCHOR_WALLET` or `SOLANA_KEYPAIR`; if unset, defaults to `~/.config/solana/id-deployer.json`
 * (this rollout’s bridge admin / deployer — same as initialize).
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import type { Cl8yBridge } from "../target/types/cl8y_bridge";
import { findBridgePda, findChainPda } from "../tests/helpers/setup";

/** Matches scripts/deploy-evm-full.sh BRIDGE_*_CHAIN_ID and live registries. */
const MAINNET_PEER_CHAINS: readonly {
  chainId: readonly [number, number, number, number];
  identifier: string;
}[] = [
  { chainId: [0, 0, 0, 0x38], identifier: "evm_56" },
  { chainId: [0, 0, 0, 0xcc], identifier: "evm_204" },
  { chainId: [0, 0, 0, 0x01], identifier: "terraclassic_columbus-5" },
  /** MegaETH mainnet — bytes4(uint32(4326)) = 0x000010e6 (see scripts/megaeth/compute-megaeth-constants.sh). */
  { chainId: [0, 0, 0x10, 0xe6], identifier: "evm_4326" },
];

async function registerChainIfNeeded(
  program: Program<Cl8yBridge>,
  admin: PublicKey,
  chainId: number[],
  identifier: string
): Promise<void> {
  const [chainPda] = findChainPda(program.programId, Buffer.from(chainId));
  const info = await program.provider.connection.getAccountInfo(chainPda);
  if (info) {
    console.log(
      `[register-mainnet-chains] skip (exists): ${identifier} → ${chainPda.toBase58()}`
    );
    return;
  }
  await program.methods
    .registerChain({ chainId, identifier })
    .accounts({
      admin,
    })
    .rpc();
  console.log(
    `[register-mainnet-chains] registered ${identifier} chainId=[${chainId
      .map((b) => "0x" + b.toString(16))
      .join(",")}] → ${chainPda.toBase58()}`
  );
}

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
      `Wallet ${adminPk.toBase58()} is not bridge admin (${bridge.admin.toBase58()}). Use the same key as initialize.`
    );
  }

  for (const { chainId, identifier } of MAINNET_PEER_CHAINS) {
    await registerChainIfNeeded(program, adminPk, [...chainId], identifier);
  }
  console.log("[register-mainnet-chains] done");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
