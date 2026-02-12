/**
 * useChainRegistry Hook
 *
 * React hook wrapping chain discovery with React Query caching.
 * Provides bytes4 chain ID -> BridgeChainConfig resolution.
 */

import { useQuery } from '@tanstack/react-query'
import { buildChainIdMap } from '../services/chainDiscovery'

const QUERY_KEY = ['chainRegistry']

/**
 * Hook to get the complete bytes4 -> chain config map.
 * Cached via React Query with 5-minute stale time.
 */
export function useChainRegistry() {
  return useQuery({
    queryKey: QUERY_KEY,
    queryFn: buildChainIdMap,
    staleTime: 5 * 60 * 1000, // 5 minutes
    gcTime: 30 * 60 * 1000, // 30 minutes
  })
}

/**
 * Hook to resolve a specific bytes4 chain ID to a config.
 * Uses the cached registry map.
 */
export function useChainByBytes4(bytes4Hex: string | null | undefined) {
  const { data: registry, ...rest } = useChainRegistry()

  return {
    chain: bytes4Hex && registry ? registry.get(bytes4Hex.toLowerCase()) : undefined,
    ...rest,
  }
}

/**
 * Hook to get chain config by chain ID (numeric or string).
 * For EVM chains, also checks bytes4 registry.
 */
export function useChainByChainId(chainId: number | string | null | undefined) {
  const { data: registry, ...rest } = useChainRegistry()

  // First try direct chain ID lookup
  // Then try bytes4 lookup if it's a hex string
  const chain = chainId
    ? (() => {
        // Try to find by chainId directly (would need getAllBridgeChains imported)
        // For now, iterate registry values
        if (registry) {
          for (const config of registry.values()) {
            if (config.chainId === chainId) {
              return config
            }
          }
        }
        return undefined
      })()
    : undefined

  return {
    chain,
    ...rest,
  }
}
