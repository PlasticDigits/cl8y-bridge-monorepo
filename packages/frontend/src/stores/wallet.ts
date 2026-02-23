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
  connectDevWallet,
  tryReconnect,
  WalletName,
  WalletType,
  TerraWalletType,
} from '../services/terra';
import { NETWORKS, DEFAULT_NETWORK } from '../utils/constants';

// Re-export for convenience
export { WalletName, WalletType };
export type { TerraWalletType };

/** Map TerraWalletType (persisted string) back to cosmes WalletName for reconnection */
const WALLET_TYPE_TO_NAME: Record<TerraWalletType, WalletName> = {
  station: WalletName.STATION,
  keplr: WalletName.KEPLR,
  luncdash: WalletName.LUNCDASH,
  galaxy: WalletName.GALAXYSTATION,
  leap: WalletName.LEAP,
  cosmostation: WalletName.COSMOSTATION,
};

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
  connectSimulated: () => void;
  disconnect: () => Promise<void>;
  attemptReconnect: () => Promise<boolean>;
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
    (set, get) => ({
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

      // Connect dev wallet (DEV_MODE only) using cosmes MnemonicWallet
      connectSimulated: () => {
        const result = connectDevWallet()
        const chainId = NETWORKS[DEFAULT_NETWORK as keyof typeof NETWORKS].terra.chainId
        set({
          connected: true,
          connecting: false,
          connectingWallet: null,
          address: result.address,
          walletType: result.walletType as TerraWalletType,
          connectionType: result.connectionType,
          chainId,
          luncBalance: '0',
        })
      },

      // Connect to wallet
      connect: async (walletName: WalletName, walletTypeParam: WalletType = WalletType.EXTENSION) => {
        set({ connecting: true, connectingWallet: walletName });
        
        try {
          const effectiveWalletType = walletName === WalletName.LUNCDASH 
            ? WalletType.WALLETCONNECT 
            : walletTypeParam;
          
          const result = await connectTerraWallet(walletName, effectiveWalletType);
          const chainId = NETWORKS[DEFAULT_NETWORK as keyof typeof NETWORKS].terra.chainId
          
          set({
            connected: true,
            connecting: false,
            connectingWallet: null,
            address: result.address,
            walletType: result.walletType,
            connectionType: result.connectionType,
            chainId,
          });
          
          console.log('Terra wallet connected:', result.address, result.walletType);
        } catch (error) {
          console.error('Wallet connection failed:', error);
          set({ connecting: false, connectingWallet: null });
          throw error;
        }
      },

      // Attempt to silently reconnect a previously-connected wallet (e.g. page refresh).
      // Extensions re-request key; WalletConnect restores cached session.
      attemptReconnect: async () => {
        const { walletType, connectionType, address } = get()
        if (!walletType || !address) return false

        const walletName = WALLET_TYPE_TO_NAME[walletType]
        if (!walletName) return false

        const effectiveType = connectionType ?? WalletType.EXTENSION

        try {
          const result = await tryReconnect(walletName, effectiveType)
          if (result) {
            const chainId = NETWORKS[DEFAULT_NETWORK as keyof typeof NETWORKS].terra.chainId
            set({
              connected: true,
              address: result.address,
              chainId,
            })
            return true
          }
        } catch (error) {
          console.warn('Auto-reconnect failed:', error)
        }

        // Clear persisted state on failed reconnection
        set({
          connected: false,
          address: null,
          walletType: null,
          connectionType: null,
          chainId: null,
        })
        return false
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
        connectionType: state.connectionType,
        address: state.address,
      }),
    }
  )
);
