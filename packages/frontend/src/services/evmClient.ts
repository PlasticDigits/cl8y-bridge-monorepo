/**
 * EVM Client Factory
 *
 * Cached viem PublicClient factory for multi-chain queries.
 * Reuses clients per chain to avoid creating multiple instances.
 */

import { createPublicClient, http, type PublicClient } from 'viem'
import type { BridgeChainConfig } from '../types/chain'

const clientCache = new Map<string, PublicClient>()

/**
 * Get or create a cached PublicClient for the given bridge chain config.
 * Clients are keyed by rpcUrl to ensure one client per RPC endpoint.
 */
export function getEvmClient(chain: BridgeChainConfig): PublicClient {
  if (chain.type !== 'evm') {
    throw new Error(`Cannot create EVM client for ${chain.type} chain`)
  }

  const cacheKey = chain.rpcUrl
  const cached = clientCache.get(cacheKey)

  if (cached) {
    return cached
  }

  const client = createPublicClient({
    transport: http(chain.rpcUrl, { timeout: 10000 }),
    chain: {
      id: chain.chainId as number,
      name: chain.name,
      nativeCurrency: { decimals: 18, name: 'ETH', symbol: 'ETH' },
      rpcUrls: { default: { http: [chain.rpcUrl] } },
    },
  })

  clientCache.set(cacheKey, client)
  return client
}

/**
 * Clear the client cache (useful for testing or config changes).
 */
export function clearEvmClientCache(): void {
  clientCache.clear()
}
