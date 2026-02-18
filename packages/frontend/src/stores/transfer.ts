import { create } from 'zustand'
import type { TransferRecord, TransferStatus } from '../types/transfer'

const STORAGE_KEY = 'cl8y-bridge-transactions'

export interface ActiveTransfer {
  id: string
  direction: 'evm-to-terra' | 'terra-to-evm' | 'evm-to-evm'
  sourceChain: string
  destChain: string
  amount: string
  status: TransferStatus
  txHash: string | null
  recipient: string
  startedAt: number
}

export interface TransferState {
  activeTransfer: ActiveTransfer | null
  setActiveTransfer: (transfer: ActiveTransfer | null) => void
  updateActiveTransfer: (updates: Partial<ActiveTransfer>) => void
  recordTransfer: (record: Omit<TransferRecord, 'id' | 'timestamp'>) => string
  updateTransferRecord: (id: string, updates: Partial<TransferRecord>) => void
  getTransferByXchainHashId: (xchainHashId: string) => TransferRecord | null
  getAllTransfers: () => TransferRecord[]
}

function readTransfers(): TransferRecord[] {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (!stored) return []
    return JSON.parse(stored) as TransferRecord[]
  } catch {
    return []
  }
}

function writeTransfers(list: TransferRecord[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(list.slice(0, 100)))
}

export const useTransferStore = create<TransferState>((set) => ({
  activeTransfer: null,

  setActiveTransfer: (transfer) => set({ activeTransfer: transfer }),

  updateActiveTransfer: (updates) =>
    set((state) => {
      if (!state.activeTransfer) return state
      return {
        activeTransfer: { ...state.activeTransfer, ...updates },
      }
    }),

  recordTransfer: (record) => {
    const id = `tx-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
    const fullRecord: TransferRecord = {
      ...record,
      id,
      timestamp: Date.now(),
      lifecycle: record.lifecycle || 'deposited',
    }
    const list = readTransfers()
    list.unshift(fullRecord)
    writeTransfers(list)
    // Notify same-tab listeners (storage event only fires cross-tab)
    window.dispatchEvent(new CustomEvent('cl8y-transfer-recorded'))
    return id
  },

  updateTransferRecord: (id, updates) => {
    const list = readTransfers()
    const idx = list.findIndex((t) => t.id === id)
    if (idx === -1) return
    list[idx] = { ...list[idx], ...updates }
    writeTransfers(list)
    window.dispatchEvent(new CustomEvent('cl8y-transfer-updated', { detail: { id } }))
  },

  getTransferByXchainHashId: (xchainHashId) => {
    const list = readTransfers()
    return list.find((t) => t.xchainHashId === xchainHashId) || null
  },

  getAllTransfers: () => {
    return readTransfers()
  },
}))
