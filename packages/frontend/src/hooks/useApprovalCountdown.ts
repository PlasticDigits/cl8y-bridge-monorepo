/**
 * useApprovalCountdown - Polls cancel window remaining when transfer is approved.
 *
 * Used by TransferStatusPage to show a countdown timer during the approval step.
 * Terra returns cancel_window_remaining from the pending_withdraw query.
 * EVM computes it from approvedAt + getCancelWindow - block.timestamp.
 */

import { useQuery } from '@tanstack/react-query'
import type { Hex } from 'viem'
import { getChainsForTransfer } from '../utils/bridgeChains'
import { BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'
import { getEvmClient } from '../services/evmClient'
import { queryEvmPendingWithdraw } from '../services/evmBridgeQueries'
import { queryTerraPendingWithdraw } from '../services/terraBridgeQueries'
import type { BridgeChainConfig } from '../types/chain'

const POLL_INTERVAL_MS = 1000

export function useApprovalCountdown(
  transferHash: Hex | undefined,
  destChainDisplayName: string | undefined,
  enabled: boolean
) {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = getChainsForTransfer()
  const destChainInfo = chains.find((c) => c.name === destChainDisplayName)
  const destChainKey = destChainInfo?.id
  const destChainConfig: BridgeChainConfig | undefined = destChainKey
    ? (BRIDGE_CHAINS[tier][destChainKey] as BridgeChainConfig)
    : undefined

  const { data: remainingSeconds } = useQuery({
    queryKey: ['approvalCountdown', transferHash, destChainKey],
    queryFn: async (): Promise<number | undefined> => {
      if (!transferHash || !destChainConfig?.bridgeAddress) return undefined

      if (destChainConfig.type === 'evm') {
        const client = getEvmClient(destChainConfig)
        const result = await queryEvmPendingWithdraw(
          client,
          destChainConfig.bridgeAddress as `0x${string}`,
          transferHash,
          destChainConfig.chainId as number
        )
        return result?.cancelWindowRemaining
      }

      if (destChainConfig.type === 'cosmos') {
        const lcdUrls = destChainConfig.lcdFallbacks ?? (destChainConfig.lcdUrl ? [destChainConfig.lcdUrl] : [])
        if (lcdUrls.length === 0) return undefined
        const result = await queryTerraPendingWithdraw(
          lcdUrls,
          destChainConfig.bridgeAddress,
          transferHash,
          destChainConfig
        )
        return result?.cancelWindowRemaining
      }

      return undefined
    },
    enabled: enabled && !!transferHash && !!destChainConfig?.bridgeAddress,
    refetchInterval: POLL_INTERVAL_MS,
  })

  return remainingSeconds
}
