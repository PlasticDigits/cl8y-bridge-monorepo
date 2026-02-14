/**
 * useHashVerification Hook
 *
 * Orchestrates transfer lookup and comparison. Fetches source/dest data via
 * useMultiChainLookup, computes expected hash, and determines status.
 */

import { useCallback, useState } from 'react'
import type { Hex } from 'viem'
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

  const ZERO_BYTES32 = ('0x' + '0'.repeat(64)) as Hex

  // Compute hash from the best available data.
  // Prefer dest (always has correct chain IDs from the EVM/Terra contract).
  // For Terra deposits, destChain is zero (not stored in deposit record);
  // fill it from dest if available.
  const computedHash = (() => {
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
    if (source && source.destChain !== ZERO_BYTES32) {
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
    return null
  })()

  const matches: boolean | null = (() => {
    if (!source || !dest) return null
    // When both sides are found, compare field by field.
    // Some fields may be zero on one side (e.g. Terra deposits don't store destChain),
    // so skip zero fields in the comparison.
    const srcChainOk = source.srcChain === ZERO_BYTES32 || dest.srcChain === ZERO_BYTES32 || source.srcChain === dest.srcChain
    const destChainOk = source.destChain === ZERO_BYTES32 || dest.destChain === ZERO_BYTES32 || source.destChain === dest.destChain
    const srcAccountMatch = source.srcAccount === dest.srcAccount
    const destAccountMatch = source.destAccount === dest.destAccount
    const tokenMatch = source.token === dest.token
    const amountMatch = source.amount === dest.amount
    const nonceMatch = source.nonce === dest.nonce
    return srcChainOk && destChainOk && srcAccountMatch && destAccountMatch && tokenMatch && amountMatch && nonceMatch
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
