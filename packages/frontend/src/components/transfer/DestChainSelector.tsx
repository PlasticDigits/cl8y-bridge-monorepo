import type { ChainInfo } from '../../lib/chains'

export interface DestChainSelectorProps {
  chains: ChainInfo[]
  value: string
  onChange: (chainId: string) => void
}

export function DestChainSelector({ chains, value, onChange }: DestChainSelectorProps) {
  return (
    <div>
      <label className="mb-1 block text-xs font-semibold uppercase tracking-wide text-gray-300">To</label>
      <div className="relative">
        <select
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="w-full cursor-pointer appearance-none border-2 border-white/20 bg-[#161616] pl-3 pr-10 py-2 text-sm text-white focus:border-cyan-300 focus:outline-none"
        >
          {chains.map((chain) => (
            <option key={chain.id} value={chain.id}>
              {chain.icon} {chain.name}
            </option>
          ))}
        </select>
        <div className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-gray-300">
          <svg
            className="h-4 w-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2.5}
            strokeLinecap="square"
            strokeLinejoin="miter"
          >
            <path d="M6 9l6 6 6-6" />
          </svg>
        </div>
      </div>
    </div>
  )
}
