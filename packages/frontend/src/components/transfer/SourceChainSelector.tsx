import { useState, useEffect } from 'react'
import type { ChainInfo } from '../../lib/chains'
import { ChainSelect } from './ChainSelect'
export interface SourceChainSelectorProps {
  chains: ChainInfo[]
  value: string
  onChange: (chainId: string) => void
  balance?: string
  balanceLabel?: string
  bridgeMax?: string
  periodEndsAt?: number
  fetchedAtWallMs?: number
  windowActive?: boolean
  disabled?: boolean
}
export function SourceChainSelector({
  chains,
  value,
  onChange,
  balance,
  balanceLabel,
  bridgeMax,
  periodEndsAt,
  fetchedAtWallMs,
  windowActive,
  disabled,
}: SourceChainSelectorProps) {
  const [countdown, setCountdown] = useState('')
  useEffect(() => {
    if (!periodEndsAt || !fetchedAtWallMs) { setCountdown(''); return }
    if (!windowActive) { setCountdown('N/A'); return }
    const tick = () => {
      const elapsed = Date.now() - fetchedAtWallMs
      const remainMs = periodEndsAt * 1000 - (fetchedAtWallMs + elapsed)
      if (remainMs <= 0) { setCountdown('00:00:00'); return }
      const h = Math.floor(remainMs / 3600000)
      const m = Math.floor((remainMs % 3600000) / 60000)
      const s = Math.floor((remainMs % 60000) / 1000)
      setCountdown(
        String(h).padStart(2, '0') + ':' +
        String(m).padStart(2, '0') + ':' +
        String(s).padStart(2, '0')
      )
    }
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [periodEndsAt, fetchedAtWallMs, windowActive])
  return (
    <div data-testid="source-chain">
      <ChainSelect
        chains={chains}
        value={value}
        onChange={onChange}
        label="From"
        id="source-chain-select"
        disabled={disabled}
      />
      <p className="mt-1 text-[11px] uppercase tracking-wide text-gray-400">
        {bridgeMax !== undefined && (
          <>
            <span className="text-cyan-400/90">Max: {bridgeMax}</span>
            {countdown && <span className="text-cyan-400/60 ml-1">{countdown}</span>}
            {balance !== undefined && <span className="mx-1.5">|</span>}
          </>
        )}
        {balance !== undefined && (
          <span>Balance: {balance} {balanceLabel ?? ''}</span>
        )}
      </p>
    </div>
  )
}
