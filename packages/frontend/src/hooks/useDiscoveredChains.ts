/**
 * useDiscoveredChains Hook
 *
 * Queries the on-chain ChainRegistry to discover which V2 chain IDs
 * are actually registered, then filters the static chain list to only
 * show live chains. Falls back to the full static list if the RPC
 * query fails.
 */

import { useQuery } from '@tanstack/react-query'
import type { ChainInfo } from '../types/chain'
import { getChainsForTransfer, BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'
import { discoverRegisteredChains } from '../services/chainDiscovery'

async function fetchDiscoveredChains(): Promise<ChainInfo[]> {
  const staticChains = getChainsForTransfer()
  const tier = DEFAULT_NETWORK as NetworkTier
  const configs = BRIDGE_CHAINS[tier]

  const allConfigs = Object.values(configs)
  const registered = await discoverRegisteredChains(allConfigs)

  if (!registered) {
    return staticChains
  }

  return staticChains.filter((chain) => {
    if (chain.type === 'cosmos') return true

    const config = configs[chain.id]
    if (!config?.bytes4ChainId) return false

    return registered.has(config.bytes4ChainId.toLowerCase())
  })
}

export function useDiscoveredChains() {
  const query = useQuery({
    queryKey: ['discoveredChains'],
    queryFn: fetchDiscoveredChains,
    staleTime: 5 * 60 * 1000,
    gcTime: 30 * 60 * 1000,
  })

  return {
    chains: query.data ?? getChainsForTransfer(),
    isLoading: query.isLoading,
    error: query.error,
  }
}
