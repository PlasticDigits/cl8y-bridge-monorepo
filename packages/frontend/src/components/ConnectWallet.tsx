import { useAccount, useDisconnect } from 'wagmi'
import { useUIStore } from '../stores/ui'
import { EvmWalletModal } from './wallet'

export function ConnectWallet() {
  const { address, isConnected, chain } = useAccount()
  const { disconnect } = useDisconnect()
  const { showEvmWalletModal, setShowEvmWalletModal } = useUIStore()

  if (isConnected && address) {
    return (
      <div className="flex items-center gap-2 sm:gap-3">
        <div className="flex items-center gap-2 rounded-lg px-3 py-2 border border-emerald-400/30 bg-emerald-950/35">
          <div className="w-2 h-2 bg-emerald-400 rounded-full" />
          <span className="text-emerald-100 text-xs sm:text-sm">{chain?.name || 'Unknown'}</span>
        </div>
        <button
          onClick={() => disconnect()}
          className="btn-muted"
        >
          <span className="text-white text-xs sm:text-sm font-medium">
            {address.slice(0, 6)}...{address.slice(-4)}
          </span>
          <svg className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
          </svg>
        </button>
      </div>
    )
  }

  return (
    <>
      <button
        onClick={() => setShowEvmWalletModal(true)}
        className="btn-primary"
      >
        <svg className="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"
          />
        </svg>
        <span className="text-slate-950 text-sm font-semibold">Connect EVM</span>
      </button>
      <EvmWalletModal isOpen={showEvmWalletModal} onClose={() => setShowEvmWalletModal(false)} />
    </>
  )
}
