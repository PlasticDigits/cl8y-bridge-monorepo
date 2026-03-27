/**
 * useSourceChainTokenMappings - For EVM source, returns which Terra tokens have
 * a token_dest_mapping on the source chain. Only these tokens exist on that chain.
 * Used to filter the bridge dropdown and resolve the correct token address.
 */

import { useQueries } from '@tanstack/react-query'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { bytes32ToAddress } from '../services/evm/tokenRegistry'
import { bytes32ToSolanaAddress } from '../services/solana/address'
import { CONTRACTS, DEFAULT_NETWORK } from '../utils/constants'

/** V2 chain id for Solana (matches bridge `solana_localnet` registration). */
function isSolanaBytes4(bytes4: string | undefined): boolean {
  if (!bytes4) return false
  const h = bytes4.replace(/^0x/i, '').toLowerCase().padStart(8, '0')
  return h.slice(-8) === '00000005'
}

export interface SourceChainTokenMappings {
  /** Map terra token id -> evm address on source chain. Only tokens with a mapping. */
  mappings: Record<string, string>
  /** Map terra token id -> EVM decimals from the per-chain dest mapping. */
  decimalsMap: Record<string, number>
  isLoading: boolean
}

/**
 * Query token_dest_mapping for each registry token on the source EVM chain.
 * Returns only tokens that have a mapping (exist on that chain).
 */
export function useSourceChainTokenMappings(
  registryTokens:
    | Array<{ token: string; evm_token_address?: string; enabled?: boolean }>
    | undefined,
  sourceChainBytes4: string | undefined,
  enabled: boolean
): SourceChainTokenMappings {
  const tokens = (registryTokens ?? []).filter(
    (t) => t.enabled !== false
  )
  const terraBridge = CONTRACTS[DEFAULT_NETWORK].terraBridge
  const queries = useQueries({
    queries: tokens.map((t) => ({
      queryKey: ['tokenDestMapping', terraBridge, t.token, sourceChainBytes4],
      queryFn: async () => {
        if (!t.token || !sourceChainBytes4) return null
        const result = await queryTokenDestMapping(t.token, sourceChainBytes4)
        if (!result) return null
        const hex = result.hex as `0x${string}`
        const address = isSolanaBytes4(sourceChainBytes4)
          ? bytes32ToSolanaAddress(hex)
          : bytes32ToAddress(hex)
        return { address, decimals: result.decimals }
      },
      enabled: enabled && !!t.token && !!sourceChainBytes4,
      staleTime: 60_000,
    })),
  })

  const mappings: Record<string, string> = {}
  const decimalsMap: Record<string, number> = {}
  let isLoading = false

  // If enabled but upstream registryTokens haven't loaded yet, we're still
  // waiting — report loading so callers don't use fallback addresses.
  if (enabled && !registryTokens) {
    isLoading = true
  }

  tokens.forEach((t, i) => {
    const q = queries[i]
    if (q?.isLoading) isLoading = true
    const data = q?.data
    if (data) {
      mappings[t.token] = data.address
      decimalsMap[t.token] = data.decimals
    }
  })

  return { mappings, decimalsMap, isLoading }
}
