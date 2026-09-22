import { beforeEach, describe, expect, it, vi } from 'vitest'
import { WalletName, WalletType } from '@goblinhunt/cosmes/wallet'
import { useWalletStore } from '../../stores/wallet'
import { useWalletConnectPairingStore } from '../../stores/walletConnectPairing'

vi.mock('./connect', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./connect')>()
  return {
    ...actual,
    tryReconnect: vi.fn(),
  }
})

import { tryReconnect } from './connect'
import {
  hasCachedWalletConnectSession,
  resumeWalletConnectAfterForeground,
} from './walletConnectForeground'
import { STORAGE_CONSENT_KEY } from '../../lib/storageConsent'

const mockTryReconnect = vi.mocked(tryReconnect)

const PAIRING = {
  uri: 'wc:00e46b69-d0cc-4b3e-b6a2-cee442f97188@1?bridge=https%3A%2F%2Fwalletconnect.luncdash.com&key=abc',
  name: 'LUNC Dash',
  android: '',
  ios: '',
  isStation: true,
  isLuncDash: true,
}

describe('resumeWalletConnectAfterForeground (GL-137)', () => {
  beforeEach(() => {
    mockTryReconnect.mockReset()
    localStorage.clear()
    localStorage.setItem(
      STORAGE_CONSENT_KEY,
      JSON.stringify({ v: 1, decided: true, optionalThirdParty: true }),
    )
    useWalletStore.setState({
      connected: false,
      connecting: false,
      connectingWallet: null,
      connectingSince: null,
      address: null,
      walletType: null,
      connectionType: null,
      connectionError: null,
    })
    useWalletConnectPairingStore.setState({ isOpen: false, payload: null })
  })

  it('detects cosmes wcSession keys only', () => {
    const store = new Map<string, string>([
      ['unrelated', '{}'],
      ['cosmes.wallet.luncdash.wcSession', '{"connected":true}'],
    ])
    const storage = {
      get length() {
        return store.size
      },
      key(i: number) {
        return [...store.keys()][i] ?? null
      },
      getItem(k: string) {
        return store.get(k) ?? null
      },
    }
    expect(hasCachedWalletConnectSession(storage)).toBe(true)
    expect(hasCachedWalletConnectSession({ length: 0, key: () => null, getItem: () => null })).toBe(false)
  })

  it('does not cancel or mint a new URI when the pairing sheet is open without a cached session', async () => {
    useWalletStore.setState({
      connecting: true,
      connectingWallet: WalletName.LUNCDASH,
      connectingSince: Date.now(),
    })
    useWalletConnectPairingStore.setState({ isOpen: true, payload: PAIRING })

    const result = await resumeWalletConnectAfterForeground()

    expect(result).toBe('left-in-flight')
    expect(mockTryReconnect).not.toHaveBeenCalled()
    expect(useWalletConnectPairingStore.getState().isOpen).toBe(true)
    expect(useWalletConnectPairingStore.getState().payload?.uri).toBe(PAIRING.uri)
    expect(useWalletStore.getState().connecting).toBe(true)
  })

  it('ignores visibility when not connecting', async () => {
    expect(await resumeWalletConnectAfterForeground()).toBe('ignored')
    expect(mockTryReconnect).not.toHaveBeenCalled()
  })

  it('picks up a cached session via tryReconnect without closing the pairing sheet', async () => {
    localStorage.setItem('cosmes.wallet.luncdash.wcSession', JSON.stringify({ connected: true }))
    useWalletStore.setState({
      connecting: true,
      connectingWallet: WalletName.LUNCDASH,
      connectingSince: Date.now(),
    })
    useWalletConnectPairingStore.setState({ isOpen: true, payload: PAIRING })
    mockTryReconnect.mockResolvedValue({ address: 'terra1cached' })

    const result = await resumeWalletConnectAfterForeground()

    expect(result).toBe('connected')
    expect(mockTryReconnect).toHaveBeenCalledWith(WalletName.LUNCDASH, WalletType.WALLETCONNECT)
    expect(useWalletStore.getState().connected).toBe(true)
    expect(useWalletStore.getState().address).toBe('terra1cached')
    expect(useWalletStore.getState().connecting).toBe(false)
    expect(useWalletConnectPairingStore.getState().isOpen).toBe(true)
    expect(useWalletConnectPairingStore.getState().payload?.uri).toBe(PAIRING.uri)
  })
})
