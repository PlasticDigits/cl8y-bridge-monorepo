/**
 * useHashMonitor Hook
 *
 * Fetches all transfer hashes from deposits and withdraws via RPC/LCD,
 * merges with localStorage verification records, and provides paginated data.
 */

import { useCallback, useEffect, useState } from 'react'
import { fetchAllXchainHashIds, type MonitorHashEntry } from '../services/hashMonitor'
import { getVerificationRecords } from '../components/verify/RecentVerifications'
import type { HashStatus } from '../types/transfer'

const PAGE_SIZE = 20

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
      status: storedData?.status ?? inferStatus(e),
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

  useEffect(() => {
    refresh()
  }, [refresh])

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
    error,
    pageSize: PAGE_SIZE,
    refresh,
  }
}
