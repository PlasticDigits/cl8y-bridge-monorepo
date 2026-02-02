/**
 * Wallet State Management for CL8Y Bridge
 * 
 * Uses Zustand for lightweight, hook-based state management.
 * Handles wallet connection state for Terra Classic wallets.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import {
  connectTerraWallet,
  disconnectTerraWallet,
  isStationInstalled,
  isKeplrInstalled,
  isLeapInstalled,
  isCosmostationInstalled,
  WalletName,
  WalletType,
  TerraWalletType,
} from '../services/wallet';

// Re-export for convenience
export { WalletName, WalletType };
export type { TerraWalletType };

export interface WalletState {
  // Connection state
  connected: boolean;
  connecting: boolean;
  address: string | null;
  walletType: TerraWalletType | null;
  connectionType: WalletType | null;
  
  // Network state
  chainId: string | null;
  
  // Balances (micro units)
  luncBalance: string;
  
  // Connecting state for specific wallets
  connectingWallet: WalletName | null;
  
  // Modal state (for triggering wallet modal from other components)
  showWalletModal: boolean;
  
  // Actions
  connect: (walletName: WalletName, walletType?: WalletType) => Promise<void>;
  disconnect: () => Promise<void>;
  setBalances: (balances: { lunc?: string }) => void;
  setConnecting: (connecting: boolean) => void;
  cancelConnection: () => void;
  setShowWalletModal: (show: boolean) => void;
}

// Wallet availability checks
export function checkWalletAvailability() {
  return {
    station: isStationInstalled(),
    keplr: isKeplrInstalled(),
    leap: isLeapInstalled(),
    cosmostation: isCosmostationInstalled(),
    // WalletConnect-only wallets are always "available"
    luncdash: true,
    galaxy: true,
  };
}

export const useWalletStore = create<WalletState>()(
  persist(
    (set, _get) => ({
      // Initial state
      connected: false,
      connecting: false,
      address: null,
      walletType: null,
      connectionType: null,
      chainId: null,
      luncBalance: '0',
      connectingWallet: null,
      showWalletModal: false,

      // Connect to wallet
      connect: async (walletName: WalletName, walletTypeParam: WalletType = WalletType.EXTENSION) => {
        set({ connecting: true, connectingWallet: walletName });
        
        try {
          // LUNC Dash always uses WalletConnect
          const effectiveWalletType = walletName === WalletName.LUNCDASH 
            ? WalletType.WALLETCONNECT 
            : walletTypeParam;
          
          const result = await connectTerraWallet(walletName, effectiveWalletType);
          
          set({
            connected: true,
            connecting: false,
            connectingWallet: null,
            address: result.address,
            walletType: result.walletType,
            connectionType: result.connectionType,
            chainId: result.walletType === 'station' ? 'columbus-5' : 'columbus-5', // Adjust based on network
          });
          
          console.log('Terra wallet connected:', result.address, result.walletType);
        } catch (error) {
          console.error('Wallet connection failed:', error);
          set({ connecting: false, connectingWallet: null });
          throw error;
        }
      },

      // Disconnect wallet
      disconnect: async () => {
        try {
          await disconnectTerraWallet();
        } catch (e) {
          console.error('Disconnect error (non-fatal):', e);
        }
        
        set({
          connected: false,
          connecting: false,
          connectingWallet: null,
          address: null,
          walletType: null,
          connectionType: null,
          chainId: null,
          luncBalance: '0',
        });
      },

      // Update balances
      setBalances: (balances) => {
        set((state) => ({
          luncBalance: balances.lunc ?? state.luncBalance,
        }));
      },

      // Set connecting state
      setConnecting: (connecting) => {
        set({ connecting });
      },

      // Cancel pending connection
      cancelConnection: () => {
        set({ connecting: false, connectingWallet: null });
      },

      // Control wallet modal visibility
      setShowWalletModal: (show: boolean) => {
        set({ showWalletModal: show });
      },
    }),
    {
      name: 'cl8y-bridge-wallet-storage',
      partialize: (state) => ({
        walletType: state.walletType,
        address: state.address,
        // Don't persist balances - refresh on reconnect
      }),
    }
  )
);
