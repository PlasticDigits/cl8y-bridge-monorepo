/**
 * Read-only post-deploy check: Solana TokenMapping PDAs for Terra Classic `src_chain`
 * × `encode_token_address(CW20)`, plus Terra LCD `token_dest_mapping` → Solana.
 *
 * Run from repo root: `./scripts/verify-terra-solana-token-mappings.sh`
 * Or: `cd packages/contracts-solana && npx tsx scripts/verify-terra-solana-token-mappings.ts`
 *
 * Env: SOLANA_RPC_URL, TERRA_LCD, TERRA_BRIDGE, SOLANA_PROGRAM_ID, TERRA_TESTA/B/TDEC,
 *      TERRA_SRC_CHAIN_HEX8 (default 00000001 = columbus-5 V2 id)
 */

import { Connection, PublicKey } from "@solana/web3.js";
import { terraTokenIdToSrcTokenBytesStrict } from "../../frontend/src/services/terraTokenEncoding.ts";

const SOLANA_RPC =
  process.env.SOLANA_RPC_URL?.trim() || "https://solana-rpc.publicnode.com/";
const TERRA_LCD =
  process.env.TERRA_LCD?.trim() || "https://terra-classic-fcd.publicnode.com";
const TERRA_BRIDGE =
  process.env.TERRA_BRIDGE?.trim() ||
  "terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la";
const PROGRAM_ID = new PublicKey(
  process.env.SOLANA_PROGRAM_ID?.trim() ||
    "4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt",
);

function terraSrcChain4FromEnv(): Buffer {
  const h = (process.env.TERRA_SRC_CHAIN_HEX8?.trim() || "00000001").toLowerCase();
  if (!/^[0-9a-f]{8}$/.test(h)) {
    throw new Error("TERRA_SRC_CHAIN_HEX8 must be exactly 8 hex digits (big-endian bytes4)");
  }
  return Buffer.from([
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
    parseInt(h.slice(6, 8), 16),
  ]);
}

const CHAIN_TERRA = terraSrcChain4FromEnv();
const SOL_CHAIN_B64 = Buffer.from([0, 0, 0, 5]).toString("base64");
const TOKEN_SEED = Buffer.from("token");

const ROWS: { label: string; cw20: string }[] = [
  {
    label: "testa",
    cw20:
      process.env.TERRA_TESTA?.trim() ||
      "terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh",
  },
  {
    label: "testb",
    cw20:
      process.env.TERRA_TESTB?.trim() ||
      "terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3",
  },
  {
    label: "tdec",
    cw20:
      process.env.TERRA_TDEC?.trim() ||
      "terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv",
  },
];

async function lcdTokenDestMapping(
  localToken: string,
): Promise<{ ok: boolean; detail: string }> {
  const q = JSON.stringify({
    token_dest_mapping: { token: localToken, dest_chain: SOL_CHAIN_B64 },
  });
  const b64 = Buffer.from(q, "utf8").toString("base64");
  const url = `${TERRA_LCD.replace(/\/$/, "")}/cosmwasm/wasm/v1/contract/${TERRA_BRIDGE}/smart/${b64}`;
  const res = await fetch(url, { headers: { "User-Agent": "cl8y-verify-terra-solana/1" } });
  if (!res.ok) {
    return { ok: false, detail: `HTTP ${res.status}` };
  }
  const j = (await res.json()) as { data?: { dest_token?: string } };
  const dt = j.data?.dest_token;
  if (!dt) return { ok: false, detail: "no data.dest_token" };
  return { ok: true, detail: dt };
}

function findTokenPda(destToken32: Uint8Array): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [TOKEN_SEED, CHAIN_TERRA, Buffer.from(destToken32)],
    PROGRAM_ID,
  );
  return pda;
}

async function main(): Promise<void> {
  const conn = new Connection(SOLANA_RPC, "confirmed");
  let failed = false;

  console.log("Solana RPC:", SOLANA_RPC);
  console.log("Terra LCD:", TERRA_LCD);
  console.log("Terra bridge:", TERRA_BRIDGE);
  console.log("Solana program:", PROGRAM_ID.toBase58());
  console.log("Terra src_chain bytes4 (hex):", CHAIN_TERRA.toString("hex"));
  console.log("");

  for (const r of ROWS) {
    const key32 = Buffer.from(terraTokenIdToSrcTokenBytesStrict(r.cw20));
    const pda = findTokenPda(key32);
    const info = await conn.getAccountInfo(pda);
    const solOk = !!(info && info.data.length > 0);

    const lcd = await lcdTokenDestMapping(r.cw20);
    const lcdNote =
      lcd.detail.length > 120 ? `${lcd.detail.slice(0, 120)}…` : lcd.detail;
    const line = `[${r.label}] cw20=${r.cw20.slice(0, 24)}…\n  encode_token_address hex: 0x${key32.toString("hex")}\n  TokenMapping PDA: ${pda.toBase58()} → ${solOk ? "OK (account exists)" : "FAIL (missing)"}\n  Terra token_dest_mapping→Solana: ${lcd.ok ? "OK" : "FAIL"} (${lcdNote})`;
    console.log(line);
    console.log("");
    if (!solOk || !lcd.ok) failed = true;
  }

  if (failed) {
    console.error(
      "One or more checks failed. Register Solana mappings: npx tsx scripts/register-mainnet-tokens.ts",
    );
    process.exit(1);
  }
  console.log("All checks passed.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
