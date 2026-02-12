import type { ChainInfo } from '../../lib/chains'

export interface DestChainSelectorProps {
  chains: ChainInfo[]
  value: string
  onChange: (chainId: string) => void
}

export function DestChainSelector({ chains, value, onChange }: DestChainSelectorProps) {
  return (
    <div>
      <label className="block text-sm font-medium text-gray-400 mb-2">To</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full bg-gray-900 border border-gray-700 rounded-lg px-4 py-3 text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
      >
        {chains.map((chain) => (
          <option key={chain.id} value={chain.id}>
            {chain.icon} {chain.name}
          </option>
        ))}
      </select>
    </div>
  )
}
