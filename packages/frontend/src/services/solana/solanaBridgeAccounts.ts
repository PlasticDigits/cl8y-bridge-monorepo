/**
 * Solana cl8y_bridge PDAs and BridgeConfig account layout.
 * Matches `packages/contracts-solana/programs/cl8y-bridge/src/state/bridge.rs`.
 *
 * - Use {@link getSolanaProgramIdString} for building instructions / TokenMapping PDAs (always the deploy address).
 * - Use `bridgeAddress` on {@link BridgeChainConfig} for Solana as the BridgeConfig PDA (RPC reads, display).
 */

import { PublicKey } from "@solana/web3.js";
import type { BridgeChainConfig } from "../../types/chain";

/** Anchor seed `BridgeConfig::SEED` (`b"bridge"`). */
export const BRIDGE_CONFIG_SEED = Buffer.from("bridge");

export function findBridgeConfigPda(programId: PublicKey): PublicKey {
  const [pda] = PublicKey.findProgramAddressSync(
    [BRIDGE_CONFIG_SEED],
    programId,
  );
  return pda;
}

/** Base58 BridgeConfig PDA, or empty string if `programIdStr` is missing/invalid. */
export function bridgeConfigPdaBase58(programIdStr: string): string {
  const t = programIdStr.trim();
  if (!t) return "";
  try {
    return findBridgeConfigPda(new PublicKey(t)).toBase58();
  } catch {
    return "";
  }
}

/**
 * Deployed program id for Solana bridge instructions and PDA derivation.
 * Prefer `programId`. Falls back to `bridgeAddress` only for legacy configs that stored the program id there
 * (before `bridgeAddress` meant the BridgeConfig PDA). New configs should set both from env.
 */
export function getSolanaProgramIdString(
  chain: BridgeChainConfig,
): string | null {
  if (chain.type !== "solana") return null;
  const pid = chain.programId?.trim();
  if (pid) return pid;
  return chain.bridgeAddress?.trim() || null;
}

/**
 * Parsed `BridgeConfig` fields after the 8-byte Anchor account discriminator.
 * Layout: admin, operator, fee_bps, withdraw_delay, deposit_nonce, accrued_native_fees, paused, chain_id, bump.
 */
export function parseBridgeConfigFromAnchorAccount(
  data: Uint8Array,
): {
  admin: PublicKey;
  operator: PublicKey;
  feeBps: number;
  withdrawDelaySeconds: bigint;
} | null {
  const disc = 8;
  const minLen = disc + 32 + 32 + 2 + 8;
  if (data.length < minLen) return null;
  const view = new DataView(
    data.buffer,
    data.byteOffset + disc,
    data.byteLength - disc,
  );
  const admin = new PublicKey(data.subarray(disc, disc + 32));
  const operator = new PublicKey(data.subarray(disc + 32, disc + 64));
  const feeBps = view.getUint16(64, true);
  const withdrawDelaySeconds = view.getBigInt64(66, true);
  return { admin, operator, feeBps, withdrawDelaySeconds };
}
