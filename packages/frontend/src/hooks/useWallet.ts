/**
 * useWallet Hook for CL8Y Bridge
 * 
 * Provides Terra wallet connection and state management functionality.
 * Wraps the wallet store with additional utilities.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { useWalletStore, checkWalletAvailability, WalletName, WalletType } from '../stores/wallet';
import { connectTerraWallet } from '../services/terra';
import { NETWORKS, DEFAULT_NETWORK, LCD_CONFIG } from '../utils/constants';

/** Auto-cancel WalletConnect attempts that haven't resolved in this many ms */
const WC_CONNECTION_TIMEOUT_MS = 60_000;

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
    connectingSince,
    showWalletModal,
    connect: storeConnect,
    connectSimulated: storeConnectSimulated,
    disconnect: storeDisconnect,
    attemptReconnect,
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

  // Auto-reconnect on mount when persisted state indicates a previous connection.
  // Runs once; restores the cosmes controller connection so the wallet is usable.
  const reconnectAttempted = useRef(false);
  useEffect(() => {
    if (reconnectAttempted.current) return;
    if (connected) return; // already connected
    if (!walletType || !address) return; // no persisted session
    reconnectAttempted.current = true;
    attemptReconnect();
  }, [connected, walletType, address, attemptReconnect]);

  // Auto-cancel stale WalletConnect connection attempts (e.g. WebSocket died
  // while Safari was backgrounded during a LUNC Dash app-switch).
  useEffect(() => {
    if (!connecting || !connectingSince) return;
    const remaining = WC_CONNECTION_TIMEOUT_MS - (Date.now() - connectingSince);
    if (remaining <= 0) {
      console.warn('[Wallet] Connection attempt timed out, resetting')
      cancelConnection();
      return;
    }
    const timer = setTimeout(() => {
      if (useWalletStore.getState().connecting) {
        console.warn('[Wallet] Connection attempt timed out, resetting')
        cancelConnection();
      }
    }, remaining);
    return () => clearTimeout(timer);
  }, [connecting, connectingSince, cancelConnection]);

  // When the page becomes visible again after a WalletConnect app-switch (e.g.
  // returning from LUNC Dash on iPad), retry the connection. The WC bridge may
  // have a completed session cached in localStorage that we can pick up.
  useEffect(() => {
    const handleVisibility = async () => {
      if (document.visibilityState !== 'visible') return;
      const state = useWalletStore.getState();
      if (!state.connecting || !state.connectingWallet) return;

      const wallet = state.connectingWallet;
      console.log('[Wallet] Page became visible during WC connection, retrying', wallet);

      cancelConnection();
      await new Promise((r) => setTimeout(r, 500));

      try {
        const chainId = NETWORKS[DEFAULT_NETWORK as keyof typeof NETWORKS].terra.chainId;
        const result = await connectTerraWallet(wallet, WalletType.WALLETCONNECT);
        useWalletStore.setState({
          connected: true,
          connecting: false,
          connectingWallet: null,
          connectingSince: null,
          address: result.address,
          walletType: result.walletType,
          connectionType: result.connectionType,
          chainId,
        });
      } catch {
        console.warn('[Wallet] Visibility-triggered reconnect failed');
      }
    };

    document.addEventListener('visibilitychange', handleVisibility);
    return () => document.removeEventListener('visibilitychange', handleVisibility);
  }, [cancelConnection]);

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
      await refreshBalances();
    } catch (error) {
      console.error('Connection failed:', error);
      throw error;
    }
  }, [storeConnect, refreshBalances]);

  // Connect simulated wallet (dev mode)
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
      const interval = setInterval(refreshBalances, 30000);
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
