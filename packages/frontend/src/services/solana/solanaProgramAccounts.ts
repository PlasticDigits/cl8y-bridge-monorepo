/**
 * Scan cl8y_bridge program accounts for Settings UI (cancelers, token mappings).
 * Uses `getProgramAccounts` — some public RPCs disable it; callers should use
 * {@link withSolanaReadFallback} and/or a dedicated endpoint.
 */

import bs58 from "bs58";
import { Connection, PublicKey } from "@solana/web3.js";
import { anchorAccountDiscriminator } from "../../utils/anchorDiscriminator";

/** `TokenMapping` account: 8-byte disc + Borsh body (see on-chain `TokenMapping`). */
const TOKEN_MAPPING_DATA_LEN = 8 + 80;

/** `CancelerEntry` account: 8-byte disc + pubkey + active + bump. */
const CANCELER_ENTRY_DATA_LEN = 8 + 34;

function memcmpAccountDiscriminator(accountStructName: string) {
  const disc = anchorAccountDiscriminator(accountStructName);
  return {
    memcmp: { offset: 0, bytes: bs58.encode(disc) },
  };
}

/**
 * Active canceler pubkeys (`CancelerEntry.active == true`), sorted.
 */
export async function fetchSolanaBridgeCancelerPubkeys(
  connection: Connection,
  programId: PublicKey,
): Promise<string[]> {
  const accounts = await connection.getProgramAccounts(programId, {
    commitment: "confirmed",
    filters: [
      { dataSize: CANCELER_ENTRY_DATA_LEN },
      memcmpAccountDiscriminator("CancelerEntry"),
    ],
    encoding: "base64",
  });
  const out: string[] = [];
  for (const { account } of accounts) {
    const data = Buffer.from(account.data as Buffer);
    if (data.length < 8 + 32 + 1) continue;
    const active = data[8 + 32] !== 0;
    if (!active) continue;
    out.push(new PublicKey(data.subarray(8, 40)).toBase58());
  }
  return out.sort();
}

export interface SolanaTokenMappingRow {
  mappingPda: PublicKey;
  localMint: PublicKey;
  destChain: Uint8Array;
  destToken: Uint8Array;
  decimals: number;
  srcDecimals: number;
}

/** Parse on-chain `TokenMapping` account (with 8-byte Anchor discriminator). */
export function parseSolanaTokenMappingAccount(
  mappingPda: PublicKey,
  data: Buffer,
): SolanaTokenMappingRow | null {
  if (data.length < TOKEN_MAPPING_DATA_LEN) return null;
  const expectDisc = anchorAccountDiscriminator("TokenMapping");
  if (!data.subarray(0, 8).equals(expectDisc)) return null;
  const body = data.subarray(8);
  const localMint = new PublicKey(body.subarray(0, 32));
  const destChain = body.subarray(32, 36);
  const destToken = body.subarray(36, 68);
  const decimals = body[69] ?? 0;
  const srcDecimals = body[70] ?? 0;
  return {
    mappingPda,
    localMint,
    destChain,
    destToken,
    decimals,
    srcDecimals,
  };
}

/**
 * All `TokenMapping` accounts for the program.
 */
export async function fetchSolanaBridgeTokenMappingRows(
  connection: Connection,
  programId: PublicKey,
): Promise<SolanaTokenMappingRow[]> {
  const accounts = await connection.getProgramAccounts(programId, {
    commitment: "confirmed",
    filters: [
      { dataSize: TOKEN_MAPPING_DATA_LEN },
      memcmpAccountDiscriminator("TokenMapping"),
    ],
    encoding: "base64",
  });
  const out: SolanaTokenMappingRow[] = [];
  for (const { pubkey, account } of accounts) {
    const data = Buffer.from(account.data as Buffer);
    const row = parseSolanaTokenMappingAccount(pubkey, data);
    if (row) out.push(row);
  }
  return out;
}
