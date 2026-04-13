import { describe, expect, it } from "vitest";
import {
  DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY,
  normalizeDecimalsBigInt,
  parseWithdrawRateLimitAccount,
  resolveSolanaEffectiveWithdrawLimits,
  SOLANA_RATE_LIMIT_WINDOW_SECS,
} from "./solanaWithdrawRateLimit";
import { anchorAccountDiscriminator } from "../../utils/anchorDiscriminator";

describe("normalizeDecimalsBigInt", () => {
  it("matches known 18→9 truncation", () => {
    const a = 1_000_000_000_000_000_000n;
    expect(normalizeDecimalsBigInt(a, 18, 9)).toBe(1_000_000_000n);
  });

  it("matches known 6→18 multiply", () => {
    const a = 1_000_000n;
    expect(normalizeDecimalsBigInt(a, 6, 18)).toBe(1_000_000_000_000_000_000n);
  });
});

describe("resolveSolanaEffectiveWithdrawLimits", () => {
  it("matches implicit EVM-style defaults for supply 1_000_000", () => {
    const r = resolveSolanaEffectiveWithdrawLimits(false, 0n, 0n, 0n, 1_000_000n);
    expect(r.min).toBe(1n);
    expect(r.maxTx).toBe(100n);
    expect(r.maxPeriod).toBe(100n);
  });

  it("uses zero-supply implicit floor", () => {
    const r = resolveSolanaEffectiveWithdrawLimits(false, 0n, 0n, 0n, 0n);
    expect(r.min).toBe(0n);
    expect(r.maxTx).toBe(0n);
    expect(r.maxPeriod).toBe(DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY);
  });

  it("respects explicit admin config", () => {
    const r = resolveSolanaEffectiveWithdrawLimits(true, 5n, 100n, 1000n, 999n);
    expect(r).toEqual({ min: 5n, maxTx: 100n, maxPeriod: 1000n });
  });
});

function writeU128LE(buf: Buffer, offset: number, val: bigint) {
  let v = val;
  for (let i = 0; i < 16; i++) {
    buf[offset + i] = Number(v & 0xffn);
    v >>= 8n;
  }
}

describe("parseWithdrawRateLimitAccount", () => {
  it("parses a minimal valid account buffer", () => {
    const disc = anchorAccountDiscriminator("WithdrawRateLimit");
    const body = Buffer.alloc(74);
    let o = 0;
    body[o] = 1; // explicit_config true
    o += 1;
    writeU128LE(body, o, 7n);
    o += 16;
    writeU128LE(body, o, 8n);
    o += 16;
    writeU128LE(body, o, 9n);
    o += 16;
    body.writeBigInt64LE(1000n, o);
    o += 8;
    writeU128LE(body, o, 3n);
    o += 16;
    body[o] = 2; // bump
    const raw = Buffer.concat([disc, body]);
    const p = parseWithdrawRateLimitAccount(raw);
    expect(p).not.toBeNull();
    expect(p!.explicitConfig).toBe(true);
    expect(p!.minPerTransaction).toBe(7n);
    expect(p!.maxPerTransaction).toBe(8n);
    expect(p!.maxPerPeriod).toBe(9n);
    expect(p!.windowStart).toBe(1000n);
    expect(p!.used).toBe(3n);
  });

  it("returns null for wrong discriminator", () => {
    const bad = Buffer.alloc(82, 1);
    expect(parseWithdrawRateLimitAccount(bad)).toBeNull();
  });
});

describe("SOLANA_RATE_LIMIT_WINDOW_SECS", () => {
  it("is 24h", () => {
    expect(SOLANA_RATE_LIMIT_WINDOW_SECS).toBe(86_400);
  });
});
