import { useMemo, useState } from 'react'
import { useConnect } from 'wagmi'
import type { Connector } from 'wagmi'
import { Modal } from '../ui'
import { EvmWalletOption } from './EvmWalletOption'
import { useEvmWalletDiscovery } from '../../hooks/useEvmWalletDiscovery'
import { detectInAppBrowser } from '../../utils/detectInAppBrowser'

export interface EvmWalletModalProps {
  isOpen: boolean
  onClose: () => void
}

export function EvmWalletModal({ isOpen, onClose }: EvmWalletModalProps) {
  const { connectors } = useEvmWalletDiscovery()
  const [connectingUid, setConnectingUid] = useState<string | null>(null)
  const inAppBrowser = useMemo(() => detectInAppBrowser(), [])
  const { connect, isPending, error } = useConnect({
    mutation: {
      onSuccess: () => {
        setConnectingUid(null)
        onClose()
      },
      onError: () => {
        setConnectingUid(null)
      },
    },
  })

  const handleConnect = (connector: Connector) => {
    setConnectingUid(connector.uid)
    // Close our modal when WalletConnect is selected so its QR modal is visible
    // (WalletConnect's modal would otherwise be hidden behind ours due to z-index)
    if (connector.type === 'walletConnect') {
      onClose()
    }
    connect({ connector })
  }

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Connect EVM Wallet">
      <div className="p-6 space-y-4">
        {error && (
          <div className="p-3 bg-red-500/10 border border-red-500/30 rounded-lg text-sm text-red-400">
            {error.message}
          </div>
        )}
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
        <p className="text-gray-400 text-sm">
          Choose a wallet to connect to this app
        </p>
        <div className="space-y-2">
          {connectors.map((connector) => (
            <EvmWalletOption
              key={connector.uid}
              connector={connector}
              onClick={() => handleConnect(connector)}
              isLoading={isPending && connectingUid === connector.uid}
              disabled={isPending}
            />
          ))}
        </div>
        {connectors.length === 0 && (
          <p className="text-gray-500 text-sm py-4">
            No wallets detected. Install MetaMask or another EVM wallet extension.
          </p>
        )}
      </div>
    </Modal>
  )
}
