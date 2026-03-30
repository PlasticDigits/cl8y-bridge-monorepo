/**
 * Terra bridge `incoming_token_mapping.src_token` encoding for wasm execute payloads.
 * Matches `encode_token_address` in packages/contracts-terraclassic/bridge/src/hash.rs:
 * - Native denoms (short, not a terra1 CW20): keccak256(UTF-8 bytes)
 * - CW20 `terra1…` (44 or 64 chars): canonical address bytes, left-padded to 32 bytes
 *
 * Aligns with Settings token verification (`useTokenVerification` / `terraAddressToBytes32`).
 */
import { hexToBytes, keccak256, toBytes, type Hex } from 'viem'

import { terraAddressToBytes32 } from './hashVerification'

/** Base64-encoded 32-byte `src_token` for Terra `set_incoming_token_mapping`. */
export function terraIncomingSrcTokenB64(terraTokenId: string): string {
  const isCw20Shape =
    terraTokenId.startsWith('terra1') &&
    (terraTokenId.length === 44 || terraTokenId.length === 64)
  if (isCw20Shape) {
    const hex = terraAddressToBytes32(terraTokenId)
    return Buffer.from(hexToBytes(hex)).toString('base64')
  }
  const hash = keccak256(toBytes(terraTokenId)) as Hex
  return Buffer.from(hexToBytes(hash)).toString('base64')
}
