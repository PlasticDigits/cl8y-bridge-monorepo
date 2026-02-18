/**
 * EVM Client Factory
 *
 * Cached viem PublicClient factory for multi-chain queries.
 * Reuses clients per chain to avoid creating multiple instances.
 * Supports RPC fallbacks via viem's fallback() transport when
 * multiple URLs are configured.
 */

import { createPublicClient, http, fallback, type PublicClient } from 'viem'
import type { BridgeChainConfig } from '../types/chain'

const clientCache = new Map<string, PublicClient>()

/**
 * Get or create a cached PublicClient for the given bridge chain config.
 * When `rpcFallbacks` are configured, uses viem's fallback() transport
 * to automatically cycle through URLs on failure.
 */
export function getEvmClient(chain: BridgeChainConfig): PublicClient {
  if (chain.type !== 'evm') {
    throw new Error(`Cannot create EVM client for ${chain.type} chain`)
  }

  const urls = [chain.rpcUrl, ...(chain.rpcFallbacks ?? [])]
  const cacheKey = urls.join('|')
  const cached = clientCache.get(cacheKey)

  if (cached) {
    return cached
  }

  const transport = urls.length > 1
    ? fallback(urls.map(url => http(url, { timeout: 10_000 })))
    : http(urls[0], { timeout: 10_000 })

  const client = createPublicClient({
    transport,
    chain: {
      id: chain.chainId as number,
      name: chain.name,
      nativeCurrency: { decimals: 18, name: 'ETH', symbol: 'ETH' },
      rpcUrls: { default: { http: urls } },
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
