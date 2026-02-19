import { useState, useRef, useEffect } from 'react'
import { TokenLogo } from '../ui'
import { sounds } from '../../lib/sounds'
import { useTokenList } from '../../hooks/useTokenList'
import { useTokenOptionsDisplayMap } from '../../hooks/useTokenDisplayInfo'
import { getTokenFromList } from '../../services/tokenlist'
import { isAddressLike, shortenAddress } from '../../utils/shortenAddress'

export interface TokenOption {
  id: string
  symbol: string
  tokenId: string
  /** EVM token address when source is EVM - used for onchain symbol lookup */
  evmTokenAddress?: string
}

export interface TokenSelectProps {
  tokens: TokenOption[]
  value: string
  onChange: (tokenId: string) => void
  id?: string
  disabled?: boolean
  /** Source chain RPC URL when source is EVM - enables onchain symbol lookup for dropdown */
  sourceChainRpcUrl?: string
}

function getDisplayLabel(
  token: TokenOption,
  tokenlist: { symbol: string; name?: string } | null,
  displayMap?: Record<string, string>
): string {
  const fromMap = displayMap?.[token.id]
  if (fromMap) return fromMap
  const fromList = tokenlist?.symbol
  const fallback = token.symbol
  const label = fromList ?? fallback
  return isAddressLike(label) ? shortenAddress(label) : label
}

function getAddressForBlockie(token: TokenOption): string | undefined {
  if (token.evmTokenAddress && token.evmTokenAddress.startsWith('0x')) {
    return token.evmTokenAddress
  }
  return isAddressLike(token.tokenId) ? token.tokenId : undefined
}

export function TokenSelect({ tokens, value, onChange, id = 'token-select', disabled, sourceChainRpcUrl }: TokenSelectProps) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const { data: tokenlist } = useTokenList()
  const displayMap = useTokenOptionsDisplayMap(tokens, sourceChainRpcUrl)

  const selected = tokens.find((t) => t.id === value) ?? tokens[0]

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    if (open) {
      document.addEventListener('mousedown', handleClickOutside)
    }
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [open])

  if (tokens.length === 0) return null

  const canOpen = tokens.length > 1

  return (
    <div ref={containerRef} className="relative z-30">
      <button
        id={id}
        type="button"
        role="combobox"
        data-testid="token-select"
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls={`${id}-listbox`}
        aria-label="Select token"
        onClick={() => {
          if (canOpen && !disabled) {
            sounds.playButtonPress()
            setOpen((o) => !o)
          }
        }}
        disabled={disabled}
        className={`flex items-center gap-2 rounded border border-white/20 bg-white/5 px-2 py-1 text-xs uppercase tracking-wide text-gray-400 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50 ${
          canOpen ? 'cursor-pointer hover:bg-white/10 hover:text-gray-300 focus:border-cyan-300' : 'cursor-default'
        }`}
      >
        <TokenLogo
          symbol={selected?.symbol}
          tokenId={selected?.tokenId}
          addressForBlockie={selected ? getAddressForBlockie(selected) : undefined}
          size={18}
        />
        <span>{selected ? getDisplayLabel(selected, tokenlist ? getTokenFromList(tokenlist, selected.id) : null, displayMap) : '—'}</span>
        {canOpen && (
          <svg
            className={`h-3 w-3 shrink-0 text-gray-400 transition-transform ${open ? 'rotate-180' : ''}`}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2.5}
          >
            <path d="M6 9l6 6 6-6" strokeLinecap="square" strokeLinejoin="miter" />
          </svg>
        )}
      </button>

      {open && (
        <ul
          id={`${id}-listbox`}
          role="listbox"
          aria-labelledby={id}
          className="absolute right-0 top-full z-[9999] mt-1 max-h-72 min-w-[180px] overflow-y-auto overflow-x-hidden border-2 border-gray-700 bg-[#161616] shadow-xl"
        >
          {tokens.map((token) => {
            const isSelected = token.id === value
            const listMatch = tokenlist ? getTokenFromList(tokenlist, token.id) : null
            const displayLabel = getDisplayLabel(token, listMatch, displayMap)
            const addressForBlockie = getAddressForBlockie(token)
            return (
              <li
                key={token.id}
                role="option"
                aria-selected={isSelected}
                data-tokenid={token.id}
                onClick={() => {
                  sounds.playButtonPress()
                  onChange(token.id)
                  setOpen(false)
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    onChange(token.id)
                    setOpen(false)
                  }
                }}
                tabIndex={0}
                className={`flex cursor-pointer items-center gap-2 px-3 py-2 text-sm text-white hover:bg-white/10 ${
                  isSelected ? 'bg-cyan-900/30' : ''
                }`}
              >
                <TokenLogo
                  symbol={token.symbol}
                  tokenId={token.tokenId}
                  addressForBlockie={addressForBlockie}
                  size={18}
                />
                <span className="truncate">{displayLabel}</span>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
