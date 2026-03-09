/**
 * useApprovalCountdown - Polls cancel window remaining when transfer is approved.
 *
 * Used by TransferStatusPage to show a countdown timer during the approval step.
 * Also returns the on-chain `executed` flag so TransferStatusPage can advance the
 * lifecycle to 'executed' as soon as the operator completes the withdrawal.
 *
 * Terra returns cancel_window_remaining from the pending_withdraw query.
 * EVM computes it from approvedAt + getCancelWindow - block.timestamp.
 */

import { useQuery } from '@tanstack/react-query'
import type { Hex } from 'viem'
import { BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK } from '../utils/constants'
import { getEvmClient } from '../services/evmClient'
import { queryEvmPendingWithdraw } from '../services/evmBridgeQueries'
import { queryTerraPendingWithdraw } from '../services/terraBridgeQueries'
import type { BridgeChainConfig } from '../types/chain'

const POLL_INTERVAL_MS = 1000

interface ApprovalCountdownResult {
  cancelWindowRemaining: number | undefined
  executed: boolean
}

export function useApprovalCountdown(
  xchainHashId: Hex | undefined,
  destChainKey: string | undefined,
  enabled: boolean
): ApprovalCountdownResult {
  const tier = DEFAULT_NETWORK as NetworkTier
  const destChainConfig: BridgeChainConfig | undefined = destChainKey
    ? (BRIDGE_CHAINS[tier][destChainKey] as BridgeChainConfig)
    : undefined

  const { data } = useQuery({
    queryKey: ['approvalCountdown', xchainHashId, destChainKey],
    queryFn: async (): Promise<{ cancelWindowRemaining?: number; executed: boolean }> => {
      if (!xchainHashId || !destChainConfig?.bridgeAddress) {
        return { executed: false }
      }

      if (destChainConfig.type === 'evm') {
        const client = getEvmClient(destChainConfig)
        const result = await queryEvmPendingWithdraw(
          client,
          destChainConfig.bridgeAddress as `0x${string}`,
          xchainHashId,
          destChainConfig.chainId as number
        )
        return {
          cancelWindowRemaining: result?.cancelWindowRemaining,
          executed: result?.executed ?? false,
        }
      }

      if (destChainConfig.type === 'cosmos') {
        const lcdUrls = destChainConfig.lcdFallbacks ?? (destChainConfig.lcdUrl ? [destChainConfig.lcdUrl] : [])
        if (lcdUrls.length === 0) return { executed: false }
        const result = await queryTerraPendingWithdraw(
          lcdUrls,
          destChainConfig.bridgeAddress,
          xchainHashId,
          destChainConfig
        )
        return {
          cancelWindowRemaining: result?.cancelWindowRemaining,
          executed: result?.executed ?? false,
        }
      }

      return { executed: false }
    },
    enabled: enabled && !!xchainHashId && !!destChainConfig?.bridgeAddress,
    refetchInterval: POLL_INTERVAL_MS,
  })

  return {
    cancelWindowRemaining: data?.cancelWindowRemaining,
    executed: data?.executed ?? false,
  }
}
