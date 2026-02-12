/**
 * useTerraDeposit - Terra → EVM lock flow
 *
 * Extracts Terra lock logic from BridgeForm. Executes lock on Terra bridge
 * with native uluna coins.
 */

import { useState, useCallback } from 'react'
import { executeContractWithCoins } from '../services/terra'
import { CONTRACTS, DEFAULT_NETWORK } from '../utils/constants'
import { useTransferStore } from '../stores/transfer'
export type TerraDepositStatus = 'idle' | 'locking' | 'success' | 'error'

export interface UseTerraDepositReturn {
  status: TerraDepositStatus
  txHash: string | null
  error: string | null
  lock: (params: {
    amountMicro: string
    destChainId: number
    recipientEvm: string
  }) => Promise<string | null>
  reset: () => void
}

export function useTerraDeposit(): UseTerraDepositReturn {
  const [status, setStatus] = useState<TerraDepositStatus>('idle')
  const [txHash, setTxHash] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const { setActiveTransfer, updateActiveTransfer, recordTransfer } = useTransferStore()

  const lock = useCallback(
    async ({
      amountMicro,
      destChainId,
      recipientEvm,
    }: {
      amountMicro: string
      destChainId: number
      recipientEvm: string
    }): Promise<string | null> => {
      const bridgeAddress = CONTRACTS[DEFAULT_NETWORK].terraBridge
      if (!bridgeAddress) {
        const err = 'Terra bridge address not configured'
        setError(err)
        setStatus('error')
        return null
      }

      setStatus('locking')
      setError(null)
      setTxHash(null)

      const transferId = `terra-lock-${Date.now()}`
      setActiveTransfer({
        id: transferId,
        direction: 'terra-to-evm',
        sourceChain: 'terra',
        destChain: destChainId === 31337 ? 'anvil' : destChainId === 56 ? 'bsc' : destChainId === 204 ? 'opbnb' : 'ethereum',
        amount: amountMicro,
        status: 'pending',
        txHash: null,
        recipient: recipientEvm,
        startedAt: Date.now(),
      })

      try {
        const lockMsg = {
          lock: {
            dest_chain_id: destChainId,
            recipient: recipientEvm,
          },
        }

        const result = await executeContractWithCoins(bridgeAddress, lockMsg, [
          { denom: 'uluna', amount: amountMicro },
        ])

        setTxHash(result.txHash)
        setStatus('success')
        updateActiveTransfer({ txHash: result.txHash, status: 'confirmed' })
        recordTransfer({
          type: 'withdrawal',
          direction: 'terra-to-evm',
          sourceChain: 'terra',
          destChain: String(destChainId),
          amount: amountMicro,
          status: 'confirmed',
          txHash: result.txHash,
        })
        setActiveTransfer(null)
        return result.txHash
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Lock failed'
        setError(msg)
        setStatus('error')
        updateActiveTransfer({ status: 'failed' })
        setActiveTransfer(null)
        return null
      }
    },
    [setActiveTransfer, updateActiveTransfer, recordTransfer]
  )

  const reset = useCallback(() => {
    setStatus('idle')
    setTxHash(null)
    setError(null)
  }, [])

  return { status, txHash, error, lock, reset }
}
