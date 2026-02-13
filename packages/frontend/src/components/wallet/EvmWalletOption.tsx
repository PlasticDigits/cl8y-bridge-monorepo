import type { Connector } from 'wagmi'
import { sounds } from '../../lib/sounds'

export interface EvmWalletOptionProps {
  connector: Connector
  onClick: () => void
  isLoading?: boolean
  disabled?: boolean
}

function getWalletIcon(connector: Connector): React.ReactNode {
  const name = (connector.name || '').toLowerCase()
  if (name.includes('metamask')) {
    return (
      <div className="w-10 h-10 rounded-xl bg-orange-500/20 flex items-center justify-center">
        <span className="text-orange-400 text-lg">🦊</span>
      </div>
    )
  }
  if (name.includes('rabby')) {
    return (
      <div className="w-10 h-10 rounded-xl bg-blue-500/20 flex items-center justify-center">
        <span className="text-blue-400 text-lg">🐰</span>
      </div>
    )
  }
  if (name.includes('coinbase')) {
    return (
      <div className="w-10 h-10 rounded-xl bg-blue-600/20 flex items-center justify-center">
        <span className="text-blue-400 text-lg">◆</span>
      </div>
    )
  }
  if (name.includes('walletconnect')) {
    return (
      <div className="w-10 h-10 rounded-xl bg-indigo-500/20 flex items-center justify-center">
        <svg className="w-6 h-6 text-indigo-400" viewBox="0 0 32 32" fill="none">
          <path
            fill="currentColor"
            d="M9 7a5 5 0 015 5h0a5 5 0 015-5M9 19a5 5 0 015-5h0a5 5 0 015 5M9 13a3 3 0 013 3h8a3 3 0 013-3"
          />
        </svg>
      </div>
    )
  }
  return (
    <div className="w-10 h-10 rounded-xl bg-gray-600/20 flex items-center justify-center">
      <svg className="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1"
        />
      </svg>
    </div>
  )
}

export function EvmWalletOption({ connector, onClick, isLoading, disabled }: EvmWalletOptionProps) {
  const displayName =
    connector.type === 'mock' ? 'Simulated EVM Wallet' : connector.name || 'Wallet'

  return (
    <button
      type="button"
      onClick={() => {
        sounds.playButtonPress()
        onClick()
      }}
      disabled={disabled}
      className="w-full flex items-center gap-4 p-4 rounded-xl border border-white/5 hover:border-blue-500/40 hover:bg-blue-500/5 transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:border-white/5 disabled:hover:bg-transparent"
    >
      {getWalletIcon(connector)}
      <div className="flex-1 text-left">
        <p className="font-medium text-white">{displayName}</p>
      </div>
      {isLoading ? (
        <div className="w-5 h-5 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
      ) : (
        <svg
          className="w-5 h-5 text-gray-500"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>
      )}
    </button>
  )
}
