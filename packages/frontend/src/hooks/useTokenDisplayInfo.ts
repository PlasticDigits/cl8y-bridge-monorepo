/**
 * useTokenDisplayInfo - Shared hook for friendly token display.
 * Combines tokenlist, onchain queries (CW20/ERC20), blockie fallback, and shortened addresses.
 * Used by TokenSelect, FeeBreakdown, TokenCard, etc.
 */

import { useMemo } from 'react'
import { useQueries } from '@tanstack/react-query'
import { useTokenList } from './useTokenList'
import { useCw20TokenInfo, useEvmTokenInfo, fetchEvmTokenInfo, fetchCw20TokenInfo, getCachedTokenInfo } from './useTokenOnchainInfo'
import { getTokenFromList } from '../services/tokenlist'
import type { BridgeChainConfig } from '../types/chain'
import { isAddressLike, shortenAddress } from '../utils/shortenAddress'
import { getTokenDisplaySymbol } from '../utils/tokenLogos'

export interface TokenDisplayInfo {
  displayLabel: string
  symbol: string
  name?: string
  addressForBlockie?: string
  hasLogo: boolean
}

/**
 * Get display info for a Terra token (denom or CW20 address).
 */
export function useTerraTokenDisplayInfo(tokenId: string | undefined) {
  const { data: tokenlist } = useTokenList()
  const isCw20 = tokenId?.startsWith('terra1')
  const { data: cw20Info } = useCw20TokenInfo(tokenId, !!isCw20)

  return useMemo((): TokenDisplayInfo => {
    if (!tokenId) return { displayLabel: '', symbol: '', hasLogo: false }

    const fromList = tokenlist ? getTokenFromList(tokenlist, tokenId) : null
    const fromOnchain = cw20Info
    const symbol = fromList?.symbol ?? fromOnchain?.symbol ?? getTokenDisplaySymbol(tokenId)
    const name = fromList?.name ?? fromOnchain?.name
    const hasLogo = !!(fromList?.symbol || tokenId === 'uluna' || tokenId === 'uusd')
    const addressForBlockie = isCw20 && !hasLogo ? tokenId : undefined
    const displayLabel =
      symbol && !isAddressLike(symbol)
        ? symbol
        : isAddressLike(tokenId)
          ? shortenAddress(tokenId)
          : tokenId

    return {
      displayLabel,
      symbol: symbol || displayLabel,
      name,
      addressForBlockie,
      hasLogo,
    }
  }, [tokenId, tokenlist, cw20Info])
}

/**
 * Get display info for an EVM token address.
 * Prefer chainConfig (enables RPC fallbacks) over rpcUrl.
 */
export function useEvmTokenDisplayInfo(
  evmAddress: string | undefined,
  rpcUrlOrConfig: string | BridgeChainConfig | undefined,
  enabled: boolean
) {
  const { data: evmInfo } = useEvmTokenInfo(evmAddress, rpcUrlOrConfig, enabled)

  return useMemo((): TokenDisplayInfo => {
    if (!evmAddress || !evmAddress.startsWith('0x'))
      return { displayLabel: '', symbol: '', hasLogo: false }

    const fromOnchain = evmInfo
    const symbol = fromOnchain?.symbol ?? ''
    const name = fromOnchain?.name ?? ''
    const displayLabel =
      symbol || (name && !isAddressLike(name))
        ? symbol || name
        : shortenAddress(evmAddress)

    return {
      displayLabel,
      symbol: symbol || displayLabel,
      name: name || undefined,
      addressForBlockie: evmAddress,
      hasLogo: false,
    }
  }, [evmAddress, evmInfo])
}

/**
 * Resolve display labels for token select options (dropdown).
 * Uses tokenlist + CW20 onchain for Terra, ERC20 onchain for EVM when sourceChainConfig provided.
 * Prefer sourceChainConfig (enables RPC fallbacks) over sourceChainRpcUrl.
 */
export function useTokenOptionsDisplayMap(
  tokens: Array<{ id: string; symbol: string; tokenId: string; evmTokenAddress?: string }>,
  /** Chain config (enables RPC fallbacks) or single rpcUrl for backward compat */
  sourceChainConfigOrRpcUrl?: BridgeChainConfig | string
): Record<string, string> {
  const { data: tokenlist } = useTokenList()
  const terraCw20Tokens = tokens.filter((t) => t.tokenId?.startsWith('terra1'))
  const evmTokens = tokens.filter((t) => t.evmTokenAddress && t.evmTokenAddress.startsWith('0x'))
  const evmRpcOrConfig = sourceChainConfigOrRpcUrl

  const cw20Results = useQueries({
    queries: terraCw20Tokens.map((t) => {
      const cached = getCachedTokenInfo(`cw20:${t.tokenId}`)
      return {
        queryKey: ['cw20TokenInfo', t.tokenId],
        queryFn: async () => {
          const res = await fetchCw20TokenInfo(t.tokenId)
          return { tokenId: t.tokenId, symbol: res.symbol, name: res.name }
        },
        enabled: !!t.tokenId,
        staleTime: 5 * 60 * 1000,
        initialData: cached ? { tokenId: t.tokenId, symbol: cached.symbol, name: cached.name } : undefined,
      }
    }),
  })

  const evmResults = useQueries({
    queries: evmTokens.map((t) => {
      const cached = t.evmTokenAddress ? getCachedTokenInfo(`erc20:${t.evmTokenAddress.toLowerCase()}`) : undefined
      return {
        queryKey: ['evmTokenInfo', t.evmTokenAddress, evmRpcOrConfig],
        queryFn: () =>
          t.evmTokenAddress && evmRpcOrConfig
            ? fetchEvmTokenInfo(t.evmTokenAddress, evmRpcOrConfig)
            : { symbol: '', name: '' },
        enabled: !!t.evmTokenAddress && !!evmRpcOrConfig,
        staleTime: 5 * 60 * 1000,
        initialData: cached,
      }
    }),
  })

  return useMemo(() => {
    const map: Record<string, string> = {}
    for (const t of tokens) {
      const fromList = tokenlist ? getTokenFromList(tokenlist, t.id) : null
      if (fromList?.symbol && !isAddressLike(fromList.symbol)) {
        map[t.id] = fromList.symbol
        continue
      }
      if (t.tokenId?.startsWith('terra1')) {
        const idx = terraCw20Tokens.findIndex((x) => x.tokenId === t.tokenId)
        const data = cw20Results[idx]?.data
        const symbol = data?.symbol ?? ''
        if (symbol) {
          map[t.id] = symbol
          continue
        }
      }
      if (t.evmTokenAddress && evmRpcOrConfig) {
        const idx = evmTokens.findIndex((x) => x.evmTokenAddress === t.evmTokenAddress)
        const data = evmResults[idx]?.data
        const symbol = data?.symbol ?? (data?.name && !isAddressLike(data.name) ? data.name : '')
        if (symbol) {
          map[t.id] = symbol
          continue
        }
      }
      const fallback = getTokenDisplaySymbol(t.tokenId)
      map[t.id] =
        fallback && !isAddressLike(fallback) ? fallback : isAddressLike(t.symbol) ? shortenAddress(t.symbol) : t.symbol
    }
    return map
  }, [tokens, tokenlist, terraCw20Tokens, cw20Results, evmTokens, evmResults, evmRpcOrConfig])
}

/**
 * Batch fetch display info for multiple EVM token addresses.
 * Returns a map of address (lowercase) -> TokenDisplayInfo for use in TokenCard chains list.
 */
export function useEvmTokensDisplayInfo(
  items: Array<{ address: string; rpcUrl?: string }>
) {
  const evmItems = items.filter((i) => i.address?.startsWith('0x') && i.rpcUrl)
  const results = useQueries({
    queries: evmItems.map(({ address, rpcUrl }) => ({
      queryKey: ['evmTokenInfo', address, rpcUrl],
      queryFn: () => fetchEvmTokenInfo(address, rpcUrl!),
      enabled: true,
      staleTime: 5 * 60 * 1000,
    })),
  })
  return useMemo(() => {
    const map: Record<string, TokenDisplayInfo> = {}
    evmItems.forEach((item, i) => {
      const data = results[i]?.data
      const symbol = data?.symbol ?? ''
      const name = data?.name ?? ''
      const displayLabel =
        symbol || (name && !isAddressLike(name))
          ? symbol || name
          : shortenAddress(item.address)
      map[item.address.toLowerCase()] = {
        displayLabel,
        symbol: symbol || displayLabel,
        name: name || undefined,
        addressForBlockie: item.address,
        hasLogo: false,
      }
    })
    return map
  }, [evmItems, results])
}
