import { useState, useRef, useEffect } from 'react'
import type { ChainInfo } from '../../lib/chains'
import { isIconImagePath } from '../../utils/chainlist'

export interface ChainSelectProps {
  chains: ChainInfo[]
  value: string
  onChange: (chainId: string) => void
  label: string
  id?: string
}

export function ChainSelect({
  chains,
  value,
  onChange,
  label,
  id = 'chain-select',
}: ChainSelectProps) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  const selected = chains.find((c) => c.id === value) ?? chains[0]

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

  return (
    <div ref={containerRef} className="relative">
      <label
        id={`${id}-label`}
        htmlFor={id}
        className="mb-1 block font-['Chakra_Petch'] text-sm font-bold uppercase tracking-wider text-[#b8ff3d] [text-shadow:1px_1px_0_#000]"
      >
        {label}
      </label>
      <button
        id={id}
        type="button"
        role="combobox"
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls={`${id}-listbox`}
        aria-labelledby={label ? `${id}-label` : undefined}
        onClick={() => setOpen((o) => !o)}
        className="flex w-full cursor-pointer items-center gap-2 border-2 border-white/20 bg-[#161616] px-3 py-2 text-left text-sm text-white focus:border-cyan-300 focus:outline-none"
      >
        {isIconImagePath(selected.icon) ? (
          <img
            src={selected.icon}
            alt=""
            className="h-5 w-5 shrink-0 rounded-full object-contain"
          />
        ) : (
          <span className="text-base">{selected.icon}</span>
        )}
        <span className="flex-1 truncate">{selected.name}</span>
        <svg
          className={`h-4 w-4 shrink-0 text-gray-300 transition-transform ${open ? 'rotate-180' : ''}`}
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
          className="absolute z-50 mt-1 max-h-48 w-full overflow-auto border-2 border-white/20 bg-[#161616] shadow-lg"
        >
          {chains.map((chain) => {
            const isSelected = chain.id === value
            return (
              <li
                key={chain.id}
                role="option"
                aria-selected={isSelected}
                data-chainid={chain.id}
                onClick={() => {
                  onChange(chain.id)
                  setOpen(false)
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    onChange(chain.id)
                    setOpen(false)
                  }
                }}
                tabIndex={0}
                className={`flex cursor-pointer items-center gap-2 px-3 py-2 text-sm text-white hover:bg-white/10 ${
                  isSelected ? 'bg-cyan-900/30' : ''
                }`}
              >
                {isIconImagePath(chain.icon) ? (
                  <img
                    src={chain.icon}
                    alt=""
                    className="h-5 w-5 shrink-0 rounded-full object-contain"
                  />
                ) : (
                  <span className="text-base">{chain.icon}</span>
                )}
                <span className="truncate">{chain.name}</span>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
