/**
 * useWallet Hook for CL8Y Bridge
 * 
 * Provides Terra wallet connection and state management functionality.
 * Wraps the wallet store with additional utilities.
 */

import { useCallback, useEffect, useState } from 'react';
import { useWalletStore, checkWalletAvailability, WalletName, WalletType } from '../stores/wallet';
import { NETWORKS, DEFAULT_NETWORK, LCD_CONFIG } from '../utils/constants';

export { WalletName, WalletType };

/**
 * Fetch native balance from LCD
 */
async function fetchNativeBalance(address: string, denom: string): Promise<string> {
  const networkConfig = NETWORKS[DEFAULT_NETWORK].terra;
  
  for (const lcd of networkConfig.lcdFallbacks) {
    try {
      const response = await fetch(
        `${lcd}/cosmos/bank/v1beta1/balances/${address}/by_denom?denom=${denom}`,
        { signal: AbortSignal.timeout(LCD_CONFIG.requestTimeout) }
      );
      
      if (!response.ok) continue;
      
      const data = await response.json();
      return data.balance?.amount || '0';
    } catch {
      continue;
    }
  }
  
  return '0';
}

export function useWallet() {
  const {
    connected,
    connecting,
    address,
    walletType,
    connectionType,
    chainId,
    luncBalance,
    connectingWallet,
    showWalletModal,
    connect: storeConnect,
    connectSimulated: storeConnectSimulated,
    disconnect: storeDisconnect,
    setBalances,
    setConnecting,
    cancelConnection,
    setShowWalletModal,
  } = useWalletStore();

  // Track wallet availability
  const [walletAvailability, setWalletAvailability] = useState(checkWalletAvailability);

  // Check wallet availability on mount and periodically
  useEffect(() => {
    const check = () => setWalletAvailability(checkWalletAvailability());
    check();
    const interval = setInterval(check, 1000);
    return () => clearInterval(interval);
  }, []);

  // Refresh balances from chain
  const refreshBalances = useCallback(async () => {
    if (!address) return;

    try {
      const lunc = await fetchNativeBalance(address, 'uluna');
      console.log('Balance fetched:', { lunc });
      setBalances({ lunc });
    } catch (error) {
      console.error('Failed to refresh balances:', error);
    }
  }, [address, setBalances]);

  // Connect to wallet
  const connect = useCallback(async (
    walletName: WalletName = WalletName.STATION,
    walletTypeParam: WalletType = WalletType.EXTENSION
  ) => {
    try {
      await storeConnect(walletName, walletTypeParam);
      // Refresh balances after connection
      await refreshBalances();
    } catch (error) {
      console.error('Connection failed:', error);
      throw error;
    }
  }, [storeConnect, refreshBalances]);

  // Connect simulated wallet (dev mode - no extensions, cannot sign)
  const connectSimulated = useCallback(() => {
    storeConnectSimulated();
  }, [storeConnectSimulated]);

  // Disconnect wallet
  const disconnect = useCallback(async () => {
    await storeDisconnect();
  }, [storeDisconnect]);

  // Auto-refresh balances when connected
  useEffect(() => {
    if (connected && address) {
      refreshBalances();
      
      // Set up periodic refresh
      const interval = setInterval(refreshBalances, 30000); // 30 seconds
      return () => clearInterval(interval);
    }
  }, [connected, address, refreshBalances]);

  return {
    // State
    connected,
    connecting,
    address,
    walletType,
    connectionType,
    chainId,
    connectingWallet,
    showWalletModal,
    
    // Balances
    luncBalance,
    
    // Wallet availability
    isStationAvailable: walletAvailability.station,
    isKeplrAvailable: walletAvailability.keplr,
    isLeapAvailable: walletAvailability.leap,
    isCosmostationAvailable: walletAvailability.cosmostation,
    
    // Actions
    connect,
    connectSimulated,
    disconnect,
    refreshBalances,
    setConnecting,
    cancelConnection,
    setShowWalletModal,
  };
}
