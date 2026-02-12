import { useCallback, useEffect, useState } from 'react'

const STORAGE_KEY = 'cl8y-bridge-verifications'
const DEFAULT_LIMIT = 5

export interface VerificationRecord {
  hash: string
  timestamp: number
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
      <h3 className="text-sm font-medium text-gray-400">Recent Verifications</h3>
      <div className="space-y-2">
        {items.map((item) => (
          <div
            key={item.hash}
            className="bg-gray-900/50 rounded-lg p-3 border border-gray-700/50 flex items-center justify-between"
          >
            <code className="text-xs text-gray-400 font-mono truncate flex-1">
              {item.hash.slice(0, 18)}…{item.hash.slice(-10)}
            </code>
            <span className="text-xs text-gray-600 ml-2">
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
  list.unshift(record)
  localStorage.setItem(STORAGE_KEY, JSON.stringify(list.slice(0, 50)))
  window.dispatchEvent(new CustomEvent('cl8y-verification-recorded'))
}
