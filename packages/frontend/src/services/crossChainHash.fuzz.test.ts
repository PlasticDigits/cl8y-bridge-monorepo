/**
 * Property / fuzz checks: V2 xchain hash (EVM / Terra / Solana) and Solana withdraw helpers.
 * Manual 224-byte layout + keccak must match viem `abi.encode` path (INV-HFE1).
 */
import { describe, expect, it } from "vitest";
import { keccak_256 } from "@noble/hashes/sha3";
import { computeXchainHashIdBytes } from "./hashVerification";
import { computeTransferHash } from "./solana/transaction";
import { withdrawSubmitSrcAccountBytes32 } from "./solana/srcAccountBytes32";
import { resolveWithdrawSrcTokenBytesForSolana } from "./solana/resolveWithdrawSrcTokenBytes";

/** Deterministic PRNG (Mulberry32) for reproducible fuzz runs. */
function mulberry32(seed: number): () => number {
  return function () {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function randBytes(rng: () => number, len: number): Uint8Array {
  const out = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    out[i] = Math.floor(rng() * 256);
  }
  return out;
}

function randU128(rng: () => number): bigint {
  let x = 0n;
  for (let i = 0; i < 16; i++) {
    x = (x << 8n) | BigInt(Math.floor(rng() * 256));
  }
  return x;
}

function randU64(rng: () => number): bigint {
  let x = 0n;
  for (let i = 0; i < 8; i++) {
    x = (x << 8n) | BigInt(Math.floor(rng() * 256));
  }
  return x;
}

/**
 * Same 224-byte preimage as `multichain-rs::compute_xchain_hash_id` / Solana `compute_transfer_hash`.
 */
function computeXchainHashManual224(
  srcChain: Uint8Array,
  destChain: Uint8Array,
  srcAccount: Uint8Array,
  destAccount: Uint8Array,
  token: Uint8Array,
  amount: bigint,
  nonce: bigint,
): Uint8Array {
  const buf = new Uint8Array(224);
  buf.set(srcChain.subarray(0, 4), 0);
  buf.set(destChain.subarray(0, 4), 32);
  buf.set(srcAccount.subarray(0, 32), 64);
  buf.set(destAccount.subarray(0, 32), 96);
  buf.set(token.subarray(0, 32), 128);

  let a = amount;
  for (let i = 0; i < 16; i++) {
    buf[191 - i] = Number(a & 0xffn);
    a >>= 8n;
  }

  let n = nonce;
  for (let i = 0; i < 8; i++) {
    buf[223 - i] = Number(n & 0xffn);
    n >>= 8n;
  }

  return keccak_256(buf);
}

/** Big-endian u32 → bytes4 (matches `chainIdToBytes4` / V2 PDA chain slices). */
function u32ToBytes4(n: number): Uint8Array {
  const b = new Uint8Array(4);
  b[0] = (n >>> 24) & 0xff;
  b[1] = (n >>> 16) & 0xff;
  b[2] = (n >>> 8) & 0xff;
  b[3] = n & 0xff;
  return b;
}

const CATALOG_CHAIN_IDS = [
  1, 2, 5, 56, 137, 31337, 16993, 8453, 42161, 17611, // 17611 = 0x44cb (bytes4 BE)
] as const;

describe("crossChainHash fuzz (V2 layout vs viem)", () => {
  const FUZZ_ITERATIONS = process.env.CI ? 800 : 4000;

  it(`manual 224-byte keccak matches computeXchainHashIdBytes (${FUZZ_ITERATIONS} cases)`, () => {
    const rng = mulberry32(0xdeadbeef);
    for (let _i = 0; _i < FUZZ_ITERATIONS; _i++) {
      const srcChain = randBytes(rng, 4);
      const destChain = randBytes(rng, 4);
      const srcAccount = randBytes(rng, 32);
      const destAccount = randBytes(rng, 32);
      const token = randBytes(rng, 32);
      const amount = randU128(rng);
      const nonce = randU64(rng);

      const viaViem = computeXchainHashIdBytes(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce,
      );
      const viaManual = computeXchainHashManual224(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce,
      );
      expect(Buffer.from(viaViem).toString("hex")).toBe(
        Buffer.from(viaManual).toString("hex"),
      );

      const viaSolanaHelper = computeTransferHash(
        srcChain,
        destChain,
        srcAccount,
        destAccount,
        token,
        amount,
        nonce,
      );
      expect(Buffer.from(viaSolanaHelper).toString("hex")).toBe(
        Buffer.from(viaManual).toString("hex"),
      );
    }
  });

  it("catalogued chain-id pairs match manual hash (permutation sample)", () => {
    const rng = mulberry32(0xcafe0022);
    for (const srcId of CATALOG_CHAIN_IDS) {
      for (const destId of CATALOG_CHAIN_IDS) {
        if (srcId === destId) continue;
        const srcChain = u32ToBytes4(srcId);
        const destChain = u32ToBytes4(destId);
        for (let k = 0; k < 8; k++) {
          const srcAccount = randBytes(rng, 32);
          const destAccount = randBytes(rng, 32);
          const token = randBytes(rng, 32);
          const amount = randU128(rng);
          const nonce = randU64(rng);
          const a = computeXchainHashIdBytes(
            srcChain,
            destChain,
            srcAccount,
            destAccount,
            token,
            amount,
            nonce,
          );
          const b = computeXchainHashManual224(
            srcChain,
            destChain,
            srcAccount,
            destAccount,
            token,
            amount,
            nonce,
          );
          expect(Buffer.from(a).toString("hex")).toBe(
            Buffer.from(b).toString("hex"),
          );
        }
      }
    }
  });
});

describe("withdrawSubmitSrcAccountBytes32 fuzz", () => {
  it("EVM-style 0x+40 hex always yields 32 bytes with address in last 20 bytes (2000 cases)", () => {
    const rng = mulberry32(0x5c4acc00);
    for (let i = 0; i < 2000; i++) {
      const addr20 = randBytes(rng, 20);
      const hex =
        `0x${Buffer.from(addr20).toString("hex")}` as `0x${string}`;
      const u8 = withdrawSubmitSrcAccountBytes32(hex);
      expect(u8.length).toBe(32);
      expect(Buffer.from(u8.subarray(0, 12)).toString("hex")).toBe(
        "000000000000000000000000",
      );
      expect(Buffer.from(u8.subarray(12, 32)).toString("hex")).toBe(
        Buffer.from(addr20).toString("hex"),
      );
    }
  });
});

describe("resolveWithdrawSrcTokenBytesForSolana fuzz", () => {
  it("0x+40 token resolves to 32 bytes with trailing 20-byte address (500 cases)", () => {
    const rng = mulberry32(0x70c30001);
    for (let i = 0; i < 500; i++) {
      const addr20 = randBytes(rng, 20);
      const hex =
        `0x${Buffer.from(addr20).toString("hex")}` as `0x${string}`;
      const bytes = resolveWithdrawSrcTokenBytesForSolana(hex);
      expect(bytes).not.toBeNull();
      expect(bytes!.length).toBe(32);
      expect(Buffer.from(bytes!.subarray(12, 32)).toString("hex")).toBe(
        Buffer.from(addr20).toString("hex"),
      );
    }
  });
});
