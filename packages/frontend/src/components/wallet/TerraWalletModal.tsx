import { useCallback, useMemo } from 'react'
import { useWallet, WalletName, WalletType } from '../../hooks/useWallet'
import { Modal } from '../ui'
import { TerraWalletOption, getTerraWalletIcon } from './TerraWalletOption'
import { DEV_MODE } from '../../utils/constants'
import { detectInAppBrowser } from '../../utils/detectInAppBrowser'

const WC_WALLETS = new Set<WalletName>([WalletName.LUNCDASH, WalletName.GALAXYSTATION])

export interface TerraWalletModalProps {
  isOpen: boolean
  onClose: () => void
}

export function TerraWalletModal({ isOpen, onClose }: TerraWalletModalProps) {
  const {
    connecting,
    connectingWallet,
    connectionError,
    isStationAvailable,
    isKeplrAvailable,
    isLeapAvailable,
    isCosmostationAvailable,
    connect,
    connectSimulated,
    cancelConnection,
    clearConnectionError,
  } = useWallet()

  const isWcConnecting = connecting && connectingWallet != null && WC_WALLETS.has(connectingWallet)
  const inAppBrowser = useMemo(() => detectInAppBrowser(), [])

  // Modal already handles Escape key - this just adds cancelConnection on close
  const closeModal = useCallback(() => {
    onClose()
    if (connecting) cancelConnection()
  }, [connecting, cancelConnection, onClose])

  const handleConnect = async (walletName: WalletName, walletType: WalletType = WalletType.EXTENSION) => {
    clearConnectionError()
    try {
      await connect(walletName, walletType)
      onClose()
    } catch {
      // connectionError is set by the store; displayed below
    }
  }

  const handleRetry = (walletName: WalletName) => {
    cancelConnection()
    setTimeout(() => handleConnect(walletName, WalletType.WALLETCONNECT), 100)
  }

  const wallets = [
    {
      walletName: WalletName.STATION,
      name: 'Terra Station',
      description: isStationAvailable ? 'Recommended' : 'Not installed',
      available: isStationAvailable,
    },
    {
      walletName: WalletName.KEPLR,
      name: 'Keplr',
      description: isKeplrAvailable ? 'Cosmos ecosystem' : 'Not installed',
      available: isKeplrAvailable,
    },
    {
      walletName: WalletName.LEAP,
      name: 'Leap',
      description: isLeapAvailable ? 'Multi-chain' : 'Not installed',
      available: isLeapAvailable,
    },
    {
      walletName: WalletName.COSMOSTATION,
      name: 'Cosmostation',
      description: isCosmostationAvailable ? 'Cosmos wallet' : 'Not installed',
      available: isCosmostationAvailable,
    },
    {
      walletName: WalletName.LUNCDASH,
      name: 'LUNC Dash',
      description: 'Mobile wallet',
      available: true,
    },
    {
      walletName: WalletName.GALAXYSTATION,
      name: 'Galaxy Station',
      description: 'Mobile wallet',
      available: true,
    },
  ]

  return (
    <Modal isOpen={isOpen} onClose={closeModal} title="Connect Wallet">
      <div className="p-6 space-y-3">
        {connectionError && (
          <div className="flex items-start gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-lg text-sm text-red-300">
            <svg className="w-4 h-4 mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <div className="flex-1">
              <p>{connectionError}</p>
            </div>
            <button
              type="button"
              onClick={clearConnectionError}
              className="text-red-400 hover:text-red-300 shrink-0"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        )}
        {DEV_MODE && (
          <>
            <p className="text-xs text-amber-500/70 uppercase tracking-wider mb-2 font-medium">Dev Mode</p>
            <TerraWalletOption
              name="Simulated Terra Wallet"
              description="No extension required (cannot sign transactions)"
              available={true}
              loading={false}
              onClick={() => {
                connectSimulated()
                onClose()
              }}
              disabled={connecting}
              icon="🔧"
            />
            <p className="text-xs text-amber-500/70 uppercase tracking-wider mt-4 mb-2 font-medium">Browser Extension</p>
          </>
        )}
        {!DEV_MODE && (
          <p className="text-xs text-amber-500/70 uppercase tracking-wider mb-2 font-medium">Browser Extension</p>
        )}
        {wallets.slice(0, 4).map((w) => (
          <TerraWalletOption
            key={w.walletName}
            name={w.name}
            description={w.description}
            available={w.available}
            loading={connectingWallet === w.walletName}
            onClick={() => handleConnect(w.walletName, WalletType.EXTENSION)}
            disabled={connecting}
            icon={getTerraWalletIcon(w.walletName)}
          />
        ))}
        <p className="text-xs text-amber-500/70 uppercase tracking-wider mt-4 mb-2 font-medium">Mobile / WalletConnect</p>
        {inAppBrowser.isInAppBrowser && (
          <div className="p-3 bg-amber-500/10 border border-amber-500/30 rounded-lg text-sm text-amber-300">
            <p className="font-medium">
              In-app browser detected{inAppBrowser.browserName ? ` (${inAppBrowser.browserName})` : ''}
            </p>
            <p className="text-xs text-amber-400/80 mt-1">
              WalletConnect deep links may not work here. For the best experience,
              copy this page URL and open it in your device&apos;s default browser.
            </p>
          </div>
        )}
        {wallets.slice(4).map((w) => (
          <div key={w.walletName}>
            <TerraWalletOption
              name={w.name}
              description={w.description}
              available={w.available}
              loading={connectingWallet === w.walletName}
              onClick={() => handleConnect(w.walletName, WalletType.WALLETCONNECT)}
              disabled={connecting}
              icon={getTerraWalletIcon(w.walletName)}
            />
            {connectingWallet === w.walletName && isWcConnecting && (
              <div className="flex items-center gap-2 mt-1 ml-14">
                <p className="text-xs text-gray-400">Waiting for wallet&hellip;</p>
                <button
                  type="button"
                  onClick={() => handleRetry(w.walletName)}
                  className="text-xs text-blue-400 hover:text-blue-300 underline"
                >
                  Retry
                </button>
                <button
                  type="button"
                  onClick={cancelConnection}
                  className="text-xs text-gray-500 hover:text-gray-400 underline"
                >
                  Cancel
                </button>
              </div>
            )}
          </div>
        ))}
      </div>
    </Modal>
  )
}
