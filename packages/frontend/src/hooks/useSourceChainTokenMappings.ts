/**
 * useSourceChainTokenMappings - For EVM source, returns which Terra tokens have
 * a token_dest_mapping on the source chain. Only these tokens exist on that chain.
 * Used to filter the bridge dropdown and resolve the correct token address.
 */

import { useQueries } from '@tanstack/react-query'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { bytes32ToAddress } from '../services/evm/tokenRegistry'
import { CONTRACTS, DEFAULT_NETWORK } from '../utils/constants'

export interface SourceChainTokenMappings {
  /** Map terra token id -> evm address on source chain. Only tokens with a mapping. */
  mappings: Record<string, string>
  isLoading: boolean
}

/**
 * Query token_dest_mapping for each registry token on the source EVM chain.
 * Returns only tokens that have a mapping (exist on that chain).
 */
export function useSourceChainTokenMappings(
  registryTokens:
    | Array<{ token: string; evm_token_address: string; enabled?: boolean }>
    | undefined,
  sourceChainBytes4: string | undefined,
  enabled: boolean
): SourceChainTokenMappings {
  const tokens = (registryTokens ?? []).filter(
    (t) => t.enabled !== false && t.evm_token_address
  )
  const terraBridge = CONTRACTS[DEFAULT_NETWORK].terraBridge
  const queries = useQueries({
    queries: tokens.map((t) => ({
      queryKey: ['tokenDestMapping', terraBridge, t.token, sourceChainBytes4],
      queryFn: async () => {
        if (!t.token || !sourceChainBytes4) return null
        const hex = await queryTokenDestMapping(t.token, sourceChainBytes4)
        if (!hex) return null
        return bytes32ToAddress(hex as `0x${string}`)
      },
      enabled: enabled && !!t.token && !!sourceChainBytes4,
      staleTime: 60_000,
    })),
  })

  const mappings: Record<string, string> = {}
  let isLoading = false

  // If enabled but upstream registryTokens haven't loaded yet, we're still
  // waiting — report loading so callers don't use fallback addresses.
  if (enabled && !registryTokens) {
    isLoading = true
  }

  tokens.forEach((t, i) => {
    const q = queries[i]
    if (q?.isLoading) isLoading = true
    const addr = q?.data
    if (addr) {
      mappings[t.token] = addr
    }
  })

  return { mappings, isLoading }
}
