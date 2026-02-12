/**
 * useTransferLookup Hook
 *
 * @deprecated Use useMultiChainLookup instead for multi-chain support.
 * This hook is kept for backward compatibility and delegates to useMultiChainLookup.
 *
 * Fetches source (deposit) and destination (pending withdraw) data for a given
 * transfer hash via RPC. Queries bridge contracts directly - no operator API.
 */

import type { Hex } from 'viem'
import { useMultiChainLookup } from './useMultiChainLookup'

// Re-export types for backward compatibility

export interface DepositData {
  chainId: number
  srcChain: Hex
  destChain: Hex
  srcAccount: Hex
  destAccount: Hex
  token: Hex
  amount: bigint
  nonce: bigint
  timestamp: bigint
}

export interface PendingWithdrawData {
  chainId: number
  srcChain: Hex
  destChain: Hex
  srcAccount: Hex
  destAccount: Hex
  token: Hex
  amount: bigint
  nonce: bigint
  submittedAt: bigint
  approvedAt: bigint
  approved: boolean
  cancelled: boolean
  executed: boolean
}

export interface TransferLookupResult {
  source: DepositData | null
  dest: PendingWithdrawData | null
  loading: boolean
  error: string | null
}

/**
 * @deprecated Use useMultiChainLookup instead.
 * This hook delegates to useMultiChainLookup but returns the old interface.
 */
export function useTransferLookup() {
  const multiChain = useMultiChainLookup()

  return {
    source: multiChain.source,
    dest: multiChain.dest,
    loading: multiChain.loading,
    error: multiChain.error,
    lookup: multiChain.lookup,
  }
}
