import { sounds } from '../../lib/sounds'

export interface SolanaWalletOptionProps {
  name: string
  icon: string
  installed: boolean
  onClick: () => void
  connecting?: boolean
}

function getSolanaWalletIcon(icon: string): React.ReactNode {
  switch (icon) {
    case 'phantom':
      return (
        <div className="w-10 h-10 rounded-xl bg-purple-500/20 flex items-center justify-center">
          <span className="text-purple-400 text-lg">👻</span>
        </div>
      )
    case 'solflare':
      return (
        <div className="w-10 h-10 rounded-xl bg-orange-500/20 flex items-center justify-center">
          <span className="text-orange-400 text-lg">🔥</span>
        </div>
      )
    case 'backpack':
      return (
        <div className="w-10 h-10 rounded-xl bg-red-500/20 flex items-center justify-center">
          <span className="text-red-400 text-lg">🎒</span>
        </div>
      )
    case 'coinbase':
      return (
        <div className="w-10 h-10 rounded-xl bg-blue-600/20 flex items-center justify-center">
          <span className="text-blue-400 text-lg">◆</span>
        </div>
      )
    default:
      return (
        <div className="w-10 h-10 rounded-xl bg-gray-600/20 flex items-center justify-center">
          <span className="text-gray-400 text-lg">{icon.charAt(0).toUpperCase()}</span>
        </div>
      )
  }
}

export function SolanaWalletOption({
  name,
  icon,
  installed,
  onClick,
  connecting,
}: SolanaWalletOptionProps) {
  return (
    <button
      type="button"
      onClick={() => {
        sounds.playButtonPress()
        onClick()
      }}
      disabled={!installed || connecting}
      className="w-full flex items-center gap-4 p-4 rounded-xl border border-white/5 hover:border-purple-500/40 hover:bg-purple-500/5 transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:border-white/5 disabled:hover:bg-transparent"
    >
      {getSolanaWalletIcon(icon)}
      <div className="flex-1 text-left">
        <p className="font-medium text-white">{name}</p>
        {!installed && (
          <p className="text-xs text-gray-500">Not installed</p>
        )}
      </div>
      {connecting ? (
        <div className="w-5 h-5 border-2 border-purple-500 border-t-transparent rounded-full animate-spin" />
      ) : (
        installed && (
          <svg
            className="w-5 h-5 text-gray-500"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
        )
      )}
    </button>
  )
}
