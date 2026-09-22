import { useCallback, useMemo, useState } from 'react'
import { useWallet, WalletName, WalletType } from '../../hooks/useWallet'
import { Modal } from '../ui'
import { TerraWalletOption, getTerraWalletIcon } from './TerraWalletOption'
import { DEV_MODE } from '../../utils/constants'
import { detectInAppBrowser } from '../../utils/detectInAppBrowser'
import { isWalletConnectMobileClient } from '../../utils/walletConnectPairing'
import { resolveConnectWalletOptions } from '../../utils/terraConnectWalletOptions'
import { useWalletConnectPairingStore } from '../../stores/walletConnectPairing'
import { StorageJitPrompt } from '../legal/StorageJitPrompt'
import { useStorageConsentActions } from '../../hooks/useStorageConsent'
import { allowsWalletConnectInfra } from '../../lib/storageConsent'

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

  const { jitWalletConnect } = useStorageConsentActions()
  const [pendingWcJit, setPendingWcJit] = useState<{
    walletName: WalletName
    walletType: WalletType
  } | null>(null)

  const pairingOpen = useWalletConnectPairingStore((s) => s.isOpen)
  const isMobileClient = isWalletConnectMobileClient()
  const inAppBrowser = useMemo(() => detectInAppBrowser(), [])

  const options = useMemo(
    () =>
      resolveConnectWalletOptions({
        isMobileClient,
        keplrInjected: isKeplrAvailable,
        stationInjected: isStationAvailable,
        cosmostationInjected: isCosmostationAvailable,
      }),
    [isMobileClient, isKeplrAvailable, isStationAvailable, isCosmostationAvailable]
  )

  const connectingOption = options.find((o) => o.walletName === connectingWallet)
  const isWcConnecting =
    connecting && connectingOption?.walletType === WalletType.WALLETCONNECT

  const closeModal = useCallback(() => {
    onClose()
    if (connecting) cancelConnection()
  }, [connecting, cancelConnection, onClose])

  const runConnect = async (walletName: WalletName, walletType: WalletType) => {
    clearConnectionError()
    try {
      await connect(walletName, walletType)
      onClose()
    } catch {
      // connectionError is set by the store; displayed below
    }
  }

  const handleConnect = async (walletName: WalletName, walletType: WalletType = WalletType.EXTENSION) => {
    if (walletType === WalletType.WALLETCONNECT && !allowsWalletConnectInfra()) {
      setPendingWcJit({ walletName, walletType })
      return
    }
    await runConnect(walletName, walletType)
  }

  const handleRetry = (walletName: WalletName, walletType: WalletType) => {
    cancelConnection()
    setTimeout(() => handleConnect(walletName, walletType), 100)
  }

  const extensionInstalled = (walletName: WalletName): boolean => {
    switch (walletName) {
      case WalletName.STATION:
        return isStationAvailable
      case WalletName.KEPLR:
        return isKeplrAvailable
      case WalletName.LEAP:
        return isLeapAvailable
      case WalletName.COSMOSTATION:
        return isCosmostationAvailable
      default:
        return true
    }
  }

  const optionAvailable = (walletName: WalletName, walletType: WalletType): boolean => {
    if (walletType === WalletType.WALLETCONNECT) return true
    return extensionInstalled(walletName)
  }

  const optionDescription = (
    walletName: WalletName,
    walletType: WalletType,
    connectionLabel: string
  ): string => {
    if (walletType === WalletType.WALLETCONNECT) {
      if (walletName === WalletName.KEPLR) return 'WalletConnect — Open in Keplr'
      if (walletName === WalletName.LUNCDASH || walletName === WalletName.GALAXYSTATION) {
        return 'Mobile wallet'
      }
      return connectionLabel
    }
    if (walletName === WalletName.STATION) return isStationAvailable ? 'Recommended' : 'Not installed'
    if (walletName === WalletName.KEPLR) return isKeplrAvailable ? 'Cosmos ecosystem' : 'Not installed'
    if (walletName === WalletName.LEAP) return isLeapAvailable ? 'Multi-chain' : 'Not installed'
    if (walletName === WalletName.COSMOSTATION) {
      return isCosmostationAvailable ? 'Cosmos wallet' : 'Not installed'
    }
    return connectionLabel
  }

  const showMobileHint = isMobileClient && !inAppBrowser.isInAppBrowser

  if (pairingOpen) return null

  return (
    <>
    <StorageJitPrompt
      kind={pendingWcJit ? 'walletConnect' : null}
      onCancel={() => setPendingWcJit(null)}
      onConfirm={() => {
        if (!pendingWcJit) return
        jitWalletConnect()
        const { walletName, walletType } = pendingWcJit
        setPendingWcJit(null)
        void runConnect(walletName, walletType)
      }}
    />
    <Modal isOpen={isOpen} onClose={closeModal} title="Connect Wallet" rootTestId="terra-wallet-modal-portal">
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
            <p className="text-xs text-amber-500/70 uppercase tracking-wider mt-4 mb-2 font-medium">
              {isMobileClient ? 'Wallets' : 'Browser Extension'}
            </p>
          </>
        )}
        {!DEV_MODE && (
          <p className="text-xs text-amber-500/70 uppercase tracking-wider mb-2 font-medium">
            {isMobileClient ? 'Wallets' : 'Browser Extension'}
          </p>
        )}
        {showMobileHint && (
          <p className="text-xs text-gray-400" data-testid="wallet-modal-mobile-hint">
            Use Open or Copy next. Leap is desktop-only — use Lunc Dash, Galaxy Station, or
            Keplr. Wallet in-app browser also works.
          </p>
        )}
        {inAppBrowser.isInAppBrowser && (
          <div
            className="p-3 bg-amber-500/10 border border-amber-500/30 rounded-lg text-sm text-amber-300"
            data-testid="wallet-modal-in-app-banner"
          >
            <p className="font-medium">
              In-app browser detected{inAppBrowser.browserName ? ` (${inAppBrowser.browserName})` : ''}
            </p>
            <p className="text-xs text-amber-400/80 mt-1">
              WalletConnect deep links may not work here. For the best experience,
              copy this page URL and open it in your device&apos;s default browser.
            </p>
          </div>
        )}
        {options.map((w) => {
          const available = optionAvailable(w.walletName, w.walletType)
          return (
            <div key={`${w.walletName}-${w.walletType}`}>
              <TerraWalletOption
                name={w.name}
                description={optionDescription(w.walletName, w.walletType, w.connectionLabel)}
                available={available}
                loading={connectingWallet === w.walletName}
                onClick={() => handleConnect(w.walletName, w.walletType)}
                disabled={connecting}
                icon={getTerraWalletIcon(w.walletName)}
              />
              {connectingWallet === w.walletName && isWcConnecting && (
                <div className="flex items-center gap-2 mt-1 ml-14">
                  <p className="text-xs text-gray-400">Waiting for wallet&hellip;</p>
                  <button
                    type="button"
                    onClick={() => handleRetry(w.walletName, w.walletType)}
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
          )
        })}
      </div>
    </Modal>
    </>
  )
}
