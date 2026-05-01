/**
 * EVM destination: rate-limit execution block detection for Transfer Status (GL-127).
 * Uses the same TokenRegistry withdraw window as Settings / Transfer form.
 */

import { useMemo } from 'react'
import type { Hex } from 'viem'
import { bytes32ToAddress } from '../services/evm/tokenRegistry'
import { useTokenDetails, type UnifiedBridgeConfig } from './useBridgeConfig'
import type { PendingWithdrawData } from './useTransferLookup'
import type { BridgeChainConfig } from '../types/chain'
import { BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'
import { computeEvmExecutionRateLimitStatus } from '../services/evmExecutionRateLimit'
import type { TerraRateLimitStatus } from '../services/terraBridgeQueries'

function toUnifiedEvmConfig(chain: BridgeChainConfig): UnifiedBridgeConfig | null {
  if (chain.type !== 'evm' || !chain.bridgeAddress) return null
  const tier = DEFAULT_NETWORK as NetworkTier
  const match = (Object.entries(BRIDGE_CHAINS[tier]) as [string, BridgeChainConfig][]).find(
    ([, c]) => c.chainId === chain.chainId && c.type === 'evm',
  )
  const chainId = match?.[0] ?? String(chain.chainId)
  return {
    chainId,
    chainName: chain.name,
    type: 'evm',
    cancelWindowSeconds: null,
    feeBps: null,
    feeCollector: null,
    admin: null,
    loaded: true,
    chainConfig: chain,
    bridgeAddress: chain.bridgeAddress,
  }
}

export function useEvmExecutionRateLimitStatus(
  dest: PendingWithdrawData | null,
  destChain: BridgeChainConfig | null,
  enabled: boolean,
): { data: TerraRateLimitStatus | null; isLoading: boolean } {
  const unified = useMemo(
    () => (destChain?.type === 'evm' ? toUnifiedEvmConfig(destChain) : null),
    [destChain],
  )

  const tokenAddr = useMemo(() => {
    if (!dest?.token || !dest.token.startsWith('0x') || dest.token.length !== 66) return null
    try {
      return bytes32ToAddress(dest.token as Hex)
    } catch {
      return null
    }
  }, [dest?.token])

  const tokenQueryEnabled =
    enabled && !!unified && !!tokenAddr && !!dest && dest.approved && !dest.executed

  const { data: tokenDetails, isLoading } = useTokenDetails(unified, tokenAddr, tokenQueryEnabled)

  const data = useMemo((): TerraRateLimitStatus | null => {
    if (!tokenQueryEnabled || !dest) return null
    return computeEvmExecutionRateLimitStatus(dest, tokenDetails?.withdrawRateLimit ?? null)
  }, [tokenQueryEnabled, dest, tokenDetails?.withdrawRateLimit])

  return { data, isLoading }
}
