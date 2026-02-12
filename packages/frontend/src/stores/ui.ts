import { create } from 'zustand'

export interface UIState {
  showEvmWalletModal: boolean
  setShowEvmWalletModal: (show: boolean) => void
}

export const useUIStore = create<UIState>((set) => ({
  showEvmWalletModal: false,
  setShowEvmWalletModal: (show) => set({ showEvmWalletModal: show }),
}))
