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
}

export const supportedChains: ChainInfo[] = [
  {
    id: 'ethereum',
    name: 'Ethereum',
    chainId: 1,
    type: 'evm',
    icon: '⟠',
    rpcUrl: 'https://eth.llamarpc.com',
    explorerUrl: 'https://etherscan.io',
    nativeCurrency: {
      name: 'Ether',
      symbol: 'ETH',
      decimals: 18,
    },
  },
  {
    id: 'bsc',
    name: 'BNB Chain',
    chainId: 56,
    type: 'evm',
    icon: '⬡',
    rpcUrl: 'https://bsc-dataseed.binance.org',
    explorerUrl: 'https://bscscan.com',
    nativeCurrency: {
      name: 'BNB',
      symbol: 'BNB',
      decimals: 18,
    },
  },
  {
    id: 'polygon',
    name: 'Polygon',
    chainId: 137,
    type: 'evm',
    icon: '⬢',
    rpcUrl: 'https://polygon-rpc.com',
    explorerUrl: 'https://polygonscan.com',
    nativeCurrency: {
      name: 'MATIC',
      symbol: 'MATIC',
      decimals: 18,
    },
  },
  {
    id: 'terra',
    name: 'Terra Classic',
    chainId: 'columbus-5',
    type: 'cosmos',
    icon: '🌙',
    rpcUrl: 'https://terra-classic-rpc.publicnode.com',
    explorerUrl: 'https://finder.terra.money/classic',
    nativeCurrency: {
      name: 'Luna Classic',
      symbol: 'LUNC',
      decimals: 6,
    },
  },
  {
    id: 'anvil',
    name: 'Anvil (Local)',
    chainId: 31337,
    type: 'evm',
    icon: '🔨',
    rpcUrl: 'http://localhost:8545',
    explorerUrl: '',
    nativeCurrency: {
      name: 'Ether',
      symbol: 'ETH',
      decimals: 18,
    },
  },
  {
    id: 'localterra',
    name: 'LocalTerra',
    chainId: 'localterra',
    type: 'cosmos',
    icon: '🌙',
    rpcUrl: 'http://localhost:26657',
    explorerUrl: '',
    nativeCurrency: {
      name: 'Luna',
      symbol: 'LUNA',
      decimals: 6,
    },
  },
]

export function getChainById(id: string): ChainInfo | undefined {
  return supportedChains.find((chain) => chain.id === id)
}

export function getChainByChainId(chainId: number | string): ChainInfo | undefined {
  return supportedChains.find((chain) => chain.chainId === chainId)
}

export function getEvmChains(): ChainInfo[] {
  return supportedChains.filter((chain) => chain.type === 'evm')
}

export function getCosmosChains(): ChainInfo[] {
  return supportedChains.filter((chain) => chain.type === 'cosmos')
}

export function getExplorerTxUrl(chainId: string, txHash: string): string {
  const chain = getChainById(chainId)
  if (!chain || !chain.explorerUrl) return ''
  
  if (chain.type === 'evm') {
    return `${chain.explorerUrl}/tx/${txHash}`
  } else {
    return `${chain.explorerUrl}/tx/${txHash}`
  }
}