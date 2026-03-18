import { useState } from 'react'
import { Modal } from '../ui'
import { SolanaWalletOption } from './SolanaWalletOption'
import { detectSolanaWallets } from '../../services/solana/detect'
import { useSolanaWallet } from '../../hooks/useSolanaWallet'
import type { SolanaWalletType } from '../../stores/solanaWallet'

export interface SolanaWalletModalProps {
  isOpen: boolean
  onClose: () => void
}

export function SolanaWalletModal({ isOpen, onClose }: SolanaWalletModalProps) {
  const { connect, connecting, connectionError } = useSolanaWallet()
  const [connectingWallet, setConnectingWallet] = useState<string | null>(null)

  const wallets = detectSolanaWallets()

  const handleConnect = (walletType: SolanaWalletType) => {
    setConnectingWallet(walletType)
    connect(walletType).then(() => {
      setConnectingWallet(null)
      onClose()
    }).catch(() => {
      setConnectingWallet(null)
    })
  }

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Connect Solana Wallet">
      <div className="p-6 space-y-4">
        {connectionError && (
          <div className="p-3 bg-red-500/10 border border-red-500/30 rounded-lg text-sm text-red-400">
            {connectionError}
          </div>
        )}
        <p className="text-gray-400 text-sm">
          Choose a Solana wallet to connect
        </p>
        <div className="space-y-2">
          {wallets.map((wallet) => (
            <SolanaWalletOption
              key={wallet.name}
              name={wallet.name}
              icon={wallet.icon}
              installed={wallet.installed}
              connecting={connecting && connectingWallet === wallet.icon}
              onClick={() =>
                handleConnect(
                  wallet.name.toLowerCase().replace(' wallet', '') as SolanaWalletType
                )
              }
            />
          ))}
        </div>
        {wallets.every((w) => !w.installed) && (
          <p className="text-gray-500 text-sm py-4">
            No Solana wallets detected. Install Phantom, Solflare, or Backpack.
          </p>
        )}
      </div>
    </Modal>
  )
}
