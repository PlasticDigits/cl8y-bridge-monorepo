import { useEffect, useState } from 'react'
import { formatAddress } from '../utils/format'
import { sounds } from '../lib/sounds'
import { useAccount, useBalance, useDisconnect } from 'wagmi'
import { useUIStore } from '../stores/ui'

function getGasSymbol(chainId?: number): 'ETH' | 'BNB' {
  if (chainId === 56 || chainId === 97 || chainId === 204 || chainId === 5611) {
    return 'BNB'
  }
  return 'ETH'
}

function getChainLogoPath(chainId?: number): string | undefined {
  if (chainId === 56 || chainId === 97) return '/chains/binancesmartchain-icon.png'
  if (chainId === 204 || chainId === 5611) return '/chains/opbnb-icon.png'
  if (chainId === 31337) return '/chains/anvil-icon.png'
  if (chainId === 31338) return '/chains/anvil2-icon.png'
  if (chainId === 1) return '/chains/ethereum-icon.png'
  return undefined
}

function formatGasBalance(formattedBalance?: string): string {
  const parsed = Number.parseFloat(formattedBalance ?? '0')
  if (!Number.isFinite(parsed)) return '0.00'
  return parsed.toLocaleString('en-US', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  })
}

export function ConnectWallet() {
  const { address, isConnected, chain } = useAccount()
  const { disconnect } = useDisconnect()
  const { setShowEvmWalletModal } = useUIStore()
  const gasSymbol = getGasSymbol(chain?.id)
  const chainLogoPath = getChainLogoPath(chain?.id)
  const [logoLoadFailed, setLogoLoadFailed] = useState(false)
  const { data: gasBalance, isLoading: isGasBalanceLoading } = useBalance({
    address,
    chainId: chain?.id,
    query: {
      enabled: isConnected && !!address && !!chain?.id,
      refetchInterval: 30000,
    },
  })

  useEffect(() => {
    setLogoLoadFailed(false)
  }, [chainLogoPath])

  const [showDropdown, setShowDropdown] = useState(false)
  if (isConnected && address) {
    return (
      <div className="relative">
        <button
          onClick={() => {
            sounds.playButtonPress()
            setShowDropdown(!showDropdown)
          }}
          className="flex items-center gap-2 sm:gap-3 px-3 sm:px-4 py-2 glass border-2 border-white/30 hover:border-white/60 rounded-none transition-all group shadow-[3px_3px_0_#000]"
        >
          <div className="text-right hidden sm:block">
            <p className="text-sm font-mono font-medium text-white">
              {isGasBalanceLoading ? '--' : formatGasBalance(gasBalance?.formatted)}{' '}
              <span className="text-emerald-300">{gasSymbol}</span>
            </p>
            <p className="text-xs text-gray-500">{formatAddress(address, 6)}</p>
          </div>
          <div className="sm:hidden text-xs font-mono font-medium text-white">
            {isGasBalanceLoading ? '--' : formatGasBalance(gasBalance?.formatted)}{' '}
            <span className="text-emerald-300">{gasSymbol}</span>
          </div>
          <div className="w-8 h-8 shrink-0 flex items-center justify-center overflow-hidden rounded-sm bg-black/90 p-1 border-2 border-black shadow-[2px_2px_0_#000]">
            {chainLogoPath && !logoLoadFailed ? (
              <img
                src={chainLogoPath}
                alt={chain?.name ?? 'Chain'}
                className="h-full w-full object-contain"
                onError={() => setLogoLoadFailed(true)}
              />
            ) : (
              <span className="text-[9px] font-black text-white leading-none tracking-tight">{gasSymbol}</span>
            )}
          </div>
        </button>

        {showDropdown && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setShowDropdown(false)} />
            <div className="absolute right-0 mt-2 w-48 glass border-2 border-white/35 rounded-none shadow-[4px_4px_0_#000] overflow-hidden z-50 animate-fade-in-up" style={{ animationDuration: '0.2s' }}>
              <div className="p-2">
                <div className="px-3 py-2 sm:hidden">
                  <p className="text-sm font-mono text-white">
                    {isGasBalanceLoading ? '--' : formatGasBalance(gasBalance?.formatted)} {gasSymbol}
                  </p>
                  <p className="text-xs text-gray-500">{formatAddress(address, 8)}</p>
                </div>
                <button
                  onClick={() => {
                    sounds.playButtonPress()
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
    <button
      onClick={() => {
        sounds.playButtonPress()
        setShowEvmWalletModal(true)
      }}
      className="btn-primary"
    >
      <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded bg-black p-0.5">
        <img src="/chains/ethereum-icon.png" alt="" className="h-full w-full object-contain" />
      </span>
      <span className="text-slate-950 text-sm font-semibold whitespace-nowrap hidden min-[470px]:inline">CONNECT EVM</span>
      <span className="text-slate-950 text-sm font-semibold whitespace-nowrap min-[470px]:hidden">EVM</span>
    </button>
  )
}
