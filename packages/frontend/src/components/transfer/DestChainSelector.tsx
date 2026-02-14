import type { ChainInfo } from '../../lib/chains'
import { ChainSelect } from './ChainSelect'

export interface DestChainSelectorProps {
  chains: ChainInfo[]
  value: string
  onChange: (chainId: string) => void
  disabled?: boolean
}

export function DestChainSelector({ chains, value, onChange, disabled }: DestChainSelectorProps) {
  return (
    <div data-testid="dest-chain">
      <ChainSelect
        chains={chains}
        value={value}
        onChange={onChange}
        label="To"
        id="dest-chain-select"
        disabled={disabled}
      />
    </div>
  )
}
