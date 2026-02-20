/**
 * useTokenChains - Returns which chains a token is registered on.
 * Queries token_dest_mapping for each EVM chain to get per-chain addresses.
 */

import { useMemo } from 'react'
import { useQueries } from '@tanstack/react-query'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { bytes32ToAddress, normalizeToEvmAddress } from '../services/evm/tokenRegistry'
import { BRIDGE_CHAINS, getExplorerUrlForChain, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'

export interface TokenChainInfo {
  chainId: string
  chainName: string
  type: 'cosmos' | 'evm'
  address: string
  rpcUrl?: string
  /** Base explorer URL for building token/address links */
  explorerUrl: string
}

/**
 * Get all chains a token is listed on, with addresses.
 * Terra chain always has the token. EVM chains from token_dest_mapping or evm_token_address fallback.
 */
export function useTokenChains(
  terraTokenId: string | undefined,
  evmTokenAddressFromRegistry: string | undefined
) {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier] ?? {}
  const evmChainEntries = Object.entries(chains).filter(
    (entry): entry is [string, typeof chains[string] & { bytes4ChainId: string }] =>
      entry[1].type === 'evm' && !!entry[1].bytes4ChainId
  )

  const evmQueries = useQueries({
    queries: evmChainEntries.map(([_chainId, config]) => ({
      queryKey: ['tokenDestMapping', terraTokenId, config.bytes4ChainId],
      queryFn: async () => {
        if (!terraTokenId || !config.bytes4ChainId) return null
        const hex = await queryTokenDestMapping(terraTokenId, config.bytes4ChainId)
        if (!hex) return null
        return bytes32ToAddress(hex as `0x${string}`)
      },
      enabled: !!terraTokenId && !!config.bytes4ChainId,
      staleTime: 60_000,
    })),
  })

  let fallbackEvmAddr: string | null = null
  try {
    if (evmTokenAddressFromRegistry) {
      fallbackEvmAddr = normalizeToEvmAddress(evmTokenAddressFromRegistry)
    }
  } catch {
    fallbackEvmAddr = null
  }

  return useMemo((): TokenChainInfo[] => {
    const result: TokenChainInfo[] = []

    // Cosmos/Terra chain always
    for (const [chainId, config] of Object.entries(chains)) {
      if (config.type === 'cosmos') {
        result.push({
          chainId,
          chainName: config.name,
          type: 'cosmos',
          address: terraTokenId ?? '',
          rpcUrl: config.lcdUrl,
          explorerUrl: getExplorerUrlForChain(chainId),
        })
      }
    }

    // EVM chains from token_dest_mapping or fallback
    evmChainEntries.forEach(([chainId, config], idx) => {
      const query = evmQueries[idx]
      const addr = query?.data ?? (fallbackEvmAddr && !query?.isLoading ? fallbackEvmAddr : null)
      if (addr) {
        result.push({
          chainId,
          chainName: config.name,
          type: 'evm',
          address: addr,
          rpcUrl: config.rpcUrl,
          explorerUrl: getExplorerUrlForChain(chainId),
        })
      }
    })

    return result
  }, [chains, terraTokenId, evmChainEntries, evmQueries, fallbackEvmAddr])
}
