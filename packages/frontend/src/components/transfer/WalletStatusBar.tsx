import { useAccount } from 'wagmi'
import { useWallet } from '../../hooks/useWallet'
import { formatAddress } from '../../utils/format'

export interface WalletStatusBarProps {
  onConnectEvm?: () => void
  onConnectTerra?: () => void
}

export function WalletStatusBar({ onConnectEvm, onConnectTerra }: WalletStatusBarProps) {
  const { isConnected: isEvmConnected, address: evmAddress, chain } = useAccount()
  const { connected: isTerraConnected, address: terraAddress } = useWallet()

  return (
    <div className="flex flex-wrap gap-2">
      <div
        className={`flex items-center gap-2 px-3 py-2 rounded-lg text-sm ${
          isEvmConnected
            ? 'bg-green-900/30 text-green-400 border border-green-700/50'
            : 'bg-gray-800 text-gray-500 border border-gray-700/50'
        }`}
      >
        <div className={`w-2 h-2 rounded-full ${isEvmConnected ? 'bg-green-500' : 'bg-gray-600'}`} />
        <span>EVM:</span>
        {isEvmConnected && evmAddress ? (
          <span className="font-mono">{formatAddress(evmAddress, 4)}</span>
        ) : (
          <button
            type="button"
            onClick={onConnectEvm}
            className="text-blue-400 hover:text-blue-300 underline"
          >
            Connect
          </button>
        )}
        {isEvmConnected && chain && (
          <span className="text-gray-500 text-xs">({chain.name})</span>
        )}
      </div>
      <div
        className={`flex items-center gap-2 px-3 py-2 rounded-lg text-sm ${
          isTerraConnected
            ? 'bg-green-900/30 text-green-400 border border-green-700/50'
            : 'bg-gray-800 text-gray-500 border border-gray-700/50'
        }`}
      >
        <div className={`w-2 h-2 rounded-full ${isTerraConnected ? 'bg-green-500' : 'bg-gray-600'}`} />
        <span>Terra:</span>
        {isTerraConnected && terraAddress ? (
          <span className="font-mono">{formatAddress(terraAddress, 6)}</span>
        ) : (
          <button
            type="button"
            onClick={onConnectTerra}
            className="text-amber-400 hover:text-amber-300 underline"
          >
            Connect
          </button>
        )}
      </div>
    </div>
  )
}
