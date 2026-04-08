export interface ChainInfo {
  id: string
  name: string
  chainId: number | string
  type: 'evm' | 'cosmos' | 'solana'
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
  programId?: string
}

/**
 * Bridge chain configuration for multi-chain hash verification.
 * Extends ChainInfo with required bridge contract address and optional bytes4 chain ID.
 */
export interface BridgeChainConfig {
  chainId: number | string
  type: 'evm' | 'cosmos' | 'solana'
  name: string
  rpcUrl: string
  rpcFallbacks?: string[]
  lcdUrl?: string
  lcdFallbacks?: string[]
  /**
   * EVM/Cosmos: bridge contract address.
   * Solana: BridgeConfig PDA (seed `bridge`); use `programId` for the deployed program address.
   */
  bridgeAddress: string
  bytes4ChainId?: string
  /** Solana only: deployed cl8y_bridge program id (base58). */
  programId?: string
  explorerTxUrl?: string
  faucetAddress?: string
}
