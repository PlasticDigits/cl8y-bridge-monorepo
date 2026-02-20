/**
 * Terra Bridge Token Destination Mapping
 *
 * Queries the Terra bridge's token_dest_mapping to get the destination token
 * for a specific chain (e.g. which ERC20 on Anvil1 when bridging from Terra).
 * Falls back to evm_token_address from tokens list if no per-chain mapping.
 */

import { queryContract } from './lcdClient'
import { CONTRACTS, DEFAULT_NETWORK, NETWORKS } from '../utils/constants'

interface TokenDestMappingResponse {
  token: string
  dest_chain: string // base64
  dest_token: string // base64 of 32 bytes
  dest_decimals: number
}

/**
 * Convert bytes4 hex (e.g. 0x00000003) to base64 Binary for CosmWasm.
 * Uses browser-native APIs (no Node.js Buffer).
 */
function bytes4ToBase64(bytes4Hex: string): string {
  const clean = bytes4Hex.replace(/^0x/, '').toLowerCase()
  if (clean.length !== 8) throw new Error('bytes4 must be 8 hex chars')
  const arr = new Uint8Array(clean.match(/.{2}/g)!.map((b) => parseInt(b, 16)))
  return btoa(String.fromCharCode(...arr))
}

/**
 * Decode base64 Binary to hex (32 bytes -> 0x...).
 * Uses browser-native APIs (no Node.js Buffer).
 */
function base64ToHex(b64: string): string {
  const raw = atob(b64)
  return '0x' + Array.from(raw, (c) => c.charCodeAt(0).toString(16).padStart(2, '0')).join('')
}

export interface TokenDestMappingResult {
  hex: string
  decimals: number
}

/**
 * Query Terra bridge for token destination mapping.
 * Returns the dest token bytes32 as hex + decimals, or null if no mapping.
 */
export async function queryTokenDestMapping(
  terraToken: string,
  destChainBytes4: string
): Promise<TokenDestMappingResult | null> {
  const terraBridge = CONTRACTS[DEFAULT_NETWORK].terraBridge
  if (!terraBridge) return null

  const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
  const lcdUrls = networkConfig.lcdFallbacks?.length
    ? [...networkConfig.lcdFallbacks]
    : [networkConfig.lcd]

  try {
    const destChainB64 = bytes4ToBase64(destChainBytes4)
    const res = await queryContract<TokenDestMappingResponse>(lcdUrls, terraBridge, {
      token_dest_mapping: {
        token: terraToken,
        dest_chain: destChainB64,
      },
    })

    if (!res?.dest_token) return null
    const hex = base64ToHex(res.dest_token)
    if (hex === '0x' + '0'.repeat(64)) return null
    return { hex, decimals: res.dest_decimals }
  } catch {
    return null
  }
}
