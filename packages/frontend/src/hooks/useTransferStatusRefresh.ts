/**
 * useTransferStatusRefresh Hook
 *
 * Polls on-chain status for non-terminal transfers and updates localStorage
 * records when the lifecycle has advanced. Used by RecentTransfers to keep
 * the homepage status badges up-to-date without requiring the user to
 * visit the TransferStatusPage.
 */

import { useEffect, useRef, useCallback } from 'react'
import type { Address, Hex } from 'viem'
import type { TransferRecord, TransferLifecycle } from '../types/transfer'
import { useTransferStore } from '../stores/transfer'
import { BRIDGE_CHAINS, type NetworkTier } from '../utils/bridgeChains'
import { DEFAULT_NETWORK, POLLING_INTERVAL } from '../utils/constants'
import { getEvmClient } from '../services/evmClient'
import { BRIDGE_WITHDRAW_VIEW_ABI } from '../services/evm/withdrawSubmit'
import { queryTerraPendingWithdraw } from '../services/terraBridgeQueries'

const LIFECYCLE_RANK: Record<TransferLifecycle, number> = {
  deposited: 0,
  'hash-submitted': 1,
  approved: 2,
  executed: 3,
  failed: -1,
}

function isTerminal(lifecycle?: TransferLifecycle): boolean {
  return lifecycle === 'executed' || lifecycle === 'failed'
}

function shouldAdvance(current?: TransferLifecycle, candidate?: TransferLifecycle): boolean {
  if (!candidate) return false
  const cur = current ? LIFECYCLE_RANK[current] : -1
  const cand = LIFECYCLE_RANK[candidate]
  return cand > cur
}

async function queryOnChainLifecycle(
  transfer: TransferRecord
): Promise<TransferLifecycle | null> {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier]
  const destConfig = chains[transfer.destChain]
  if (!destConfig?.bridgeAddress || !transfer.xchainHashId) return null

  try {
    if (destConfig.type === 'evm') {
      const client = getEvmClient(destConfig)
      const result = (await client.readContract({
        address: destConfig.bridgeAddress as Address,
        abi: BRIDGE_WITHDRAW_VIEW_ABI,
        functionName: 'getPendingWithdraw',
        args: [transfer.xchainHashId as Hex],
      })) as {
        submittedAt: bigint
        approvedAt: bigint
        approved: boolean
        executed: boolean
      }

      if (result.executed) return 'executed'
      if (result.approved || result.approvedAt > 0n) return 'approved'
      if (result.submittedAt > 0n) return 'hash-submitted'
      return null
    }

    if (destConfig.type === 'cosmos') {
      const lcdUrls =
        destConfig.lcdFallbacks || (destConfig.lcdUrl ? [destConfig.lcdUrl] : [])
      if (lcdUrls.length === 0) return null

      const result = await queryTerraPendingWithdraw(
        lcdUrls,
        destConfig.bridgeAddress,
        transfer.xchainHashId as Hex,
        destConfig
      )
      if (result?.executed) return 'executed'
      if (result?.approved) return 'approved'
      if (result) return 'hash-submitted'
      return null
    }
  } catch {
    // RPC/LCD error — skip this cycle
  }

  return null
}

/**
 * Periodically refreshes on-chain status for non-terminal transfers.
 * Updates localStorage records when the lifecycle has advanced.
 *
 * @param transfers - The list of transfers to monitor
 * @param intervalMs - Polling interval (defaults to 2x POLLING_INTERVAL)
 */
export function useTransferStatusRefresh(
  transfers: TransferRecord[],
  intervalMs = POLLING_INTERVAL * 2
) {
  const { updateTransferRecord } = useTransferStore()
  const runningRef = useRef(false)

  const refreshAll = useCallback(async () => {
    if (runningRef.current) return
    runningRef.current = true

    try {
      const pending = transfers.filter(
        (t) => t.xchainHashId && !isTerminal(t.lifecycle)
      )
      if (pending.length === 0) return

      const results = await Promise.allSettled(
        pending.map((t) => queryOnChainLifecycle(t))
      )

      for (let i = 0; i < pending.length; i++) {
        const r = results[i]
        if (r.status !== 'fulfilled' || !r.value) continue
        const t = pending[i]
        if (shouldAdvance(t.lifecycle, r.value)) {
          updateTransferRecord(t.id, { lifecycle: r.value })
        }
      }
    } finally {
      runningRef.current = false
    }
  }, [transfers, updateTransferRecord])

  useEffect(() => {
    refreshAll()
    const id = setInterval(refreshAll, intervalMs)
    return () => clearInterval(id)
  }, [refreshAll, intervalMs])
}
