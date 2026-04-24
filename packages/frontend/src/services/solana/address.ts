import { PublicKey } from "@solana/web3.js";

/**
 * `PublicKey` (string ctor) only checks base58 decode to 32 bytes — it does not verify
 * the point lies on the ed25519 curve. Bridge recipients must be valid user pubkeys, so
 * we require `PublicKey.isOnCurve` (GL-117: typists can swap base58 symbols and stay “decodable”
 * but off-curve).
 */
function parseOnCurveUserPubkeyBase58(address: string): PublicKey {
  const pubkey = new PublicKey(address);
  if (!PublicKey.isOnCurve(pubkey)) {
    throw new Error("Solana public key is not on the ed25519 curve");
  }
  return pubkey;
}

/**
 * Convert a Solana public key to bytes32 for use in transfer hashes.
 * Solana pubkeys are already 32 bytes.
 */
export function solanaAddressToBytes32(address: string): `0x${string}` {
  const pubkey = parseOnCurveUserPubkeyBase58(address);
  const bytes = pubkey.toBytes();
  return `0x${Buffer.from(bytes).toString("hex")}` as `0x${string}`;
}

/**
 * Convert bytes32 back to a Solana public key string.
 */
export function bytes32ToSolanaAddress(bytes32: `0x${string}`): string {
  const hex = bytes32.replace("0x", "");
  const bytes = Buffer.from(hex, "hex");
  const pubkey = new PublicKey(bytes);
  if (!PublicKey.isOnCurve(pubkey)) {
    throw new Error("Solana public key is not on the ed25519 curve");
  }
  return pubkey.toBase58();
}

/**
 * Check if a string is a valid on-curve Solana user address (ed25519 public key, base58).
 * Uses {@link PublicKey.isOnCurve} — not only 32-byte base58 decode.
 */
export function isValidSolanaAddress(address: string): boolean {
  try {
    parseOnCurveUserPubkeyBase58(address);
    return true;
  } catch {
    return false;
  }
}

/**
 * Shorten a Solana address for display.
 */
export function shortenSolanaAddress(address: string, chars = 4): string {
  return `${address.slice(0, chars)}...${address.slice(-chars)}`;
}
