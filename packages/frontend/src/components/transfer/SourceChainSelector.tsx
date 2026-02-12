import type { ChainInfo } from '../../lib/chains'

export interface SourceChainSelectorProps {
  chains: ChainInfo[]
  value: string
  onChange: (chainId: string) => void
  balance?: string
  balanceLabel?: string
}

export function SourceChainSelector({
  chains,
  value,
  onChange,
  balance,
  balanceLabel,
}: SourceChainSelectorProps) {
  return (
    <div>
      <label className="block text-sm font-medium text-gray-400 mb-2">From</label>
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
      {balance !== undefined && (
        <p className="text-xs text-gray-500 mt-1">
          Balance: {balance} {balanceLabel ?? ''}
        </p>
      )}
    </div>
  )
}
