/**
 * TokenAmountDisplay — Resolves token symbol + decimals from bytes32, then renders:
 *   compact amount (5 sigfigs) + symbol
 *   collapsible raw view: raw bigint, token address, decimals
 */

import { useState, useMemo } from 'react'
import type { Hex } from 'viem'
import type { BridgeChainConfig } from '../../types/chain'
import { useEvmTokenInfo } from '../../hooks/useTokenOnchainInfo'
import { bytes32ToAddress } from '../../services/evm/tokenRegistry'
import { formatCompact, formatAmount } from '../../utils/format'
import { getTokenDisplaySymbol } from '../../utils/tokenLogos'

export interface TokenAmountDisplayProps {
  /** Raw amount as stored on-chain (bigint) */
  amount: bigint
  /** Token identifier as bytes32 hex */
  tokenBytes32: Hex
  /** Chain config where this token lives (for ERC20 RPC queries) */
  chainConfig: BridgeChainConfig | null
}

const ZERO_ADDR = '0x0000000000000000000000000000000000000000'

function isAllZeroPrefix(bytes32: string): boolean {
  const clean = bytes32.startsWith('0x') ? bytes32.slice(2) : bytes32
  return clean.slice(0, 24) === '000000000000000000000000'
}

export function TokenAmountDisplay({ amount, tokenBytes32, chainConfig }: TokenAmountDisplayProps) {
  const [expanded, setExpanded] = useState(false)

  const evmAddress = useMemo(() => {
    try {
      if (!tokenBytes32 || tokenBytes32.length < 42) return null
      const addr = bytes32ToAddress(tokenBytes32)
      if (addr === ZERO_ADDR) return null
      return addr
    } catch {
      return null
    }
  }, [tokenBytes32])

  const terraDenom = useMemo(() => {
    if (!tokenBytes32 || isAllZeroPrefix(tokenBytes32)) return null
    try {
      const clean = tokenBytes32.startsWith('0x') ? tokenBytes32.slice(2) : tokenBytes32
      const bytes = new Uint8Array(clean.match(/.{2}/g)!.map((b) => parseInt(b, 16)))
      const firstNull = bytes.indexOf(0)
      const text = new TextDecoder().decode(firstNull > 0 ? bytes.slice(0, firstNull) : bytes)
      if (/^[a-z][a-z0-9/]{2,44}$/.test(text)) return text
    } catch { /* not a denom */ }
    return null
  }, [tokenBytes32])

  const isEvm = chainConfig?.type === 'evm'
  const shouldQueryEvm = isEvm && !!evmAddress
  const rpcConfig = shouldQueryEvm ? chainConfig! : undefined

  const { data: evmInfo } = useEvmTokenInfo(
    evmAddress ?? undefined,
    rpcConfig,
    shouldQueryEvm
  )

  const symbol = useMemo(() => {
    if (evmInfo?.symbol) return evmInfo.symbol
    if (terraDenom) return getTokenDisplaySymbol(terraDenom)
    if (evmAddress) return evmAddress.slice(0, 6) + '…' + evmAddress.slice(-4)
    return '???'
  }, [evmInfo, terraDenom, evmAddress])

  const decimals = useMemo(() => {
    if (evmInfo?.decimals != null) return evmInfo.decimals
    if (terraDenom === 'uluna' || terraDenom === 'uusd') return 6
    return 18
  }, [evmInfo, terraDenom])

  const compactAmount = formatCompact(amount, decimals, 5)
  const fullAmount = formatAmount(amount, decimals)
  const tokenAddress = evmAddress ?? terraDenom ?? tokenBytes32

  return (
    <span className="inline">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="group inline-flex items-center gap-1 text-left text-white hover:text-[#b8ff3d] transition-colors"
        title="Click to show raw details"
      >
        <span>{compactAmount} {symbol}</span>
        <span className="text-[10px] text-gray-500 group-hover:text-[#b8ff3d] transition-colors">
          {expanded ? '▾' : '▸'}
        </span>
      </button>
      {expanded && (
        <span className="mt-1.5 block space-y-0.5 border border-white/10 bg-black/40 px-2 py-1.5 text-[11px]">
          <span className="block">
            <span className="text-gray-500">Full:</span>{' '}
            <span className="text-gray-300 font-mono">{fullAmount} {symbol}</span>
          </span>
          <span className="block">
            <span className="text-gray-500">Raw:</span>{' '}
            <span className="text-gray-300 font-mono">{amount.toString()}</span>
          </span>
          <span className="block">
            <span className="text-gray-500">Token:</span>{' '}
            <span className="text-gray-300 font-mono break-all">{tokenAddress}</span>
          </span>
          <span className="block">
            <span className="text-gray-500">Decimals:</span>{' '}
            <span className="text-gray-300 font-mono">{decimals}</span>
          </span>
        </span>
      )}
    </span>
  )
}
