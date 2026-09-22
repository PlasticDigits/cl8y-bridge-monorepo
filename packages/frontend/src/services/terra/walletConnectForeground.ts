import { WalletName, WalletType } from '@goblinhunt/cosmes/wallet'
import { NETWORKS, DEFAULT_NETWORK } from '../../utils/constants'
import { useWalletStore } from '../../stores/wallet'
import { useWalletConnectPairingStore } from '../../stores/walletConnectPairing'
import { tryReconnect } from './connect'
import type { TerraWalletType } from './types'
import { allowsWalletConnectInfra } from '../../lib/storageConsent'

const WALLET_NAME_TO_TYPE: Partial<Record<WalletName, TerraWalletType>> = {
  [WalletName.STATION]: 'station',
  [WalletName.KEPLR]: 'keplr',
  [WalletName.LUNCDASH]: 'luncdash',
  [WalletName.GALAXYSTATION]: 'galaxy',
  [WalletName.LEAP]: 'leap',
  [WalletName.COSMOSTATION]: 'cosmostation',
}

export type WalletConnectForegroundResult = 'connected' | 'left-in-flight' | 'ignored'

const COSMES_WC_SESSION_KEY = /^cosmes\.wallet\..+\.wcSession$/i

/**
 * Cosmes persists WC v1/v2 sessions under `cosmes.wallet.<name>.wcSession`.
 * A cached session is the only safe input to `tryReconnect` while a pairing
 * URI is already in the user's wallet — `controller.connect()` otherwise
 * calls `createSession()` and mints a new `wc:` URI.
 */
export function hasCachedWalletConnectSession(
  storage: Pick<Storage, 'length' | 'key' | 'getItem'> | null = typeof localStorage === 'undefined' ? null : localStorage
): boolean {
  if (!storage) return false
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i)
    if (!key || !COSMES_WC_SESSION_KEY.test(key)) continue
    const raw = storage.getItem(key)
    if (raw && raw.length > 2) return true
  }
  return false
}

/**
 * Android/iOS Open (`luncdash:` / `intent:`) backgrounds Chrome. Returning to
 * the tab must not `cancelConnection()` (that closes the pairing sheet) and
 * must not start a new `connectTerraWallet` (that rotates the URI the wallet
 * may already have approved). INV-FE-WC-MOBILE-1.
 *
 * - Pairing sheet open, no cached session: leave the in-flight connect.
 * - Cached session present: `tryReconnect` restores it without `createSession`.
 */
export async function resumeWalletConnectAfterForeground(): Promise<WalletConnectForegroundResult> {
  if (!allowsWalletConnectInfra()) return 'ignored'
  const state = useWalletStore.getState()
  if (!state.connecting || !state.connectingWallet) return 'ignored'

  const pairingOpen = useWalletConnectPairingStore.getState().isOpen
  const cached = hasCachedWalletConnectSession()

  if (!pairingOpen && !cached) return 'ignored'
  if (pairingOpen && !cached) return 'left-in-flight'

  const wallet = state.connectingWallet
  try {
    const result = await tryReconnect(wallet, WalletType.WALLETCONNECT)
    if (!result) return 'left-in-flight'

    const latest = useWalletStore.getState()
    if (!latest.connecting) return 'left-in-flight'

    const chainId = NETWORKS[DEFAULT_NETWORK as keyof typeof NETWORKS].terra.chainId
    useWalletStore.setState({
      connected: true,
      connecting: false,
      connectingWallet: null,
      connectingSince: null,
      connectionError: null,
      address: result.address,
      walletType: WALLET_NAME_TO_TYPE[wallet] ?? latest.walletType,
      connectionType: WalletType.WALLETCONNECT,
      chainId,
    })
    return 'connected'
  } catch {
    console.warn('[Wallet] Visibility-triggered cached-session reconnect failed')
    return 'left-in-flight'
  }
}
