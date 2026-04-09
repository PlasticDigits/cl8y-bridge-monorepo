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
import { TerraTxError } from '../services/terra/transaction'
import { PublicKey, Transaction } from '@solana/web3.js'
import {
  buildWithdrawSubmitInstruction,
  formatSolanaUserFacingError,
  sendSolanaTransaction,
} from '../services/solana/transaction'
import { pickSolanaTxConnection } from '../services/solana/solanaRpcUrls'
import { useSolanaWalletStore } from '../stores/solanaWallet'

export interface WithdrawSubmitSolanaParams {
  rpcUrls: string[]
  programId: string
  srcChain: Uint8Array
  srcAccount: Uint8Array
  /** Remote source token id (32 bytes); must match TokenMapping PDA seeds. */
  srcToken: Uint8Array
  /** Solana SPL mint (base58) for the destination token. */
  destTokenMint: string
  amount: bigint
  nonce: bigint
  bridgeChainId: Uint8Array
  /** Lamports escrowed for operator; paid on approve (default 0). */
  operatorGas?: bigint
  /**
   * Recipient pubkey (base58); must match V2 `destAccount` on the transfer record.
   * When omitted, defaults to the connected wallet (same as payer).
   */
  destAccount?: string
}

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
        // Re-throw TerraTxError so callers can inspect the error code
        // (e.g. NONCE_ALREADY_APPROVED) and recover instead of showing a generic error.
        if (err instanceof TerraTxError) throw err
        return null
      }
    },
    []
  )

  /**
   * Submit withdrawal on a Solana destination chain.
   */
  const submitOnSolana = useCallback(
    async (params: WithdrawSubmitSolanaParams): Promise<string | null> => {
      setState({ status: 'submitting', txHash: null, error: null })
      try {
        const solanaWallet = useSolanaWalletStore.getState()
        if (!solanaWallet.address || !solanaWallet.walletType) {
          throw new Error('Solana wallet not connected')
        }
        const connection = await pickSolanaTxConnection(
          solanaWallet.walletType,
          params.rpcUrls,
        )
        const programId = new PublicKey(params.programId)
        const payer = new PublicKey(solanaWallet.address)
        const destAccount = new PublicKey(params.destAccount ?? solanaWallet.address)
        const destMint = new PublicKey(params.destTokenMint)

        const instruction = buildWithdrawSubmitInstruction(
          programId,
          payer,
          destAccount,
          params.srcChain,
          params.srcAccount,
          params.srcToken,
          destMint,
          params.amount,
          params.nonce,
          params.bridgeChainId,
          params.operatorGas ?? 0n,
        )

        const tx = new Transaction().add(instruction)
        const signature = await sendSolanaTransaction(connection, tx, solanaWallet.walletType)

        setState({ status: 'success', txHash: signature, error: null })
        return signature
      } catch (err) {
        const message = formatSolanaUserFacingError(err)
        const isRejection =
          message.toLowerCase().includes('rejected') || message.toLowerCase().includes('denied')
        const finalMessage = isRejection ? 'Transaction rejected by user' : message
        setState({
          status: 'error',
          txHash: null,
          error: finalMessage,
        })
        // Propagate so callers (e.g. useAutoWithdrawSubmit) surface the real failure instead of a generic null.
        throw new Error(finalMessage)
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
    submitOnSolana,
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
