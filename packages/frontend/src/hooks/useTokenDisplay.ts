/**
 * useTokenDisplay - Unified hook for token logo + symbol display.
 * Never returns raw full address; uses shortenAddress or on-chain lookup.
 * Powers TokenDisplay component used throughout the app.
 */

import { useMemo } from 'react'
import { BRIDGE_CHAINS } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'
import type { NetworkTier } from '../utils/bridgeChains'
import { useTerraTokenDisplayInfo, useEvmTokenDisplayInfo } from './useTokenDisplayInfo'
import { getTokenDisplaySymbol } from '../utils/tokenLogos'
import { isAddressLike, shortenAddress } from '../utils/shortenAddress'

export interface TokenDisplayProps {
  /** Token identifier: denom (uluna), CW20 address, or EVM address */
  tokenId?: string | null
  /** Pre-resolved symbol (e.g. from TransferRecord.tokenSymbol) - avoids async lookup */
  symbol?: string | null
  /** Source chain id (anvil1, ethereum, etc.) - enables EVM on-chain symbol fetch */
  sourceChain?: string | null
}

export interface TokenDisplayResult {
  /** Human-readable label: symbol, shortened address, or empty */
  displayLabel: string
  /** For blockie fallback when no logo exists */
  addressForBlockie?: string
  /** Resolved symbol for TokenLogo */
  symbol: string
}

/**
 * Resolves token identity to display label + logo props.
 * Prefers provided symbol; falls back to Terra/EVM on-chain lookup or shortenAddress.
 * Never returns raw full address as displayLabel.
 */
export function useTokenDisplay({
  tokenId,
  symbol: symbolProp,
  sourceChain,
}: TokenDisplayProps): TokenDisplayResult {
  const isTerra = useMemo(
    () =>
      !!tokenId &&
      (tokenId === 'uluna' ||
        tokenId === 'uusd' ||
        tokenId.startsWith('terra1')),
    [tokenId]
  )
  const isEvm = useMemo(() => !!tokenId?.startsWith('0x'), [tokenId])
  const rpcUrl = useMemo(() => {
    if (!sourceChain) return undefined
    const tier = DEFAULT_NETWORK as NetworkTier
    const config = BRIDGE_CHAINS[tier]?.[sourceChain]
    return config?.type === 'evm' ? config.rpcUrl : undefined
  }, [sourceChain])

  const terraInfo = useTerraTokenDisplayInfo(isTerra ? tokenId ?? undefined : undefined)
  const evmInfo = useEvmTokenDisplayInfo(
    isEvm ? tokenId ?? undefined : undefined,
    rpcUrl ?? undefined,
    !!isEvm && !!rpcUrl
  )

  return useMemo((): TokenDisplayResult => {
    // Prefer provided symbol when it's human-readable
    if (symbolProp?.trim() && !isAddressLike(symbolProp)) {
      return {
        displayLabel: symbolProp.trim(),
        addressForBlockie: isEvm && tokenId ? tokenId : undefined,
        symbol: symbolProp.trim(),
      }
    }

    if (isTerra && tokenId) {
      return {
        displayLabel: terraInfo.displayLabel,
        addressForBlockie: terraInfo.addressForBlockie,
        symbol: terraInfo.symbol || terraInfo.displayLabel,
      }
    }

    if (isEvm && tokenId) {
      if (evmInfo.displayLabel) {
        return {
          displayLabel: evmInfo.displayLabel,
          addressForBlockie: evmInfo.addressForBlockie,
          symbol: evmInfo.symbol || evmInfo.displayLabel,
        }
      }
      // No on-chain symbol: show shortened address + blockie
      return {
        displayLabel: shortenAddress(tokenId),
        addressForBlockie: tokenId,
        symbol: shortenAddress(tokenId),
      }
    }

    // Unknown format
    const fallback = tokenId ? getTokenDisplaySymbol(tokenId) : ''
    const displayLabel =
      fallback && !isAddressLike(fallback)
        ? fallback
        : tokenId && isAddressLike(tokenId)
          ? shortenAddress(tokenId)
          : fallback || ''

    return {
      displayLabel,
      addressForBlockie: tokenId && isAddressLike(tokenId) ? tokenId : undefined,
      symbol: displayLabel || fallback,
    }
  }, [
    tokenId,
    symbolProp,
    isTerra,
    isEvm,
    terraInfo,
    evmInfo,
  ])
}
