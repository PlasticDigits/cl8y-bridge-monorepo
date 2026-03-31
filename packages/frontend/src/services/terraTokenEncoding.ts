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

/** 32-byte `dest_token` for Solana↔Terra mappings: keccak256(UTF-8 `terraTokenId`). */
export function terraDestTokenKeccakUtf8Bytes(terraTokenId: string): Uint8Array {
  return hexToBytes(keccak256(toBytes(terraTokenId)) as Hex)
}

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

/**
 * Same encoding as {@link terraIncomingSrcTokenB64}, but CW20-shaped strings that fail
 * bech32 decode use keccak256(UTF-8) — matches on-chain `encode_token_address` when `addr_validate` fails.
 */
export function terraIncomingSrcTokenB64WithKeccakFallback(
  terraTokenId: string
): string | null {
  try {
    const isCw20Shape =
      terraTokenId.startsWith('terra1') &&
      (terraTokenId.length === 44 || terraTokenId.length === 64)
    if (isCw20Shape) {
      try {
        return terraIncomingSrcTokenB64(terraTokenId)
      } catch {
        // fall through to keccak path
      }
    }
    const hash = keccak256(toBytes(terraTokenId)) as Hex
    return Buffer.from(hexToBytes(hash)).toString('base64')
  } catch {
    return null
  }
}
