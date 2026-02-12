/**
 * useTokenRegistry Hook
 *
 * Queries Terra bridge contract for registered tokens.
 * Terra bridge has Tokens { start_after, limit } returning TokensResponse.
 */

import { useQuery } from '@tanstack/react-query'
import { CONTRACTS, DEFAULT_NETWORK, NETWORKS } from '../utils/constants'
import { queryContract } from '../services/lcdClient'

const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
const lcdUrls =
  networkConfig.lcdFallbacks && networkConfig.lcdFallbacks.length > 0
    ? [...networkConfig.lcdFallbacks]
    : [networkConfig.lcd]

export interface TokenEntry {
  token: string
  is_native: boolean
  evm_token_address: string
  terra_decimals: number
  evm_decimals: number
  enabled: boolean
}

export interface TokensResponse {
  tokens: TokenEntry[]
}

const DEFAULT_LIMIT = 50

export function useTokenRegistry() {
  const terraBridge = CONTRACTS[DEFAULT_NETWORK].terraBridge

  return useQuery({
    queryKey: ['tokenRegistry', terraBridge],
    queryFn: async () => {
      const allTokens: TokenEntry[] = []
      let startAfter: string | undefined
      let hasMore = true

      while (hasMore) {
        const res = await queryContract<TokensResponse>(lcdUrls, terraBridge!, {
          tokens: { start_after: startAfter, limit: DEFAULT_LIMIT },
        })
        if (!res.tokens || res.tokens.length === 0) break
        allTokens.push(...res.tokens)
        if (res.tokens.length < DEFAULT_LIMIT) break
        startAfter = res.tokens[res.tokens.length - 1].token
        hasMore = res.tokens.length === DEFAULT_LIMIT
      }

      return allTokens
    },
    enabled: !!terraBridge,
    staleTime: 60_000,
  })
}
