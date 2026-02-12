import type { ChainInfo } from '../../lib/chains'
import { ChainSelect } from './ChainSelect'

export interface DestChainSelectorProps {
  chains: ChainInfo[]
  value: string
  onChange: (chainId: string) => void
}

export function DestChainSelector({ chains, value, onChange }: DestChainSelectorProps) {
  return (
    <ChainSelect
      chains={chains}
      value={value}
      onChange={onChange}
      label="To"
      id="dest-chain-select"
    />
  )
}
