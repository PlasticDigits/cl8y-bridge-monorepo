export interface TokenConfig {
  address: string
  lockUnlockAddress?: string
  symbol: string
  decimals: number
}

export interface TokenRegistryEntry {
  symbol: string
  decimals: number
  contractAddress: string
  bridgeMode: 'MintBurn' | 'LockUnlock'
  chains: string[]
}
