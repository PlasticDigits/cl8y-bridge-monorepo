/**
 * useTokenOnchainInfo - Fetches token symbol/name from chain when not in tokenlist.
 * Supports Terra CW20 (token_info) and EVM ERC20 (symbol/name).
 */

import { useQuery } from '@tanstack/react-query'
import { queryContract } from '../services/lcdClient'
import { NETWORKS, DEFAULT_NETWORK } from '../utils/constants'
import { getAddress } from 'viem'
import { createPublicClient, http } from 'viem'

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
  return { symbol: res?.symbol ?? '', name: res?.name ?? '' }
}

export function useCw20TokenInfo(terraAddress: string | undefined, enabled: boolean) {
  return useQuery({
    queryKey: ['cw20TokenInfo', terraAddress],
    queryFn: () => (terraAddress ? fetchCw20TokenInfo(terraAddress) : { symbol: '', name: '' }),
    enabled: !!terraAddress && terraAddress.startsWith('terra1') && enabled,
    staleTime: 5 * 60 * 1000,
  })
}

/** Fetcher for EVM token symbol/name - used by useEvmTokenInfo and useQueries batch */
export async function fetchEvmTokenInfo(
  evmAddress: string,
  rpcUrl: string
): Promise<{ symbol: string; name: string }> {
  if (!evmAddress?.startsWith('0x')) return { symbol: '', name: '' }
  const addr = getAddress(evmAddress)
  const client = createPublicClient({ transport: http(rpcUrl) })
  const [symbol, name] = await Promise.all([
    client.readContract({ address: addr, abi: ERC20_ABI, functionName: 'symbol' }),
    client.readContract({ address: addr, abi: ERC20_ABI, functionName: 'name' }),
  ])
  return { symbol: symbol ?? '', name: name ?? '' }
}

export function useEvmTokenInfo(
  evmAddress: string | undefined,
  rpcUrl: string,
  enabled: boolean
) {
  return useQuery({
    queryKey: ['evmTokenInfo', evmAddress, rpcUrl],
    queryFn: () =>
      evmAddress && rpcUrl ? fetchEvmTokenInfo(evmAddress, rpcUrl) : { symbol: '', name: '' },
    enabled: !!evmAddress && evmAddress.startsWith('0x') && !!rpcUrl && enabled,
    staleTime: 5 * 60 * 1000,
  })
}
