/**
 * useTokenOnchainInfo - Fetches token symbol/name from chain when not in tokenlist.
 * Supports Terra CW20 (token_info) and EVM ERC20 (symbol/name).
 * Results are cached in localStorage for instant display on subsequent page loads.
 */

import { useQuery } from '@tanstack/react-query'
import { queryContract } from '../services/lcdClient'
import { NETWORKS, DEFAULT_NETWORK } from '../utils/constants'
import { getAddress } from 'viem'
import { createPublicClient, http } from 'viem'
import { getEvmClient } from '../services/evmClient'
import type { BridgeChainConfig } from '../types/chain'

// ---------------------------------------------------------------------------
// localStorage cache for on-chain token info
// ---------------------------------------------------------------------------

const CACHE_KEY = 'cl8y-token-info-cache'
const CACHE_TTL = 7 * 24 * 60 * 60 * 1000 // 7 days

interface CachedEntry {
  symbol: string
  name: string
  ts: number
}

function getCache(): Record<string, CachedEntry> {
  try {
    const raw = localStorage.getItem(CACHE_KEY)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}

export function getCachedTokenInfo(key: string): { symbol: string; name: string } | undefined {
  const cache = getCache()
  const entry = cache[key]
  if (!entry || !entry.symbol || Date.now() - entry.ts > CACHE_TTL) return undefined
  return { symbol: entry.symbol, name: entry.name }
}

function setCachedTokenInfo(key: string, info: { symbol: string; name: string }) {
  try {
    const cache = getCache()
    cache[key] = { symbol: info.symbol, name: info.name, ts: Date.now() }
    localStorage.setItem(CACHE_KEY, JSON.stringify(cache))
  } catch { /* quota or private browsing */ }
}

// ---------------------------------------------------------------------------

const ERC20_ABI = [
  {
    name: 'symbol',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'string' }],
  },
  {
    name: 'name',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'string' }],
  },
] as const

interface Cw20TokenInfo {
  name?: string
  symbol?: string
  decimals?: number
  total_supply?: string
}

/** Fetcher for CW20 token symbol/name - used by useCw20TokenInfo and useQueries batch */
export async function fetchCw20TokenInfo(
  terraAddress: string
): Promise<{ symbol: string; name: string }> {
  if (!terraAddress?.startsWith('terra1')) return { symbol: '', name: '' }
  const networkConfig = NETWORKS[DEFAULT_NETWORK].terra
  const lcdUrls = networkConfig.lcdFallbacks?.length
    ? [...networkConfig.lcdFallbacks]
    : [networkConfig.lcd]
  const res = await queryContract<Cw20TokenInfo>(
    lcdUrls,
    terraAddress,
    { token_info: {} },
    8000
  )
  const result = { symbol: res?.symbol ?? '', name: res?.name ?? '' }
  if (result.symbol) setCachedTokenInfo(`cw20:${terraAddress}`, result)
  return result
}

export function useCw20TokenInfo(terraAddress: string | undefined, enabled: boolean) {
  const cached = terraAddress ? getCachedTokenInfo(`cw20:${terraAddress}`) : undefined
  return useQuery({
    queryKey: ['cw20TokenInfo', terraAddress],
    queryFn: () => (terraAddress ? fetchCw20TokenInfo(terraAddress) : { symbol: '', name: '' }),
    enabled: !!terraAddress && terraAddress.startsWith('terra1') && enabled,
    staleTime: 5 * 60 * 1000,
    initialData: cached,
  })
}

/** Fetcher for EVM token symbol/name. Uses getEvmClient (with RPC fallbacks) when chainConfig provided. */
export async function fetchEvmTokenInfo(
  evmAddress: string,
  rpcUrlOrConfig: string | BridgeChainConfig
): Promise<{ symbol: string; name: string }> {
  if (!evmAddress?.startsWith('0x')) return { symbol: '', name: '' }
  const addr = getAddress(evmAddress)
  const client =
    typeof rpcUrlOrConfig === 'string'
      ? createPublicClient({ transport: http(rpcUrlOrConfig) })
      : getEvmClient(rpcUrlOrConfig as BridgeChainConfig & { chainId: number })
  const [symbol, name] = await Promise.all([
    client.readContract({ address: addr, abi: ERC20_ABI, functionName: 'symbol' }),
    client.readContract({ address: addr, abi: ERC20_ABI, functionName: 'name' }),
  ])
  const result = { symbol: symbol ?? '', name: name ?? '' }
  if (result.symbol) setCachedTokenInfo(`erc20:${evmAddress.toLowerCase()}`, result)
  return result
}

export function useEvmTokenInfo(
  evmAddress: string | undefined,
  rpcUrlOrConfig: string | BridgeChainConfig | undefined,
  enabled: boolean
) {
  const cached = evmAddress ? getCachedTokenInfo(`erc20:${evmAddress.toLowerCase()}`) : undefined
  return useQuery({
    queryKey: ['evmTokenInfo', evmAddress, rpcUrlOrConfig],
    queryFn: () =>
      evmAddress && rpcUrlOrConfig
        ? fetchEvmTokenInfo(evmAddress, rpcUrlOrConfig)
        : { symbol: '', name: '' },
    enabled: !!evmAddress && evmAddress.startsWith('0x') && !!rpcUrlOrConfig && enabled,
    staleTime: 5 * 60 * 1000,
    initialData: cached,
  })
}
