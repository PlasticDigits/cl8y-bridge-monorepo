import { useCallback, useEffect, useState } from 'react'
import type { HashStatus } from '../../types/transfer'

const STORAGE_KEY = 'cl8y-bridge-verifications'
const DEFAULT_LIMIT = 5
const MONITOR_LIMIT = 50

export interface VerificationRecord {
  hash: string
  timestamp: number
  /** Status from last verification (updated when lookup completes) */
  status?: HashStatus
  sourceChain?: string
  destChain?: string
  matches?: boolean
  cancelled?: boolean
}

export function RecentVerifications({ limit = DEFAULT_LIMIT }: { limit?: number }) {
  const [items, setItems] = useState<VerificationRecord[]>([])

  const load = useCallback(() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      const list: VerificationRecord[] = raw ? JSON.parse(raw) : []
      setItems(list.slice(0, limit))
    } catch {
      setItems([])
    }
  }, [limit])

  useEffect(() => {
    load()
    const handler = () => load()
    window.addEventListener('cl8y-verification-recorded', handler)
    return () => window.removeEventListener('cl8y-verification-recorded', handler)
  }, [load])

  if (items.length === 0) return null

  return (
    <div className="space-y-2">
      <h3 className="text-sm font-medium text-gray-300">Recent Verifications</h3>
      <div className="space-y-2">
        {items.map((item) => (
          <div
            key={item.hash}
            className="flex items-center justify-between border-2 border-white/20 bg-[#161616] p-3"
          >
            <code className="flex-1 truncate font-mono text-xs text-gray-300">
              {item.hash.slice(0, 18)}…{item.hash.slice(-10)}
            </code>
            <span className="ml-2 text-xs text-gray-400">
              {new Date(item.timestamp).toLocaleString()}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

export function recordVerification(hash: string) {
  const record: VerificationRecord = { hash, timestamp: Date.now() }
  const raw = localStorage.getItem(STORAGE_KEY)
  const list: VerificationRecord[] = raw ? JSON.parse(raw) : []
  const existing = list.findIndex((r) => r.hash.toLowerCase() === hash.toLowerCase())
  if (existing >= 0) list.splice(existing, 1)
  list.unshift(record)
  localStorage.setItem(STORAGE_KEY, JSON.stringify(list.slice(0, MONITOR_LIMIT)))
  window.dispatchEvent(new CustomEvent('cl8y-verification-recorded'))
}

export interface VerificationResult {
  status: HashStatus
  sourceChain?: string | null
  destChain?: string | null
  matches?: boolean | null
  cancelled?: boolean
}

export function recordVerificationResult(hash: string, result: VerificationResult) {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    const list: VerificationRecord[] = raw ? JSON.parse(raw) : []
    const idx = list.findIndex((r) => r.hash.toLowerCase() === hash.toLowerCase())
    if (idx >= 0) {
      list[idx] = {
        ...list[idx],
        status: result.status,
        sourceChain: result.sourceChain ?? undefined,
        destChain: result.destChain ?? undefined,
        matches: result.matches ?? undefined,
        cancelled: result.cancelled,
      }
      localStorage.setItem(STORAGE_KEY, JSON.stringify(list.slice(0, MONITOR_LIMIT)))
      window.dispatchEvent(new CustomEvent('cl8y-verification-recorded'))
    }
  } catch {
    // Ignore storage errors
  }
}

export function getVerificationRecords(): VerificationRecord[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}
