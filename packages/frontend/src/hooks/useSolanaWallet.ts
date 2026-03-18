import { useCallback, useEffect } from "react";
import { useSolanaWalletStore, SolanaWalletType } from "../stores/solanaWallet";

export function useSolanaWallet() {
  const store = useSolanaWalletStore();

  useEffect(() => {
    if (store.walletType && !store.connected && !store.connecting) {
      store.attemptReconnect();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const connect = useCallback(
    (walletType: SolanaWalletType) => store.connect(walletType),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [store.connect]
  );

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const disconnect = useCallback(() => store.disconnect(), [store.disconnect]);

  return {
    connected: store.connected,
    connecting: store.connecting,
    address: store.address,
    walletType: store.walletType,
    solBalance: store.solBalance,
    connectionError: store.connectionError,
    showWalletModal: store.showWalletModal,
    setShowWalletModal: store.setShowWalletModal,
    connect,
    disconnect,
  };
}
