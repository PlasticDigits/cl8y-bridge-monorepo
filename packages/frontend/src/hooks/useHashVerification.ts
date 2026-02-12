/**
 * useHashVerification Hook
 *
 * Orchestrates transfer lookup and comparison. Fetches source/dest data via
 * useMultiChainLookup, computes expected hash, and determines status.
 */

import { useCallback, useState } from 'react'
import {
  computeTransferHash,
  normalizeHash,
} from '../services/hashVerification'
import { useMultiChainLookup } from './useMultiChainLookup'
import type { DepositData, PendingWithdrawData } from './useTransferLookup'
import type { HashStatus } from '../types/transfer'
import type { BridgeChainConfig } from '../types/chain'

export interface HashVerificationState {
  inputHash: string | null
  hash: string | null
  status: HashStatus
  source: DepositData | null
  sourceChain: BridgeChainConfig | null
  dest: PendingWithdrawData | null
  destChain: BridgeChainConfig | null
  computedHash: string | null
  matches: boolean | null
  loading: boolean
  error: string | null
  queriedChains: string[]
  failedChains: string[]
}

export function useHashVerification() {
  const {
    source,
    sourceChain,
    dest,
    destChain,
    queriedChains,
    failedChains,
    loading,
    error,
    lookup,
  } = useMultiChainLookup()
  const [inputHash, setInputHash] = useState<string | null>(null)

  const verify = useCallback(
    async (hashInput: string) => {
      const hash = normalizeHash(hashInput) as `0x${string}`
      setInputHash(hash)
      await lookup(hash)
    },
    [lookup]
  )

  // Compute hash directly from srcChain/destChain bytes32 already present on the data.
  // This avoids re-deriving from numeric chainId, which is 0 for Terra chains.
  const computedHash = (() => {
    if (source) {
      return computeTransferHash(
        source.srcChain,
        source.destChain,
        source.srcAccount,
        source.destAccount,
        source.token,
        source.amount,
        BigInt(source.nonce)
      )
    }
    if (dest) {
      return computeTransferHash(
        dest.srcChain,
        dest.destChain,
        dest.srcAccount,
        dest.destAccount,
        dest.token,
        dest.amount,
        BigInt(dest.nonce)
      )
    }
    return null
  })()

  const matches: boolean | null = (() => {
    if (!computedHash || !source || !dest) return null
    // Compare source and dest by recomputing from both and checking equality
    const fromSource = computeTransferHash(
      source.srcChain,
      source.destChain,
      source.srcAccount,
      source.destAccount,
      source.token,
      source.amount,
      BigInt(source.nonce)
    )
    const fromDest = computeTransferHash(
      dest.srcChain,
      dest.destChain,
      dest.srcAccount,
      dest.destAccount,
      dest.token,
      dest.amount,
      BigInt(dest.nonce)
    )
    return fromSource === fromDest
  })()

  const status: HashStatus = (() => {
    if (error) return 'unknown'
    if (loading) return 'pending'
    if (dest?.cancelled) return 'canceled'
    if (dest?.executed) return 'verified'
    if (dest?.approved) return 'pending' // approved, awaiting execution
    if (dest && !dest.approved && !dest.cancelled) return 'pending'
    if (source && !dest) return 'pending' // deposit found, no withdraw yet
    if (!source && !dest) return 'unknown'
    return 'pending'
  })()

  return {
    inputHash,
    hash: source || dest ? (computedHash ?? null) : null,
    status,
    source,
    sourceChain,
    dest,
    destChain,
    computedHash,
    matches,
    loading,
    error,
    queriedChains,
    failedChains,
    verify,
  }
}
