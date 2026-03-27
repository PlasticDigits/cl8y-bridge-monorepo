/**
 * useTokenChains - Returns which chains a token is registered on.
 * Queries token_dest_mapping for each EVM and Solana chain to get per-chain addresses.
 */

import { useMemo } from 'react'
import { useQueries } from '@tanstack/react-query'
import { queryTokenDestMapping } from '../services/terraTokenDestMapping'
import { bytes32ToAddress, normalizeToEvmAddress } from '../services/evm/tokenRegistry'
import { bytes32ToSolanaAddress } from '../services/solana/address'
import { BRIDGE_CHAINS, getExplorerUrlForChain, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'

export interface TokenChainInfo {
  chainId: string
  chainName: string
  type: 'cosmos' | 'evm' | 'solana'
  address: string
  rpcUrl?: string
  /** Base explorer URL for building token/address links */
  explorerUrl: string
  /** Token decimals on this chain (from token_dest_mapping for EVM/Solana, from registry for cosmos) */
  decimals?: number
}

/**
 * Get all chains a token is listed on, with addresses.
 * Terra chain always has the token. EVM and Solana from token_dest_mapping; EVM also uses evm_token_address fallback.
 */
export function useTokenChains(
  terraTokenId: string | undefined,
  evmTokenAddressFromRegistry: string | undefined
) {
  const tier = DEFAULT_NETWORK as NetworkTier
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const chains = BRIDGE_CHAINS[tier] ?? {}
  /** EVM + Solana: LCD token_dest_mapping (bytes32 → EVM address or SPL mint). Solana requires program id. */
  const mappingChainEntries = Object.entries(chains).filter(
    (entry): entry is [string, typeof chains[string] & { bytes4ChainId: string }] => {
      const c = entry[1]
      if (!c.bytes4ChainId) return false
      if (c.type === 'evm') return true
      if (c.type === 'solana') return !!c.bridgeAddress
      return false
    }
  )

  const mappingQueries = useQueries({
    queries: mappingChainEntries.map(([_chainId, config]) => ({
      queryKey: ['tokenDestMapping', terraTokenId, config.bytes4ChainId, config.type],
      queryFn: async () => {
        if (!terraTokenId || !config.bytes4ChainId) return null
        const result = await queryTokenDestMapping(terraTokenId, config.bytes4ChainId)
        if (!result) return null
        const hex = result.hex as `0x${string}`
        if (config.type === 'solana') {
          return {
            kind: 'solana' as const,
            address: bytes32ToSolanaAddress(hex),
            decimals: result.decimals,
          }
        }
        return {
          kind: 'evm' as const,
          address: bytes32ToAddress(hex),
          decimals: result.decimals,
        }
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

    // Cosmos/Terra chain always (decimals from registry, passed by caller)
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

    // EVM + Solana from token_dest_mapping (EVM may use registry fallback when mapping missing)
    mappingChainEntries.forEach(([chainId, config], idx) => {
      const query = mappingQueries[idx]
      const queryData = query?.data
      const isEvm = config.type === 'evm'

      if (config.type === 'solana') {
        if (queryData?.kind === 'solana' && queryData.address) {
          result.push({
            chainId,
            chainName: config.name,
            type: 'solana',
            address: queryData.address,
            rpcUrl: config.rpcUrl,
            explorerUrl: getExplorerUrlForChain(chainId),
            decimals: queryData.decimals,
          })
        }
        return
      }

      const addr = queryData
        ? queryData.kind === 'evm'
          ? queryData.address
          : null
        : fallbackEvmAddr && !query?.isLoading
          ? fallbackEvmAddr
          : null
      const decimals = queryData?.kind === 'evm' ? queryData.decimals : undefined
      if (addr && isEvm) {
        result.push({
          chainId,
          chainName: config.name,
          type: 'evm',
          address: addr,
          rpcUrl: config.rpcUrl,
          explorerUrl: getExplorerUrlForChain(chainId),
          decimals,
        })
      }
    })

    return result
  }, [chains, terraTokenId, mappingChainEntries, mappingQueries, fallbackEvmAddr])
}
