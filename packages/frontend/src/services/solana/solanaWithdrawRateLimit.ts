/**
 * Solana `WithdrawRateLimit` PDA reads + implicit defaults (matches `programs/cl8y-bridge/src/rate_limit.rs`).
 */

import { Connection, PublicKey } from "@solana/web3.js";
import { getMint } from "@solana/spl-token";
import { anchorAccountDiscriminator } from "../../utils/anchorDiscriminator";
import { findWithdrawRateLimitPda } from "./transaction";

/** 24h window (seconds) — matches `RATE_LIMIT_WINDOW_SECS` on-chain. */
export const SOLANA_RATE_LIMIT_WINDOW_SECS = 86_400;

const DEFAULT_MIN_DIVISOR = 1_000_000n;
const DEFAULT_MAX_DIVISOR = 10_000n;
/** Matches `DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY` (native / zero-supply implicit path). */
export const DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY = 100_000_000_000_000_000_000n;

const WITHDRAW_RATE_LIMIT_DISC = anchorAccountDiscriminator("WithdrawRateLimit");
/** 8 disc + Borsh body through `bump` for `WithdrawRateLimit`. */
const WITHDRAW_RATE_LIMIT_MIN_LEN = 82;

function readU128LE(buf: Buffer, offset: number): bigint {
  let x = 0n;
  for (let i = 0; i < 16; i++) x |= BigInt(buf[offset + i]!) << BigInt(8 * i);
  return x;
}

/** Payout amount after decimal normalization (matches on-chain `normalize_decimals`). */
export function normalizeDecimalsBigInt(
  amount: bigint,
  srcDecimals: number,
  destDecimals: number,
): bigint {
  if (srcDecimals === destDecimals) return amount;
  if (srcDecimals > destDecimals) {
    const exp = srcDecimals - destDecimals;
    return amount / 10n ** BigInt(exp);
  }
  const exp = destDecimals - srcDecimals;
  return amount * 10n ** BigInt(exp);
}

export function resolveSolanaEffectiveWithdrawLimits(
  explicitConfig: boolean,
  minStored: bigint,
  maxTxStored: bigint,
  maxPeriodStored: bigint,
  mintSupply: bigint,
): { min: bigint; maxTx: bigint; maxPeriod: bigint } {
  if (explicitConfig) {
    return {
      min: minStored,
      maxTx: maxTxStored,
      maxPeriod: maxPeriodStored,
    };
  }
  if (mintSupply === 0n) {
    return { min: 0n, maxTx: 0n, maxPeriod: DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY };
  }
  const maxTx = mintSupply / DEFAULT_MAX_DIVISOR;
  const min = mintSupply / DEFAULT_MIN_DIVISOR;
  return { min, maxTx, maxPeriod: maxTx };
}

export interface ParsedWithdrawRateLimitAccount {
  explicitConfig: boolean;
  minPerTransaction: bigint;
  maxPerTransaction: bigint;
  maxPerPeriod: bigint;
  windowStart: bigint;
  used: bigint;
}

export function parseWithdrawRateLimitAccount(
  raw: Uint8Array | Buffer,
): ParsedWithdrawRateLimitAccount | null {
  const data = Buffer.isBuffer(raw) ? raw : Buffer.from(raw);
  if (data.length < WITHDRAW_RATE_LIMIT_MIN_LEN) return null;
  if (!data.subarray(0, 8).equals(WITHDRAW_RATE_LIMIT_DISC)) return null;
  const b = data.subarray(8);
  let o = 0;
  const explicitConfig = (b[o] ?? 0) !== 0;
  o += 1;
  const minPerTransaction = readU128LE(b, o);
  o += 16;
  const maxPerTransaction = readU128LE(b, o);
  o += 16;
  const maxPerPeriod = readU128LE(b, o);
  o += 16;
  const windowStart = b.readBigInt64LE(o);
  o += 8;
  const used = readU128LE(b, o);
  return {
    explicitConfig,
    minPerTransaction,
    maxPerTransaction,
    maxPerPeriod,
    windowStart,
    used,
  };
}

export interface SolanaWithdrawRateLimitSnapshot {
  explicitConfig: boolean;
  mintSupply: bigint;
  effectiveMin: bigint;
  effectiveMaxTx: bigint;
  effectiveMaxPeriod: bigint;
  windowStart: bigint;
  used: bigint;
}

/**
 * Read withdraw-rate-limit state for a local mint (SPL) or {@link SOLANA_NATIVE_TOKEN_PUBKEY} (native SOL mapping).
 */
export async function fetchSolanaWithdrawRateLimitSnapshot(
  connection: Connection,
  programId: PublicKey,
  localMint: PublicKey,
  isNativeMapping: boolean,
): Promise<SolanaWithdrawRateLimitSnapshot> {
  const [wrPda] = findWithdrawRateLimitPda(programId, localMint);
  const acc = await connection.getAccountInfo(wrPda, "confirmed");
  let parsed: ParsedWithdrawRateLimitAccount | null = null;
  if (acc?.data) {
    parsed = parseWithdrawRateLimitAccount(acc.data);
  }

  let mintSupply = 0n;
  if (!isNativeMapping) {
    const mintInfo = await connection.getAccountInfo(localMint, "confirmed");
    if (mintInfo?.data) {
      const owner = mintInfo.owner;
      const m = await getMint(connection, localMint, "confirmed", owner);
      mintSupply = BigInt(m.supply.toString());
    }
  }

  const explicitConfig = parsed?.explicitConfig ?? false;
  const minS = parsed?.minPerTransaction ?? 0n;
  const maxTxS = parsed?.maxPerTransaction ?? 0n;
  const maxPS = parsed?.maxPerPeriod ?? 0n;
  const { min, maxTx, maxPeriod } = resolveSolanaEffectiveWithdrawLimits(
    explicitConfig,
    minS,
    maxTxS,
    maxPS,
    isNativeMapping ? 0n : mintSupply,
  );

  return {
    explicitConfig,
    mintSupply,
    effectiveMin: min,
    effectiveMaxTx: maxTx,
    effectiveMaxPeriod: maxPeriod,
    windowStart: parsed?.windowStart ?? 0n,
    used: parsed?.used ?? 0n,
  };
}
