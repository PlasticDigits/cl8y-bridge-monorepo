/**
 * Read cl8y_bridge PendingWithdraw PDA by xchain hash (matches useAutoWithdrawSubmit polling).
 */

import { PublicKey } from "@solana/web3.js";
import type { Hex } from "viem";
import { anchorAccountDiscriminator } from "../../utils/anchorDiscriminator";
import type { BridgeChainConfig } from "../../types/chain";
import type { PendingWithdrawData } from "../../hooks/useTransferLookup";
import { getSolanaProgramIdString } from "./solanaBridgeAccounts";
import {
  solanaRpcUrlsForBridgeChain,
  withSolanaReadFallback,
} from "./solanaRpcUrls";

/** 8-byte Anchor disc + Borsh body (through `bump`); matches on-chain `PendingWithdraw` account size. */
const PENDING_WITHDRAW_MIN_LEN = 186;

function bytes4ToBytes32Left(b: `0x${string}`): Hex {
  const hex = b.slice(2).toLowerCase();
  return (`0x${hex.padEnd(64, "0")}`) as Hex;
}

function readU128LE(buf: Buffer, offset: number): bigint {
  let x = 0n;
  for (let i = 0; i < 16; i++) x |= BigInt(buf[offset + i]!) << BigInt(8 * i);
  return x;
}

function pubkeySubToBytes32Hex(slice: Buffer): Hex {
  return (`0x${slice.toString("hex")}`) as Hex;
}

/**
 * Parse on-chain `PendingWithdraw` account (8-byte Anchor discriminator + Borsh body).
 * Layout must match `packages/contracts-solana/.../pending_withdraw.rs`.
 */
export function parseSolanaPendingWithdrawAccount(
  raw: Uint8Array | Buffer,
  destChainBytes4: `0x${string}`,
  chainNumericId: number,
): PendingWithdrawData | null {
  const data = Buffer.isBuffer(raw) ? raw : Buffer.from(raw);
  const expectDisc = anchorAccountDiscriminator("PendingWithdraw");
  if (data.length < PENDING_WITHDRAW_MIN_LEN) return null;
  if (!data.subarray(0, 8).equals(expectDisc)) return null;

  const b = data.subarray(8);
  let o = 0;
  o += 32; // transfer_hash
  const srcChain4 = b.subarray(o, o + 4);
  o += 4;
  const srcAccount = b.subarray(o, o + 32);
  o += 32;
  const destAccountPk = b.subarray(o, o + 32);
  o += 32;
  const tokenPk = b.subarray(o, o + 32);
  o += 32;
  const amount = readU128LE(b, o);
  o += 16;
  const nonce = b.readBigUInt64LE(o);
  o += 8;
  const srcDecimals = b[o] ?? 0;
  o += 1;
  const destDecimals = b[o] ?? 0;
  o += 1;
  o += 8; // operator_gas
  const approved = (b[o] ?? 0) !== 0;
  o += 1;
  const approvedAt = b.readBigInt64LE(o);
  o += 8;
  const cancelled = (b[o] ?? 0) !== 0;
  o += 1;
  const executed = (b[o] ?? 0) !== 0;

  const srcChainHex = bytes4ToBytes32Left(
    (`0x${srcChain4.toString("hex")}`) as `0x${string}`,
  );
  const destChainHex = bytes4ToBytes32Left(destChainBytes4);

  return {
    chainId: chainNumericId,
    srcChain: srcChainHex,
    destChain: destChainHex,
    srcAccount: pubkeySubToBytes32Hex(srcAccount),
    destAccount: pubkeySubToBytes32Hex(destAccountPk),
    token: pubkeySubToBytes32Hex(tokenPk),
    amount,
    nonce,
    submittedAt: 1n,
    approvedAt: approvedAt >= 0n ? approvedAt : 0n,
    approved,
    cancelled,
    executed,
    srcDecimals,
    destDecimals,
  };
}

/**
 * Query Solana bridge PendingWithdraw PDA for `hash`, if the program is configured on `chain`.
 */
export async function querySolanaPendingWithdraw(
  chain: BridgeChainConfig,
  hash: Hex,
): Promise<PendingWithdrawData | null> {
  if (chain.type !== "solana") return null;
  const programIdStr = getSolanaProgramIdString(chain);
  const rpcUrls = solanaRpcUrlsForBridgeChain(chain);
  const bytes4 = chain.bytes4ChainId as `0x${string}` | undefined;
  if (!programIdStr || rpcUrls.length === 0 || !bytes4) return null;

  let chainNumericId = 0;
  try {
    chainNumericId = parseInt(bytes4.slice(2), 16);
  } catch {
    chainNumericId = 0;
  }

  const programId = new PublicKey(programIdStr);
  const hashBytes = new Uint8Array(32);
  const hexStr = hash.replace(/^0x/i, "");
  if (hexStr.length !== 64) return null;
  for (let i = 0; i < 32; i++) {
    hashBytes[i] = parseInt(hexStr.slice(i * 2, i * 2 + 2), 16);
  }
  const [pendingPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("withdraw"), Buffer.from(hashBytes)],
    programId,
  );

  try {
    const account = await withSolanaReadFallback(rpcUrls, (connection) =>
      connection.getAccountInfo(pendingPda),
    );
    if (!account?.data) return null;
    return parseSolanaPendingWithdrawAccount(account.data, bytes4, chainNumericId);
  } catch {
    return null;
  }
}
