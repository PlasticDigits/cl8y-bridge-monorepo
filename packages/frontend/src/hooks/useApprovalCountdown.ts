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
import { querySolanaPendingWithdraw } from '../services/solana/solanaBridgeQueries'
import type { BridgeChainConfig } from '../types/chain'

const POLL_INTERVAL_MS = 1000

interface ApprovalCountdownResult {
  cancelWindowRemaining: number | undefined
  executed: boolean
}

export function useApprovalCountdown(
  xchainHashId: Hex | undefined,
  destChainKey: string | undefined,
  enabled: boolean,
  /** Solana / fallback: cancel window length in seconds (bridge withdraw_delay). */
  cancelWindowSeconds?: number | null,
): ApprovalCountdownResult {
  const tier = DEFAULT_NETWORK as NetworkTier
  const destChainConfig: BridgeChainConfig | undefined = destChainKey
    ? (BRIDGE_CHAINS[tier][destChainKey] as BridgeChainConfig)
    : undefined

  const { data } = useQuery({
    queryKey: ['approvalCountdown', xchainHashId, destChainKey, cancelWindowSeconds],
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

      if (destChainConfig.type === 'solana') {
        const pw = await querySolanaPendingWithdraw(destChainConfig, xchainHashId)
        if (!pw) return { executed: false }
        const w = cancelWindowSeconds ?? 300
        const now = Math.floor(Date.now() / 1000)
        const approvedAt = Number(pw.approvedAt)
        let cancelWindowRemaining: number | undefined
        if (pw.approved && approvedAt > 0 && !pw.executed) {
          cancelWindowRemaining = Math.max(0, approvedAt + w - now)
        }
        return {
          cancelWindowRemaining,
          executed: pw.executed,
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
