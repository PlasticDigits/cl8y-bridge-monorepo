/**
 * Re-export Terra wallet services for backward compatibility.
 * @see services/terra/ for the split implementation
 */
export {
  WalletName,
  WalletType,
  connectTerraWallet,
  disconnectTerraWallet,
  getConnectedWallet,
  getCurrentTerraAddress,
  isTerraWalletConnected,
  isStationInstalled,
  isKeplrInstalled,
  isLeapInstalled,
  isCosmostationInstalled,
  executeContractWithCoins,
  executeCw20Send,
} from './terra'

export type { TerraWalletType } from './terra'
