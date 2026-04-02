/**
 * Terra bridge `incoming_token_mapping.src_token` encoding for wasm execute payloads.
 * Matches `encode_token_address` in packages/contracts-terraclassic/bridge/src/hash.rs:
 * - Native denoms (short, not a terra1 CW20): keccak256(UTF-8 bytes)
 * - CW20 `terra1…` (44 or 64 chars): canonical address bytes, left-padded to 32 bytes
 *
 * Aligns with Settings token verification (`useTokenVerification` / `terraAddressToBytes32`).
 */
import { bytesToHex, hexToBytes, keccak256, toBytes, type Hex } from 'viem'

import { terraAddressToBytes32 } from './hashVerification'

function isCw20ShapedTerraToken(terraTokenId: string): boolean {
  return (
    terraTokenId.startsWith('terra1') &&
    (terraTokenId.length === 44 || terraTokenId.length === 64)
  )
}

/**
 * Raw 32-byte `src_token` matching Terra `encode_token_address` (hash.rs): CW20 → canonical
 * address bytes32; native denom → keccak256(UTF-8). Throws if a CW20-shaped string is not valid bech32.
 */
export function terraTokenIdToSrcTokenBytesStrict(terraTokenId: string): Uint8Array {
  if (isCw20ShapedTerraToken(terraTokenId)) {
    const hex = terraAddressToBytes32(terraTokenId)
    return new Uint8Array(hexToBytes(hex))
  }
  return new Uint8Array(hexToBytes(keccak256(toBytes(terraTokenId)) as Hex))
}

/**
 * Same as {@link terraTokenIdToSrcTokenBytesStrict}, but invalid CW20 bech32 falls back to
 * keccak256(UTF-8) — matches on-chain `encode_token_address` when `addr_validate` fails.
 * Used for Solana withdrawSubmit seeds and relaxed registration paths.
 */
export function terraTokenIdToSrcTokenBytes(terraTokenId: string): Uint8Array {
  if (isCw20ShapedTerraToken(terraTokenId)) {
    try {
      return terraTokenIdToSrcTokenBytesStrict(terraTokenId)
    } catch {
      return new Uint8Array(hexToBytes(keccak256(toBytes(terraTokenId)) as Hex))
    }
  }
  return terraTokenIdToSrcTokenBytesStrict(terraTokenId)
}

/**
 * 32-byte `dest_token` for Solana `register_token` when the remote chain is Terra
 * (TokenMapping PDA seed). Same rules as {@link terraTokenIdToSrcTokenBytesStrict} /
 * `encode_token_address`: native denom → keccak256(UTF-8); CW20 → bech32-decoded bytes
 * left-padded to 32 bytes (not keccak of the bech32 string).
 */
export function terraDestTokenKeccakUtf8Bytes(terraTokenId: string): Uint8Array {
  return terraTokenIdToSrcTokenBytesStrict(terraTokenId)
}

/**
 * `0x`-prefixed 32-byte hex for Terra-side token in:
 * - EVM `TokenRegistry.setTokenDestinationWithDecimals` when dest chain is Terra
 * - cross-chain `xchainHashId` / `HashLib.computeXchainHashId` token word
 *
 * Matches CosmWasm `encode_token_address` (`packages/contracts-terraclassic/bridge/src/hash.rs`).
 */
export function terraTokenIdToEncodeTokenAddressHex(terraTokenId: string): Hex {
  return bytesToHex(terraTokenIdToSrcTokenBytesStrict(terraTokenId)) as Hex
}

/** Base64-encoded 32-byte `src_token` for Terra `set_incoming_token_mapping`. */
export function terraIncomingSrcTokenB64(terraTokenId: string): string {
  return Buffer.from(terraTokenIdToSrcTokenBytesStrict(terraTokenId)).toString('base64')
}

/**
 * Same encoding as {@link terraIncomingSrcTokenB64}, but CW20-shaped strings that fail
 * bech32 decode use keccak256(UTF-8) — matches on-chain `encode_token_address` when `addr_validate` fails.
 */
export function terraIncomingSrcTokenB64WithKeccakFallback(
  terraTokenId: string
): string | null {
  try {
    return Buffer.from(terraTokenIdToSrcTokenBytes(terraTokenId)).toString('base64')
  } catch {
    return null
  }
}
