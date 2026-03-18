import { create } from "zustand";
import { persist } from "zustand/middleware";
import {
  connectSolanaWallet,
  disconnectSolanaWallet,
} from "../services/solana/connect";

export type SolanaWalletType = "phantom" | "solflare" | "backpack" | "coinbase";

interface SolanaWalletState {
  connected: boolean;
  connecting: boolean;
  address: string | null;
  walletType: SolanaWalletType | null;
  solBalance: string | null;
  connectionError: string | null;
  showWalletModal: boolean;
}

interface SolanaWalletActions {
  connect: (walletType: SolanaWalletType) => Promise<void>;
  disconnect: () => Promise<void>;
  attemptReconnect: () => Promise<void>;
  setBalance: (sol: string | null) => void;
  cancelConnection: () => void;
  setShowWalletModal: (show: boolean) => void;
}

const initialState: SolanaWalletState = {
  connected: false,
  connecting: false,
  address: null,
  walletType: null,
  solBalance: null,
  connectionError: null,
  showWalletModal: false,
};

export const useSolanaWalletStore = create<SolanaWalletState & SolanaWalletActions>()(
  persist(
    (set, get) => ({
      ...initialState,

      connect: async (walletType: SolanaWalletType) => {
        set({ connecting: true, connectionError: null });
        try {
          const result = await connectSolanaWallet(walletType);
          set({
            connected: result.connected,
            address: result.address,
            walletType,
            connecting: false,
            showWalletModal: false,
          });
        } catch (error: unknown) {
          set({
            connecting: false,
            connectionError: error instanceof Error ? error.message : "Failed to connect",
          });
        }
      },

      disconnect: async () => {
        const { walletType } = get();
        if (walletType) {
          try {
            await disconnectSolanaWallet(walletType);
          } catch {
            // ignore disconnect errors
          }
        }
        set(initialState);
      },

      attemptReconnect: async () => {
        const { walletType } = get();
        if (!walletType) return;
        try {
          const result = await connectSolanaWallet(walletType);
          set({
            connected: result.connected,
            address: result.address,
          });
        } catch {
          set(initialState);
        }
      },

      setBalance: (sol) => set({ solBalance: sol }),

      cancelConnection: () => {
        set({ connecting: false, connectionError: null });
      },

      setShowWalletModal: (show) => set({ showWalletModal: show }),
    }),
    {
      name: "cl8y-bridge-solana-wallet-storage",
      partialize: (state) => ({
        walletType: state.walletType,
        address: state.address,
      }),
    }
  )
);
