export interface ChainInfo {
  id: string
  name: string
  chainId: number | string
  type: 'evm' | 'cosmos'
  icon: string
  rpcUrl: string
  explorerUrl: string
  nativeCurrency: {
    name: string
    symbol: string
    decimals: number
  }
  bridgeAddress?: string
  lcdUrl?: string
  lcdFallbacks?: string[]
}

/**
 * Bridge chain configuration for multi-chain hash verification.
 * Extends ChainInfo with required bridge contract address and optional bytes4 chain ID.
 */
export interface BridgeChainConfig {
  chainId: number | string
  type: 'evm' | 'cosmos'
  name: string
  rpcUrl: string
  rpcFallbacks?: string[]
  lcdUrl?: string
  lcdFallbacks?: string[]
  bridgeAddress: string
  bytes4ChainId?: string
}
