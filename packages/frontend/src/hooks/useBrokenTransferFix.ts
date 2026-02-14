/**
 * useBrokenTransferFix Hook
 *
 * When a transfer has a pending withdraw but no matching deposit (broken hash),
 * attempts to detect the swap-bug pattern and provide fix params for resubmitting
 * withdrawSubmit on the correct chain.
 */

import { useState, useCallback, useEffect } from 'react'
import type { Hex } from 'viem'
import { detectAndGetFix, isLikelyBroken, type BrokenTransferFix } from '../services/brokenTransferFix'
import { getChainKeyByConfig } from '../utils/bridgeChains'
import type { DepositData, PendingWithdrawData } from './useTransferLookup'
import type { BridgeChainConfig } from '../types/chain'

export interface UseBrokenTransferFixResult {
  isBroken: boolean
  fix: BrokenTransferFix | null
  loading: boolean
  error: string | null
  retry: () => void
}

export function useBrokenTransferFix(
  hash: Hex | undefined,
  source: DepositData | null,
  dest: PendingWithdrawData | null,
  destChain: BridgeChainConfig | null
): UseBrokenTransferFixResult {
  const destChainKey = destChain ? getChainKeyByConfig(destChain) ?? null : null
  const [fix, setFix] = useState<BrokenTransferFix | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const runDetection = useCallback(async () => {
    if (!hash || !dest || !destChain || !destChainKey) {
      setFix(null)
      return
    }

    if (!isLikelyBroken(source, dest)) {
      setFix(null)
      setError(null)
      return
    }

    setLoading(true)
    setError(null)

    try {
      const result = await detectAndGetFix(hash, dest, destChain, destChainKey)
      setFix(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Detection failed')
      setFix(null)
    } finally {
      setLoading(false)
    }
  }, [hash, source, dest, destChain, destChainKey])

  useEffect(() => {
    runDetection()
  }, [runDetection])

  return {
    isBroken: isLikelyBroken(source, dest),
    fix,
    loading,
    error,
    retry: runDetection,
  }
}
