import { useMemo, useState } from 'react'
import { useConnect } from 'wagmi'
import type { Connector } from 'wagmi'
import { getConnectors } from 'wagmi/actions'
import { Modal } from '../ui'
import { EvmWalletOption } from './EvmWalletOption'
import { useEvmWalletDiscovery } from '../../hooks/useEvmWalletDiscovery'
import { detectInAppBrowser } from '../../utils/detectInAppBrowser'
import { StorageJitPrompt, type StorageJitKind } from '../legal/StorageJitPrompt'
import { useStorageConsentActions } from '../../hooks/useStorageConsent'
import { evmConnectorJitKind } from '../../utils/evmConnectorConsent'
import { allowsCoinbaseInfra, allowsWalletConnectInfra, subscribeStorageConsent } from '../../lib/storageConsent'
import { getWagmiConfig } from '../../lib/wagmi'
import { WC_PROJECT_ID } from '../../utils/constants'
import { sounds } from '../../lib/sounds'

export interface EvmWalletModalProps {
  isOpen: boolean
  onClose: () => void
}

function findOptionalConnector(kind: StorageJitKind): Connector | undefined {
  const connectors = getConnectors(getWagmiConfig())
  return connectors.find((connector) => {
    const name = (connector.name || '').toLowerCase()
    const id = (connector.uid || '').toLowerCase()
    if (kind === 'walletConnect') {
      return id.includes('walletconnect') || name.includes('walletconnect') || connector.type === 'walletConnect'
    }
    return id.includes('coinbase') || name.includes('coinbase')
  })
}

function GatedConnectorRow({
  label,
  onClick,
  disabled,
}: {
  label: string
  onClick: () => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      onClick={() => {
        sounds.playButtonPress()
        onClick()
      }}
      disabled={disabled}
      className="w-full flex items-center gap-4 p-4 rounded-xl border border-white/5 hover:border-blue-500/40 hover:bg-blue-500/5 transition-all duration-200 disabled:opacity-50"
    >
      <div className="flex-1 text-left">
        <p className="font-medium text-white">{label}</p>
        <p className="text-xs text-gray-500">Requires optional unlock</p>
      </div>
    </button>
  )
}

export function EvmWalletModal({ isOpen, onClose }: EvmWalletModalProps) {
  const { connectors } = useEvmWalletDiscovery()
  const [connectingUid, setConnectingUid] = useState<string | null>(null)
  const [pendingJitKind, setPendingJitKind] = useState<StorageJitKind | null>(null)
  const { jitWalletConnect, jitCoinbase } = useStorageConsentActions()
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

  const runConnect = (connector: Connector) => {
    setConnectingUid(connector.uid)
    if (connector.type === 'walletConnect') {
      onClose()
    }
    connect({ connector })
  }

  const handleConnect = (connector: Connector) => {
    const jit = evmConnectorJitKind(connector)
    if (jit) {
      setPendingJitKind(jit)
      return
    }
    runConnect(connector)
  }

  const waitForConsentRefresh = () =>
    new Promise<void>((resolve) => {
      const done = () => {
        clearTimeout(fallback)
        unsub()
        resolve()
      }
      const unsub = subscribeStorageConsent(done)
      const fallback = window.setTimeout(done, 400)
    })

  const confirmJit = async () => {
    if (!pendingJitKind) return
    if (pendingJitKind === 'walletConnect') jitWalletConnect()
    else jitCoinbase()
    const kind = pendingJitKind
    setPendingJitKind(null)
    await waitForConsentRefresh()
    const connector = findOptionalConnector(kind)
    if (connector) runConnect(connector)
  }

  const showGatedWalletConnect = Boolean(WC_PROJECT_ID) && !allowsWalletConnectInfra()
  const showGatedCoinbase = !allowsCoinbaseInfra()

  return (
    <>
      <StorageJitPrompt
        kind={pendingJitKind}
        onCancel={() => setPendingJitKind(null)}
        onConfirm={() => void confirmJit()}
      />
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
          <p className="text-gray-400 text-sm">Choose a wallet to connect to this app</p>
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
            {showGatedWalletConnect && (
              <GatedConnectorRow
                label="WalletConnect"
                disabled={isPending}
                onClick={() => setPendingJitKind('walletConnect')}
              />
            )}
            {showGatedCoinbase && (
              <GatedConnectorRow
                label="Coinbase Wallet"
                disabled={isPending}
                onClick={() => setPendingJitKind('coinbase')}
              />
            )}
          </div>
          {connectors.length === 0 && !showGatedWalletConnect && !showGatedCoinbase && (
            <p className="text-gray-500 text-sm py-4">
              No wallets detected. Install MetaMask or another EVM wallet extension.
            </p>
          )}
        </div>
      </Modal>
    </>
  )
}
