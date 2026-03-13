/**
 * useHashMonitor Hook
 *
 * Fetches all transfer hashes from deposits and withdraws via RPC/LCD,
 * merges with localStorage verification records, and provides paginated data.
 *
 * Periodically rechecks pending hashes against on-chain state so that
 * entries whose withdrawals have been executed/cancelled are updated
 * without requiring a full page refresh.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { fetchAllXchainHashIds, recheckPendingHashes, type MonitorHashEntry } from '../services/hashMonitor'
import { getVerificationRecords } from '../components/verify/RecentVerifications'
import type { HashStatus } from '../types/transfer'
import type { Hex } from 'viem'

const PAGE_SIZE = 20

/** How often to recheck pending hashes against on-chain state (ms). */
const RECHECK_INTERVAL_MS = 30_000

export interface HashMonitorRecord {
  hash: string
  status?: HashStatus
  sourceChain?: string
  destChain?: string
  matches?: boolean
  cancelled?: boolean
  timestamp: number
  source: 'deposit' | 'withdraw'
  chainName: string
  approved?: boolean
  executed?: boolean
}

function mergeWithVerificationRecords(
  entries: MonitorHashEntry[]
): HashMonitorRecord[] {
  const stored = getVerificationRecords()
  const byHash = new Map<string, { status?: HashStatus; sourceChain?: string; destChain?: string; matches?: boolean; cancelled?: boolean; timestamp: number }>()
  for (const r of stored) {
    byHash.set(r.hash.toLowerCase(), {
      status: r.status,
      sourceChain: r.sourceChain,
      destChain: r.destChain,
      matches: r.matches,
      cancelled: r.cancelled,
      timestamp: r.timestamp,
    })
  }

  const records: HashMonitorRecord[] = entries.map((e) => {
    const storedData = byHash.get(e.hash.toLowerCase())
    return {
      hash: e.hash,
      status: inferStatus(e) !== 'pending' ? inferStatus(e) : (storedData?.status ?? 'pending'),
      sourceChain: storedData?.sourceChain,
      destChain: storedData?.destChain,
      matches: storedData?.matches,
      cancelled: storedData?.cancelled ?? e.cancelled,
      timestamp: storedData?.timestamp ?? (e.timestamp ? e.timestamp * 1000 : 0),
      source: e.source,
      chainName: e.chainName,
      approved: e.approved,
      executed: e.executed,
    }
  })

  return records
}

function inferStatus(entry: MonitorHashEntry): HashStatus {
  if (entry.executed) return 'verified'
  if (entry.cancelled) return 'canceled'
  if (entry.approved) return 'pending'
  if (entry.source === 'deposit') return 'pending'
  return 'pending'
}

export function useHashMonitor() {
  const [records, setRecords] = useState<HashMonitorRecord[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [rechecking, setRechecking] = useState(false)
  const recheckTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const entries = await fetchAllXchainHashIds()
      const merged = mergeWithVerificationRecords(entries)
      setRecords(merged)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch hashes')
      setRecords([])
    } finally {
      setLoading(false)
    }
  }, [])

  /**
   * Recheck only the pending hashes against on-chain state.
   * Updates records in place without a full refetch.
   */
  const recheckPending = useCallback(async () => {
    setRecords((prev) => {
      const pending = prev.filter((r) => r.status === 'pending')
      if (pending.length === 0) return prev

      const pendingHashes = pending.map((r) => r.hash as Hex)
      setRechecking(true)

      recheckPendingHashes(pendingHashes)
        .then((updates) => {
          if (updates.size === 0) return

          setRecords((current) =>
            current.map((r) => {
              const update = updates.get(r.hash.toLowerCase())
              if (!update) return r

              if (update.notFound) {
                return { ...r, status: 'unknown' as HashStatus }
              }

              let newStatus: HashStatus = r.status ?? 'pending'
              if (update.executed) newStatus = 'verified'
              else if (update.cancelled) newStatus = 'canceled'

              return {
                ...r,
                status: newStatus,
                executed: update.executed ?? r.executed,
                cancelled: update.cancelled ?? r.cancelled,
                approved: update.approved ?? r.approved,
                timestamp: update.timestamp ? update.timestamp * 1000 : r.timestamp,
              }
            })
          )
        })
        .catch(() => {
          // Recheck failed silently; will retry next interval
        })
        .finally(() => {
          setRechecking(false)
        })

      return prev
    })
  }, [])

  // Initial fetch
  useEffect(() => {
    refresh()
  }, [refresh])

  // Periodic recheck of pending hashes
  useEffect(() => {
    recheckTimerRef.current = setInterval(() => {
      recheckPending()
    }, RECHECK_INTERVAL_MS)

    return () => {
      if (recheckTimerRef.current) {
        clearInterval(recheckTimerRef.current)
      }
    }
  }, [recheckPending])

  // Listen for manual verification events from the verify page
  useEffect(() => {
    const handler = () => {
      setRecords((prev) => {
        if (prev.length === 0) return prev
        return mergeWithVerificationRecords(
          prev.map((r) => ({
            hash: r.hash as `0x${string}`,
            source: r.source,
            chainKey: r.chainName,
            chainName: r.chainName,
            timestamp: r.timestamp ? Math.floor(r.timestamp / 1000) : undefined,
            cancelled: r.cancelled,
            approved: r.approved,
            executed: r.executed,
          }))
        )
      })
    }
    window.addEventListener('cl8y-verification-recorded', handler)
    return () => window.removeEventListener('cl8y-verification-recorded', handler)
  }, [])

  return {
    allRecords: records,
    loading,
    rechecking,
    error,
    pageSize: PAGE_SIZE,
    refresh,
    recheckPending,
  }
}
