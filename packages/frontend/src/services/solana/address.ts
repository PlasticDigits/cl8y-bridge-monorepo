import { PublicKey } from "@solana/web3.js";

/**
 * Convert a Solana public key to bytes32 for use in transfer hashes.
 * Solana pubkeys are already 32 bytes.
 */
export function solanaAddressToBytes32(address: string): `0x${string}` {
  const pubkey = new PublicKey(address);
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
  return pubkey.toBase58();
}

/**
 * Check if a string is a valid Solana address.
 */
export function isValidSolanaAddress(address: string): boolean {
  try {
    new PublicKey(address);
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
