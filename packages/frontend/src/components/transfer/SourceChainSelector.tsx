import type { ChainInfo } from '../../lib/chains'
import { ChainSelect } from './ChainSelect'

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
      <ChainSelect
        chains={chains}
        value={value}
        onChange={onChange}
        label="From"
        id="source-chain-select"
      />
      {balance !== undefined && (
        <p className="mt-1 text-[11px] uppercase tracking-wide text-gray-400">
          Balance: {balance} {balanceLabel ?? ''}
        </p>
      )}
    </div>
  )
}
