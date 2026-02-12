import { useState } from 'react'
import { useWallet } from '../hooks/useWallet'
import { formatAddress, formatAmount } from '../utils/format'
import { DECIMALS } from '../utils/constants'
import { TerraWalletModal } from './wallet/TerraWalletModal'

export function WalletButton() {
  const {
    connected,
    connecting,
    address,
    luncBalance,
    showWalletModal,
    disconnect,
    setShowWalletModal,
  } = useWallet()

  const [showDropdown, setShowDropdown] = useState(false)

  if (connected && address) {
    return (
      <div className="relative">
        <button
          onClick={() => setShowDropdown(!showDropdown)}
          className="flex items-center gap-2 sm:gap-3 px-3 sm:px-4 py-2 glass border-2 border-white/30 hover:border-white/60 rounded-none transition-all group shadow-[3px_3px_0_#000]"
        >
          <div className="text-right hidden sm:block">
            <p className="text-sm font-mono font-medium text-white">
              {formatAmount(luncBalance, DECIMALS.LUNC)} <span className="text-blue-400">LUNC</span>
            </p>
            <p className="text-xs text-gray-500">{formatAddress(address, 6)}</p>
          </div>
          <div className="w-8 h-8 border-2 border-black bg-[#b8ff3d] shadow-[2px_2px_0_#000] transition-shadow" />
        </button>

        {showDropdown && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setShowDropdown(false)} />
            <div className="absolute right-0 mt-2 w-48 glass border-2 border-white/35 rounded-none shadow-[4px_4px_0_#000] overflow-hidden z-50 animate-fade-in-up" style={{ animationDuration: '0.2s' }}>
              <div className="p-2">
                <div className="px-3 py-2 sm:hidden">
                  <p className="text-sm font-mono text-white">{formatAmount(luncBalance, DECIMALS.LUNC)} LUNC</p>
                  <p className="text-xs text-gray-500">{formatAddress(address, 8)}</p>
                </div>
                <button
                  onClick={() => {
                    disconnect()
                    setShowDropdown(false)
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2.5 text-left text-sm text-gray-300 hover:bg-white/5 hover:text-red-400 rounded-lg transition-colors"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                  </svg>
                  Disconnect
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    )
  }

  return (
    <>
      <button
        onClick={() => setShowWalletModal(true)}
        disabled={connecting}
        className="btn-primary disabled:opacity-60 disabled:cursor-not-allowed"
      >
        <span className="flex items-center gap-2">
          {connecting ? (
            <>
              <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
              <span className="hidden sm:inline">Connecting...</span>
            </>
          ) : (
            <>
              <svg className="w-4 h-4 sm:hidden" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2z" />
              </svg>
              <span className="hidden sm:inline">Connect Wallet</span>
              <span className="sm:hidden">Connect</span>
            </>
          )}
        </span>
      </button>

      <TerraWalletModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
    </>
  )
}
