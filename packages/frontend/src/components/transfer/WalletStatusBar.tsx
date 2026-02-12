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
    <div className="space-y-2">
      <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-gray-300">Source wallet</div>
      <div className="grid grid-cols-1 gap-2 border-2 border-white/20 bg-black/35 p-2 sm:grid-cols-2">
      <div
        className={`flex items-center gap-2 px-3 py-2 border text-xs uppercase tracking-wide ${
          isEvmConnected
            ? 'bg-cyan-500/12 text-cyan-100 border-cyan-300/55'
            : 'bg-[#1a1a1a] text-gray-300 border-white/20'
        }`}
      >
        <div className={`h-2 w-2 ${isEvmConnected ? 'bg-cyan-300' : 'bg-gray-600'}`} />
        <span className="text-[10px] text-gray-300">EVM</span>
        {isEvmConnected && evmAddress ? (
          <span className="font-mono normal-case text-[11px]">{formatAddress(evmAddress, 4)}</span>
        ) : (
          <button
            type="button"
            onClick={onConnectEvm}
            className="normal-case text-cyan-300 hover:text-cyan-200 underline underline-offset-2"
          >
            Connect
          </button>
        )}
        {isEvmConnected && chain && (
          <span className="text-gray-400 normal-case text-[10px]">({chain.name})</span>
        )}
      </div>
      <div
        className={`flex items-center gap-2 px-3 py-2 border text-xs uppercase tracking-wide ${
          isTerraConnected
            ? 'bg-amber-500/12 text-amber-100 border-amber-300/55'
            : 'bg-[#1a1a1a] text-gray-300 border-white/20'
        }`}
      >
        <div className={`h-2 w-2 ${isTerraConnected ? 'bg-amber-300' : 'bg-gray-600'}`} />
        <span className="text-[10px] text-gray-300">Terra</span>
        {isTerraConnected && terraAddress ? (
          <span className="font-mono normal-case text-[11px]">{formatAddress(terraAddress, 6)}</span>
        ) : (
          <button
            type="button"
            onClick={onConnectTerra}
            className="normal-case text-amber-300 hover:text-amber-200 underline underline-offset-2"
          >
            Connect
          </button>
        )}
      </div>
      </div>
    </div>
  )
}
