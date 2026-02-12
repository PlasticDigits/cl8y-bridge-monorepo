/**
 * useTransferHistory - localStorage-backed transfer history
 *
 * No backend. Persists to localStorage. Used by RecentTransfers.
 */

import { useState, useCallback, useEffect } from 'react'
import type { TransferRecord } from '../types/transfer'

const STORAGE_KEY = 'cl8y-bridge-transactions'
const DEFAULT_LIMIT = 10

export function useTransferHistory(limit: number = DEFAULT_LIMIT) {
  const [transfers, setTransfers] = useState<TransferRecord[]>([])

  const load = useCallback(() => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      const list: TransferRecord[] = raw ? JSON.parse(raw) : []
      setTransfers(list.slice(0, limit))
    } catch {
      setTransfers([])
    }
  }, [limit])

  useEffect(() => {
    load()
    const handler = () => load()
    // Listen for cross-tab storage events
    window.addEventListener('storage', handler)
    // Listen for same-tab custom events dispatched by recordTransfer
    window.addEventListener('cl8y-transfer-recorded', handler)
    return () => {
      window.removeEventListener('storage', handler)
      window.removeEventListener('cl8y-transfer-recorded', handler)
    }
  }, [load])

  return { transfers, refresh: load }
}
