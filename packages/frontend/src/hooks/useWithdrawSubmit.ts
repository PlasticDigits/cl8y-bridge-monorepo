/**
 * useWithdrawSubmit Hook
 *
 * Provides a unified interface for calling withdrawSubmit on either
 * an EVM destination (via wagmi) or a Terra destination (via cosmes).
 *
 * Step 2 of the V2 bridge protocol.
 */

import { useState, useCallback } from 'react'
import { useWriteContract, useWaitForTransactionReceipt } from 'wagmi'
import {
  buildWithdrawSubmitArgs,
  type WithdrawSubmitEvmParams,
} from '../services/evm/withdrawSubmit'
import {
  submitWithdrawOnTerra,
  type WithdrawSubmitTerraParams,
} from '../services/terra/withdrawSubmit'

export type WithdrawSubmitStatus =
  | 'idle'
  | 'switching-chain'   // EVM->EVM: switching to dest chain
  | 'submitting'        // Sending the tx
  | 'waiting-receipt'   // Waiting for tx confirmation
  | 'success'
  | 'error'

export interface WithdrawSubmitState {
  status: WithdrawSubmitStatus
  txHash: string | null
  error: string | null
}

export function useWithdrawSubmit() {
  const [state, setState] = useState<WithdrawSubmitState>({
    status: 'idle',
    txHash: null,
    error: null,
  })

  const { writeContractAsync } = useWriteContract()

  // Track the pending EVM tx hash for receipt waiting
  const [pendingEvmTxHash, setPendingEvmTxHash] = useState<`0x${string}` | undefined>()

  const { isSuccess: isReceiptSuccess } = useWaitForTransactionReceipt({
    hash: pendingEvmTxHash,
    query: { enabled: !!pendingEvmTxHash && state.status === 'waiting-receipt' },
  })

  // When receipt confirms, update state
  if (isReceiptSuccess && state.status === 'waiting-receipt') {
    setState((prev) => ({ ...prev, status: 'success' }))
    setPendingEvmTxHash(undefined)
  }

  /**
   * Submit withdrawal on an EVM destination chain.
   */
  const submitOnEvm = useCallback(
    async (params: WithdrawSubmitEvmParams): Promise<string | null> => {
      setState({ status: 'submitting', txHash: null, error: null })

      try {
        const args = buildWithdrawSubmitArgs(params)
        const txHash = await writeContractAsync(args)

        setState({ status: 'waiting-receipt', txHash, error: null })
        setPendingEvmTxHash(txHash)
        return txHash
      } catch (err) {
        const message = err instanceof Error ? err.message : 'WithdrawSubmit failed'
        const isRejection =
          message.toLowerCase().includes('rejected') || message.toLowerCase().includes('denied')
        setState({
          status: 'error',
          txHash: null,
          error: isRejection ? 'Transaction rejected by user' : message,
        })
        return null
      }
    },
    [writeContractAsync]
  )

  /**
   * Submit withdrawal on a Terra destination chain.
   */
  const submitOnTerra = useCallback(
    async (params: WithdrawSubmitTerraParams): Promise<string | null> => {
      setState({ status: 'submitting', txHash: null, error: null })

      try {
        const { txHash } = await submitWithdrawOnTerra(params)
        setState({ status: 'success', txHash, error: null })
        return txHash
      } catch (err) {
        const message = err instanceof Error ? err.message : 'WithdrawSubmit failed'
        setState({ status: 'error', txHash: null, error: message })
        return null
      }
    },
    []
  )

  /**
   * Reset state to idle.
   */
  const reset = useCallback(() => {
    setState({ status: 'idle', txHash: null, error: null })
    setPendingEvmTxHash(undefined)
  }, [])

  return {
    ...state,
    submitOnEvm,
    submitOnTerra,
    reset,
    isLoading: state.status === 'submitting' || state.status === 'waiting-receipt' || state.status === 'switching-chain',
  }
}

// Re-export types for convenience
export { type WithdrawSubmitEvmParams } from '../services/evm/withdrawSubmit'
export { type WithdrawSubmitTerraParams } from '../services/terra/withdrawSubmit'

// Re-export ABIs for direct use
export { WITHDRAW_SUBMIT_ABI, BRIDGE_WITHDRAW_VIEW_ABI } from '../services/evm/withdrawSubmit'
export { hexToUint8Array, chainIdToBytes4, evmAddressToBytes32Array } from '../services/terra/withdrawSubmit'
