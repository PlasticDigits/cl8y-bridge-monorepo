import { create } from 'zustand'
import type { TransferRecord, TransferStatus } from '../types/transfer'

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
  recordTransfer: (record: Omit<TransferRecord, 'id' | 'timestamp'>) => void
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
    const fullRecord: TransferRecord = {
      ...record,
      id: `tx-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`,
      timestamp: Date.now(),
    }
    const stored = localStorage.getItem('cl8y-bridge-transactions')
    const list: TransferRecord[] = stored ? JSON.parse(stored) : []
    list.unshift(fullRecord)
    localStorage.setItem('cl8y-bridge-transactions', JSON.stringify(list.slice(0, 100)))
    // Notify same-tab listeners (storage event only fires cross-tab)
    window.dispatchEvent(new CustomEvent('cl8y-transfer-recorded'))
  },
}))
