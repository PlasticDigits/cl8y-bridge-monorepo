import { useState, useRef, useEffect } from 'react'
import { TokenLogo } from '../ui'
import { getTokenDisplaySymbol } from '../../utils/tokenLogos'

export interface TokenOption {
  id: string
  symbol: string
  tokenId: string
}

export interface TokenSelectProps {
  tokens: TokenOption[]
  value: string
  onChange: (tokenId: string) => void
  id?: string
  disabled?: boolean
}

export function TokenSelect({ tokens, value, onChange, id = 'token-select', disabled }: TokenSelectProps) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

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
  if (tokens.length === 1 && !disabled) {
    return (
      <div className="flex items-center gap-2">
        <TokenLogo symbol={selected?.symbol} tokenId={selected?.tokenId} size={18} />
        <span className="text-xs uppercase tracking-wide text-gray-400">{selected?.symbol}</span>
      </div>
    )
  }

  return (
    <div ref={containerRef} className="relative">
      <button
        id={id}
        type="button"
        role="combobox"
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls={`${id}-listbox`}
        aria-label="Select token"
        onClick={() => !disabled && setOpen((o) => !o)}
        disabled={disabled}
        className="flex cursor-pointer items-center gap-2 rounded border border-white/20 bg-white/5 px-2 py-1 text-xs uppercase tracking-wide text-gray-400 hover:bg-white/10 hover:text-gray-300 focus:border-cyan-300 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
      >
        <TokenLogo symbol={selected?.symbol} tokenId={selected?.tokenId} size={18} />
        <span>{selected?.symbol}</span>
        <svg
          className={`h-3 w-3 shrink-0 text-gray-400 transition-transform ${open ? 'rotate-180' : ''}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2.5}
        >
          <path d="M6 9l6 6 6-6" strokeLinecap="square" strokeLinejoin="miter" />
        </svg>
      </button>

      {open && (
        <ul
          id={`${id}-listbox`}
          role="listbox"
          aria-labelledby={id}
          className="absolute right-0 top-full z-50 mt-1 max-h-48 min-w-[120px] overflow-auto border-2 border-white/20 bg-[#161616] shadow-lg"
        >
          {tokens.map((token) => {
            const isSelected = token.id === value
            return (
              <li
                key={token.id}
                role="option"
                aria-selected={isSelected}
                data-tokenid={token.id}
                onClick={() => {
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
                <TokenLogo symbol={token.symbol} tokenId={token.tokenId} size={18} />
                <span className="truncate">{token.symbol}</span>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
