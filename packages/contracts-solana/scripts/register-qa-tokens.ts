/**
 * QA: register_token on Solana for each SPL mint × (Anvil, Terra, Anvil1) V2 chain IDs.
 * Reads token addresses from QA_TOKEN_JSON (written by register-tokens.ts).
 *
 * Prerequisites: `anchor build`, local validator, bridge initialized, admin = ANCHOR_WALLET.
 */

import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";
import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import type { Cl8yBridge } from "../target/types/cl8y_bridge";
import {
  findBridgePda,
  findChainPda,
  findTokenPda,
} from "../tests/helpers/setup";

/** Wrapped SOL — UI uses `deposit_native`; no bridge SPL vault required. */
const WSOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");

async function mintTokenProgramId(
  connection: Connection,
  mint: PublicKey
): Promise<PublicKey> {
  const info = await connection.getAccountInfo(mint);
  if (!info) throw new Error(`Mint account not found: ${mint.toBase58()}`);
  return info.owner;
}

/**
 * Lock/unlock SPL deposits need a bridge-owned ATA (vault). `register_token` only creates TokenMapping.
 */
async function ensureBridgeSplVault(
  connection: Connection,
  payer: Keypair,
  programId: PublicKey,
  mint: PublicKey
): Promise<void> {
  if (mint.equals(WSOL_MINT)) return;

  const [bridgePda] = findBridgePda(programId);
  const tokenProgram = await mintTokenProgramId(connection, mint);
  const ata = getAssociatedTokenAddressSync(
    mint,
    bridgePda,
    true,
    tokenProgram
  );
  const info = await connection.getAccountInfo(ata);
  if (info) return;

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
    `[register-qa-tokens] Created bridge SPL vault for mint ${mint.toBase58()} → ${ata.toBase58()}`
  );
}

const CHAIN_ANVIL = [0x00, 0x00, 0x00, 0x01] as const;
const CHAIN_TERRA = [0x00, 0x00, 0x00, 0x02] as const;
const CHAIN_ANVIL1 = [0x00, 0x00, 0x00, 0x03] as const;

const SPL_ABC = 9;
const SPL_LUNC = 6;
const SPL_KDEC = 9;
const SPL_SOL = 9;
const SPL_T2022 = 9;

interface QaTokens {
  anvil: {
    tokenA: string;
    tokenB: string;
    tokenC: string;
    lunc: string;
    kdec: string;
    sol: string;
    t2022: string;
  };
  anvil1: {
    tokenA: string;
    tokenB: string;
    tokenC: string;
    lunc: string;
    kdec: string;
    sol: string;
    t2022: string;
  };
  terra: {
    tokenA: string;
    tokenB: string;
    tokenC: string;
    kdec: string;
    sol: string;
    t2022: string;
  };
  solana: {
    tokenA: string;
    tokenB: string;
    tokenC: string;
    lunc: string;
    kdec: string;
    t2022: string;
    wsol: string;
  };
}

const keccakCache = new Map<string, string>();

function keccak256Utf8(s: string): string {
  const c = keccakCache.get(s);
  if (c) return c;
  const out = execSync(`cast keccak "${s.replace(/"/g, '\\"')}"`, {
    encoding: "utf8",
    env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: "1" },
  }).trim();
  keccakCache.set(s, out);
  return out;
}

function evmAddrToDestTokenBytes(addr: string): Buffer {
  const b = Buffer.alloc(32);
  const hex = addr.slice(2).toLowerCase();
  for (let i = 0; i < 20; i++) {
    b[12 + i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return b;
}

function keccakHexToBytes32(hex: string): Buffer {
  return Buffer.from(hex.replace(/^0x/, ""), "hex");
}

async function registerChainIfNeeded(
  program: Program<Cl8yBridge>,
  bridgePda: PublicKey,
  admin: PublicKey,
  chainId: number[],
  identifier: string
): Promise<void> {
  const [chainPda] = findChainPda(program.programId, Buffer.from(chainId));
  const info = await program.provider.connection.getAccountInfo(chainPda);
  if (info) return;
  await program.methods
    .registerChain({ chainId, identifier })
    .accounts({
      bridge: bridgePda,
      chainEntry: chainPda,
      admin,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
  console.log(
    `[register-qa-tokens] Registered chain ${identifier} (${chainId
      .map((x) => x.toString(16))
      .join(".")})`
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
  const info = await program.provider.connection.getAccountInfo(tokenPda);
  if (info) return;

  await program.methods
    .registerToken({
      localMint,
      destChain: Array.from(destChain),
      destToken: Array.from(destToken32),
      mode: { lockUnlock: {} },
      decimals,
      srcDecimals,
    })
    .accounts({
      bridge: bridgePda,
      tokenMapping: tokenPda,
      mint: localMint,
      admin,
      systemProgram: SystemProgram.programId,
    })
    .rpc();
}

async function main(): Promise<void> {
  const qaPath = process.env.QA_TOKEN_JSON;
  if (!qaPath) {
    throw new Error(
      "QA_TOKEN_JSON must point to qa-tokens.json from register-tokens"
    );
  }
  const raw = fs.readFileSync(qaPath, "utf8");
  const t = JSON.parse(raw) as QaTokens;

  const idlPath = path.join(__dirname, "../target/idl/cl8y_bridge.json");
  if (!fs.existsSync(idlPath)) {
    throw new Error(
      `Missing ${idlPath} — run anchor build in packages/contracts-solana`
    );
  }
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf8")) as anchor.Idl;

  const rpc =
    process.env.ANCHOR_PROVIDER_URL ||
    process.env.SOLANA_RPC_URL ||
    "http://127.0.0.1:8899";
  const walletPath =
    process.env.ANCHOR_WALLET || `${process.env.HOME}/.config/solana/id.json`;
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
  const admin = kp.publicKey;

  await registerChainIfNeeded(
    program,
    bridgePda,
    admin,
    [...CHAIN_ANVIL],
    "evm_31337"
  );
  await registerChainIfNeeded(
    program,
    bridgePda,
    admin,
    [...CHAIN_TERRA],
    "terra_localterra"
  );
  await registerChainIfNeeded(
    program,
    bridgePda,
    admin,
    [...CHAIN_ANVIL1],
    "evm_31338"
  );

  const destAnvil = Buffer.from(CHAIN_ANVIL);
  const destTerra = Buffer.from(CHAIN_TERRA);
  const destAnvil1 = Buffer.from(CHAIN_ANVIL1);

  type Row = {
    mint: PublicKey;
    dec: number;
    anvilTok: Buffer;
    terraTok: Buffer;
    anvil1Tok: Buffer;
    srcAnvil: number;
    srcTerra: number;
    srcAnvil1: number;
  };

  const rows: Row[] = [
    {
      mint: new PublicKey(t.solana.tokenA),
      dec: SPL_ABC,
      anvilTok: evmAddrToDestTokenBytes(t.anvil.tokenA),
      terraTok: keccakHexToBytes32(keccak256Utf8(t.terra.tokenA)),
      anvil1Tok: evmAddrToDestTokenBytes(t.anvil1.tokenA),
      srcAnvil: 18,
      srcTerra: 6,
      srcAnvil1: 18,
    },
    {
      mint: new PublicKey(t.solana.tokenB),
      dec: SPL_ABC,
      anvilTok: evmAddrToDestTokenBytes(t.anvil.tokenB),
      terraTok: keccakHexToBytes32(keccak256Utf8(t.terra.tokenB)),
      anvil1Tok: evmAddrToDestTokenBytes(t.anvil1.tokenB),
      srcAnvil: 18,
      srcTerra: 6,
      srcAnvil1: 18,
    },
    {
      mint: new PublicKey(t.solana.tokenC),
      dec: SPL_ABC,
      anvilTok: evmAddrToDestTokenBytes(t.anvil.tokenC),
      terraTok: keccakHexToBytes32(keccak256Utf8(t.terra.tokenC)),
      anvil1Tok: evmAddrToDestTokenBytes(t.anvil1.tokenC),
      srcAnvil: 18,
      srcTerra: 6,
      srcAnvil1: 18,
    },
    {
      mint: new PublicKey(t.solana.lunc),
      dec: SPL_LUNC,
      anvilTok: evmAddrToDestTokenBytes(t.anvil.lunc),
      terraTok: keccakHexToBytes32(keccak256Utf8("uluna")),
      anvil1Tok: evmAddrToDestTokenBytes(t.anvil1.lunc),
      srcAnvil: 6,
      srcTerra: 6,
      srcAnvil1: 6,
    },
    {
      mint: new PublicKey(t.solana.kdec),
      dec: SPL_KDEC,
      anvilTok: evmAddrToDestTokenBytes(t.anvil.kdec),
      terraTok: keccakHexToBytes32(keccak256Utf8(t.terra.kdec)),
      anvil1Tok: evmAddrToDestTokenBytes(t.anvil1.kdec),
      srcAnvil: 18,
      srcTerra: 6,
      srcAnvil1: 12,
    },
    {
      mint: new PublicKey(t.solana.wsol),
      dec: SPL_SOL,
      anvilTok: evmAddrToDestTokenBytes(t.anvil.sol),
      terraTok: keccakHexToBytes32(keccak256Utf8(t.terra.sol)),
      anvil1Tok: evmAddrToDestTokenBytes(t.anvil1.sol),
      srcAnvil: 9,
      srcTerra: 9,
      srcAnvil1: 9,
    },
  ];

  for (const r of rows) {
    await registerTokenIfNeeded(
      program,
      bridgePda,
      admin,
      r.mint,
      destAnvil,
      r.anvilTok,
      r.dec,
      r.srcAnvil
    );
    await registerTokenIfNeeded(
      program,
      bridgePda,
      admin,
      r.mint,
      destTerra,
      r.terraTok,
      r.dec,
      r.srcTerra
    );
    await registerTokenIfNeeded(
      program,
      bridgePda,
      admin,
      r.mint,
      destAnvil1,
      r.anvil1Tok,
      r.dec,
      r.srcAnvil1
    );
  }

  // Token-2022 SPL mint: register Anvil + Anvil1 always; Terra only if CW20 deployed (not placeholder).
  const t2022Mint = new PublicKey(t.solana.t2022);
  const t2022AnvilTok = evmAddrToDestTokenBytes(t.anvil.t2022);
  const t2022Anvil1Tok = evmAddrToDestTokenBytes(t.anvil1.t2022);
  const terraT2022Placeholder = t.terra.t2022.startsWith("terra1placeholder_");
  await registerTokenIfNeeded(
    program,
    bridgePda,
    admin,
    t2022Mint,
    destAnvil,
    t2022AnvilTok,
    SPL_T2022,
    18
  );
  if (!terraT2022Placeholder) {
    await registerTokenIfNeeded(
      program,
      bridgePda,
      admin,
      t2022Mint,
      destTerra,
      keccakHexToBytes32(keccak256Utf8(t.terra.t2022)),
      SPL_T2022,
      6
    );
  } else {
    console.log(
      "[register-qa-tokens] Skipping Solana T2022 ↔ Terra mapping (Terra CW20 placeholder)"
    );
  }
  await registerTokenIfNeeded(
    program,
    bridgePda,
    admin,
    t2022Mint,
    destAnvil1,
    t2022Anvil1Tok,
    SPL_T2022,
    18
  );

  const uniqueMints: PublicKey[] = [];
  const seenMint = new Set<string>();
  for (const r of rows) {
    const k = r.mint.toBase58();
    if (!seenMint.has(k)) {
      seenMint.add(k);
      uniqueMints.push(r.mint);
    }
  }
  {
    const k = t2022Mint.toBase58();
    if (!seenMint.has(k)) {
      seenMint.add(k);
      uniqueMints.push(t2022Mint);
    }
  }
  for (const mint of uniqueMints) {
    await ensureBridgeSplVault(connection, kp, program.programId, mint);
  }

  console.log(
    "[register-qa-tokens] Done (register_token matrix + bridge SPL vault ATAs for lock/unlock mints)."
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
