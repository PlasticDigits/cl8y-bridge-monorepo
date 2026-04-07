/**
 * Mainnet (noneconomic test tokens): register_token for each SPL mint × (BSC, opBNB, Terra).
 * Mode: MintBurn — mint authority on each SPL mint must be the BridgeConfig PDA (see runbook).
 *
 * Prerequisites:
 *   - anchor build (target/idl/cl8y_bridge.json)
 *   - Bridge initialized; signer = bridge admin (same as register-mainnet-chains.ts)
 *   - Peer chains registered on Solana (Step 2.4): evm_56, evm_204, terraclassic_columbus-5
 *   - Creates bridge fee-collection ATAs for each mint (MintBurn still uses bridge_token_account for fees)
 *
 * Addresses match docs/deployment-solana-mainnet.md token matrix (live testa / testb / tdec).
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";
import type { Cl8yBridge } from "../target/types/cl8y_bridge";
import { findBridgePda, findTokenPda } from "../tests/helpers/setup";
import { terraDestTokenKeccakUtf8Bytes } from "../../frontend/src/services/terraTokenEncoding";

const CHAIN_BSC = Buffer.from([0, 0, 0, 0x38]);
const CHAIN_OPBNB = Buffer.from([0, 0, 0, 0xcc]);
const CHAIN_TERRA = Buffer.from([0, 0, 0, 0x01]);

/** Live mainnet noneconomic SPL mints */
const MINT_TESTA = new PublicKey(
  "6XjWBbRJW5uhd8csCiDivXGPF42yYoyDARtxEtX3oP7E"
);
const MINT_TESTB = new PublicKey(
  "EvAWhkKQzX8om5VDWjg8oEvCw9jhGGKsn3rdrNXmQScX"
);
const MINT_TDEC = new PublicKey(
  "765GMcrKxfevfBhnJmZDhdyHDon2nTwGemcgqJApNBR"
);

function evmAddrToDestToken32(addr: string): Buffer {
  const b = Buffer.alloc(32);
  const hex = addr.slice(2).toLowerCase();
  for (let i = 0; i < 20; i++) {
    b[12 + i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return b;
}

function terraCw20ToDestToken32(bech32: string): Buffer {
  return Buffer.from(terraDestTokenKeccakUtf8Bytes(bech32));
}

async function mintTokenProgramId(
  connection: Connection,
  mint: PublicKey
): Promise<PublicKey> {
  const info = await connection.getAccountInfo(mint);
  if (!info) throw new Error(`Mint account not found: ${mint.toBase58()}`);
  return info.owner;
}

async function ensureBridgeSplVault(
  connection: Connection,
  payer: Keypair,
  programId: PublicKey,
  mint: PublicKey
): Promise<void> {
  const [bridgePda] = findBridgePda(programId);
  const tokenProgram = await mintTokenProgramId(connection, mint);
  const ata = getAssociatedTokenAddressSync(
    mint,
    bridgePda,
    true,
    tokenProgram
  );
  if (await connection.getAccountInfo(ata)) return;

  await getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    mint,
    bridgePda,
    true,
    "confirmed",
    undefined,
    tokenProgram
  );
  console.log(
    `[register-mainnet-tokens] Created bridge ATA for fees: ${mint.toBase58()} → ${ata.toBase58()}`
  );
}

async function registerTokenIfNeeded(
  program: Program<Cl8yBridge>,
  bridgePda: PublicKey,
  admin: PublicKey,
  localMint: PublicKey,
  destChain: Buffer,
  destToken32: Buffer,
  decimals: number,
  srcDecimals: number
): Promise<void> {
  const [tokenPda] = findTokenPda(program.programId, destChain, destToken32);
  if (await program.provider.connection.getAccountInfo(tokenPda)) {
    console.log(
      `[register-mainnet-tokens] skip exists ${localMint.toBase58().slice(0, 8)}… chain=${destChain.toString("hex")}`
    );
    return;
  }

  await program.methods
    .registerToken({
      localMint,
      destChain: Array.from(destChain),
      destToken: Array.from(destToken32),
      mode: { mintBurn: {} },
      decimals,
      srcDecimals,
    })
    .accounts({
      bridge: bridgePda,
      tokenMapping: tokenPda,
      mint: localMint,
      admin,
      systemProgram: SystemProgram.programId,
      // Anchor 0.32 strict ResolvedAccounts omits auto-filled PDAs; runtime still needs these.
    } as never)
    .rpc();

  console.log(
    `[register-mainnet-tokens] registered ${localMint.toBase58()} → chain 0x${destChain.toString("hex")}`
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

  // BSC / opBNB ERC20 (left-padded address); Terra CW20 (encode_token_address bytes)
  const bscTesta = evmAddrToDestToken32(
    "0x3557bfd147b35C2647EAFC05c8BE757ce84D5B1c"
  );
  const bscTestb = evmAddrToDestToken32(
    "0x39c4a8d50Cdd20131eC91B3ACcc6352123F68B52"
  );
  const bscTdec = evmAddrToDestToken32(
    "0xe159c7a58d694fafba82221905d5a49e7f314330"
  );

  const opbnbTesta = evmAddrToDestToken32(
    "0xF073d5685594F465a66EA54516f0D2f76b6cc6F3"
  );
  const opbnbTestb = evmAddrToDestToken32(
    "0xe1EaAC9be88D5fb89C944B46Bdc48fad2d47185e"
  );
  const opbnbTdec = evmAddrToDestToken32(
    "0x6d66d16e6cb29351aee1960ba1c395c0fb1392dd"
  );

  const terraTesta = terraCw20ToDestToken32(
    "terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh"
  );
  const terraTestb = terraCw20ToDestToken32(
    "terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3"
  );
  const terraTdec = terraCw20ToDestToken32(
    "terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv"
  );

  type Row = {
    mint: PublicKey;
    splDecimals: number;
    bsc: Buffer;
    opbnb: Buffer;
    terra: Buffer;
    srcBsc: number;
    srcOpbnb: number;
    srcTerra: number;
  };

  const rows: Row[] = [
    {
      mint: MINT_TESTA,
      splDecimals: 9,
      bsc: bscTesta,
      opbnb: opbnbTesta,
      terra: terraTesta,
      srcBsc: 18,
      srcOpbnb: 18,
      srcTerra: 18,
    },
    {
      mint: MINT_TESTB,
      splDecimals: 9,
      bsc: bscTestb,
      opbnb: opbnbTestb,
      terra: terraTestb,
      srcBsc: 18,
      srcOpbnb: 18,
      srcTerra: 18,
    },
    {
      mint: MINT_TDEC,
      splDecimals: 6,
      bsc: bscTdec,
      opbnb: opbnbTdec,
      terra: terraTdec,
      srcBsc: 18,
      srcOpbnb: 12,
      srcTerra: 6,
    },
  ];

  for (const r of rows) {
    await ensureBridgeSplVault(connection, kp, program.programId, r.mint);
  }

  for (const r of rows) {
    await registerTokenIfNeeded(
      program,
      bridgePda,
      adminPk,
      r.mint,
      CHAIN_BSC,
      r.bsc,
      r.splDecimals,
      r.srcBsc
    );
    await registerTokenIfNeeded(
      program,
      bridgePda,
      adminPk,
      r.mint,
      CHAIN_OPBNB,
      r.opbnb,
      r.splDecimals,
      r.srcOpbnb
    );
    await registerTokenIfNeeded(
      program,
      bridgePda,
      adminPk,
      r.mint,
      CHAIN_TERRA,
      r.terra,
      r.splDecimals,
      r.srcTerra
    );
  }

  console.log("[register-mainnet-tokens] done");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
