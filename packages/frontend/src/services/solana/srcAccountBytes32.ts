/**
 * Normalize `TransferRecord.srcAccount` to 32 bytes for Solana `withdraw_submit`
 * (must match EVM `bytes32(uint256(uint160(addr)))` and full bytes32 hex).
 */

import type { Hex } from "viem";
import { hexToUint8Array, evmAddressToBytes32Array } from "../terra/withdrawSubmit";
import { solanaAddressToBytes32 } from "./address";

/**
 * @param account - EVM `0x` + 40 hex, full `0x` + 64 hex, or Solana base58
 */
export function withdrawSubmitSrcAccountBytes32(account: string): Uint8Array {
  const t = account.trim();
  if (!t) {
    return new Uint8Array(32);
  }
  if (!t.startsWith("0x")) {
    try {
      return new Uint8Array(hexToUint8Array(solanaAddressToBytes32(t)));
    } catch {
      return new Uint8Array(32);
    }
  }
  const hex = t.slice(2);
  if (hex.length === 40) {
    return new Uint8Array(evmAddressToBytes32Array(t));
  }
  if (hex.length === 64) {
    return new Uint8Array(hexToUint8Array(t as Hex));
  }
  const raw = new Uint8Array(hexToUint8Array(t as Hex));
  if (raw.length === 32) {
    return raw;
  }
  if (raw.length === 20) {
    return new Uint8Array(evmAddressToBytes32Array(t));
  }
  return new Uint8Array(32);
}
