import { useCallback } from 'react'
import { useWallet, WalletName, WalletType } from '../../hooks/useWallet'
import { Modal } from '../ui'
import { TerraWalletOption, getTerraWalletIcon } from './TerraWalletOption'

export interface TerraWalletModalProps {
  isOpen: boolean
  onClose: () => void
}

export function TerraWalletModal({ isOpen, onClose }: TerraWalletModalProps) {
  const {
    connecting,
    connectingWallet,
    isStationAvailable,
    isKeplrAvailable,
    isLeapAvailable,
    isCosmostationAvailable,
    connect,
    cancelConnection,
  } = useWallet()

  // Modal already handles Escape key - this just adds cancelConnection on close
  const closeModal = useCallback(() => {
    onClose()
    if (connecting) cancelConnection()
  }, [connecting, cancelConnection, onClose])

  const handleConnect = async (walletName: WalletName, walletType: WalletType = WalletType.EXTENSION) => {
    try {
      await connect(walletName, walletType)
      onClose()
    } catch {
      // Error is shown via useWallet / store
    }
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
        <p className="text-xs text-amber-500/70 uppercase tracking-wider mb-2 font-medium">Browser Extension</p>
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
        {wallets.slice(4).map((w) => (
          <TerraWalletOption
            key={w.walletName}
            name={w.name}
            description={w.description}
            available={w.available}
            loading={connectingWallet === w.walletName}
            onClick={() => handleConnect(w.walletName, WalletType.WALLETCONNECT)}
            disabled={connecting}
            icon={getTerraWalletIcon(w.walletName)}
          />
        ))}
      </div>
    </Modal>
  )
}
