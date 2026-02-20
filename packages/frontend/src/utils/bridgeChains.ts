/**
 * Bridge Chain Configuration
 *
 * Per-network configuration for all supported bridge chains.
 * Used by multi-chain hash verification to query deposits/withdraws across chains.
 * Display info (name, icon) comes from chainlist.json when available.
 */

import type { BridgeChainConfig, ChainInfo } from '../types/chain'
import { DEFAULT_NETWORK } from './constants'
import { getChainlist, getChainlistEntry } from './chainlist'

export type NetworkTier = 'local' | 'testnet' | 'mainnet'

/**
 * Bridge chain configurations per network tier.
 * Each tier contains configs for all supported chains (EVM + Terra).
 */
export const BRIDGE_CHAINS: Record<NetworkTier, Record<string, BridgeChainConfig>> = {
  local: {
    anvil: {
      chainId: 31337,
      type: 'evm',
      name: 'Anvil',
      rpcUrl: 'http://localhost:8545',
      bridgeAddress: import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000001', // V2 chain ID 1 (set via THIS_V2_CHAIN_ID in DeployLocal.s.sol)
    },
    anvil1: {
      chainId: 31338,
      type: 'evm',
      name: 'Anvil1',
      rpcUrl: 'http://localhost:8546',
      bridgeAddress: import.meta.env.VITE_EVM1_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000003', // V2 chain ID 3 (set via THIS_V2_CHAIN_ID in DeployLocal.s.sol)
    },
    localterra: {
      chainId: 'localterra',
      type: 'cosmos',
      name: 'LocalTerra',
      rpcUrl: 'http://localhost:26657',
      lcdUrl: 'http://localhost:1317',
      lcdFallbacks: ['http://localhost:1317'],
      bridgeAddress: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000002', // V2 chain ID 2 (local Terra)
    },
  },
  testnet: {
    ethereum: {
      chainId: 1,
      type: 'evm',
      name: 'Ethereum',
      rpcUrl: import.meta.env.VITE_ETH_RPC_URL || 'https://eth.llamarpc.com',
      rpcFallbacks: ['https://rpc.ankr.com/eth', 'https://ethereum-rpc.publicnode.com'],
      bridgeAddress: import.meta.env.VITE_ETH_BRIDGE_ADDRESS || import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000001',
    },
    bsc: {
      chainId: 97,
      type: 'evm',
      name: 'BSC Testnet',
      rpcUrl: 'https://data-seed-prebsc-1-s1.binance.org:8545',
      rpcFallbacks: ['https://data-seed-prebsc-2-s1.binance.org:8545'],
      bridgeAddress: import.meta.env.VITE_BSC_TESTNET_BRIDGE_ADDRESS || import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000061', // 97 = 0x61
    },
    opbnb: {
      chainId: 5611,
      type: 'evm',
      name: 'opBNB Testnet',
      rpcUrl: import.meta.env.VITE_OPBNB_TESTNET_RPC_URL || 'https://opbnb-testnet-rpc.bnbchain.org',
      bridgeAddress: import.meta.env.VITE_OPBNB_TESTNET_BRIDGE_ADDRESS || import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x000015eb', // 5611 = 0x15eb
    },
    terra: {
      chainId: 'rebel-2',
      type: 'cosmos',
      name: 'Terra Classic Testnet',
      rpcUrl: 'https://rpc.luncblaze.com',
      lcdUrl: 'https://lcd.luncblaze.com',
      lcdFallbacks: [
        'https://lcd.luncblaze.com',
        'https://lcd.terra-classic.hexxagon.dev',
      ],
      bridgeAddress: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000002',
    },
  },
  mainnet: {
    ethereum: {
      chainId: 1,
      type: 'evm',
      name: 'Ethereum',
      rpcUrl: import.meta.env.VITE_ETH_RPC_URL || 'https://eth.llamarpc.com',
      rpcFallbacks: ['https://rpc.ankr.com/eth', 'https://ethereum-rpc.publicnode.com'],
      bridgeAddress: import.meta.env.VITE_ETH_BRIDGE_ADDRESS || import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
      // bytes4ChainId TBD — not yet deployed; will be assigned on deployment
    },
    bsc: {
      chainId: 56,
      type: 'evm',
      name: 'BNB Chain',
      rpcUrl: 'https://bsc.publicnode.com',
      rpcFallbacks: [
        'https://bsc-dataseed1.binance.org',
        'https://binance.llamarpc.com',
        'https://bsc-dataseed1.defibit.io',
        'https://bsc-dataseed1.ninicoin.io',
        'https://bnb.api.onfinality.io/public',
        'https://bnb.rpc.subquery.network/public',
        'https://bsc.meowrpc.com',
        'https://bsc.drpc.org',
        'https://public-bsc-mainnet.fastnode.io',
      ],
      bridgeAddress: import.meta.env.VITE_BSC_BRIDGE_ADDRESS || import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000038',
    },
    opbnb: {
      chainId: 204,
      type: 'evm',
      name: 'opBNB',
      rpcUrl: 'https://opbnb-rpc.publicnode.com',
      rpcFallbacks: [
        'https://opbnb.api.pocket.network',
        'https://opbnb-mainnet-rpc.bnbchain.org',
      ],
      bridgeAddress: import.meta.env.VITE_OPBNB_BRIDGE_ADDRESS || import.meta.env.VITE_EVM_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x000000cc',
    },
    terra: {
      chainId: 'columbus-5',
      type: 'cosmos',
      name: 'Terra Classic',
      rpcUrl: 'https://terra-classic-rpc.publicnode.com',
      lcdUrl: 'https://terra-classic-lcd.publicnode.com',
      lcdFallbacks: [
        'https://terra-classic-lcd.publicnode.com',
        'https://api-lunc-lcd.binodes.com',
        'https://lcd.terra-classic.hexxagon.io',
      ],
      bridgeAddress: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000001',
    },
  },
}

/** Display info for chains (icon, explorer, currency) keyed by chain id */
const CHAIN_DISPLAY: Record<string, { icon: string; explorerUrl: string; nativeCurrency: { name: string; symbol: string; decimals: number } }> = {
  ethereum: { icon: '⟠', explorerUrl: 'https://etherscan.io', nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 } },
  bsc: { icon: '⬡', explorerUrl: 'https://bscscan.com', nativeCurrency: { name: 'BNB', symbol: 'BNB', decimals: 18 } },
  opbnb: { icon: '⬡', explorerUrl: 'https://opbnb.bscscan.com', nativeCurrency: { name: 'BNB', symbol: 'BNB', decimals: 18 } },
  terra: { icon: '🌙', explorerUrl: 'https://finder.terraclassic.community/mainnet', nativeCurrency: { name: 'Luna Classic', symbol: 'LUNC', decimals: 6 } },
  anvil: { icon: '🔨', explorerUrl: '', nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 } },
  anvil1: { icon: '🔨', explorerUrl: '', nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 } },
  localterra: { icon: '🌙', explorerUrl: '', nativeCurrency: { name: 'Luna', symbol: 'LUNA', decimals: 6 } },
}

/**
 * Get chains available for transfers in the current network.
 * Returns ChainInfo[] suitable for source/dest selectors, including Terra <> EVM and EVM <> EVM routes.
 * Uses chainlist.json for name and icon when available, otherwise falls back to CHAIN_DISPLAY.
 */
export function getChainsForTransfer(): ChainInfo[] {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chainlist = getChainlist()
  return Object.entries(BRIDGE_CHAINS[tier]).filter(([_, config]) => {
    if (!config.bridgeAddress) return false
    // EVM chains need a bytes4ChainId to participate in V2 protocol
    if (config.type === 'evm' && !config.bytes4ChainId) return false
    return true
  }).map(([id, config]) => {
    const chainlistEntry = getChainlistEntry(chainlist, id, config.chainId)
    const display = CHAIN_DISPLAY[id] ?? {
      icon: '○',
      explorerUrl: '',
      nativeCurrency: { name: 'Unknown', symbol: '???', decimals: 18 },
    }
    return {
      id,
      name: chainlistEntry?.name ?? config.name,
      chainId: config.chainId,
      type: config.type,
      icon: chainlistEntry?.icon ?? display.icon,
      rpcUrl: config.rpcUrl,
      explorerUrl: chainlistEntry?.explorerUrl ?? display.explorerUrl,
      nativeCurrency: display.nativeCurrency,
    }
  })
}

/**
 * Get all bridge chain configs for the current network tier.
 */
export function getAllBridgeChains(): BridgeChainConfig[] {
  const tier = DEFAULT_NETWORK as NetworkTier
  return Object.values(BRIDGE_CHAINS[tier])
}

/**
 * Get bridge chain config by chain ID (numeric for EVM, string for Cosmos).
 */
export function getBridgeChainByChainId(chainId: number | string): BridgeChainConfig | undefined {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier]
  return Object.values(chains).find((c) => c.chainId === chainId)
}

/**
 * Get bridge chain config by bytes4 chain ID (hex string, e.g., "0x00000001").
 */
export function getBridgeChainByBytes4(bytes4Hex: string): BridgeChainConfig | undefined {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier]
  const normalized = bytes4Hex.toLowerCase()
  return Object.values(chains).find((c) => c.bytes4ChainId?.toLowerCase() === normalized)
}

/**
 * Get bridge chain key and config by bytes4 chain ID. Returns [chainKey, config] or undefined.
 */
export function getBridgeChainEntryByBytes4(
  bytes4Hex: string
): [string, BridgeChainConfig] | undefined {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier]
  const normalized = bytes4Hex.toLowerCase()
  const entry = Object.entries(chains).find(
    ([_, c]) => (c as BridgeChainConfig & { bytes4ChainId?: string }).bytes4ChainId?.toLowerCase() === normalized
  )
  return entry ? [entry[0], entry[1]] : undefined
}

/**
 * Get explorer base URL for a bridge chain key (e.g. "terra", "bsc").
 * Uses chainlist when loaded, otherwise falls back to CHAIN_DISPLAY.
 */
export function getExplorerUrlForChain(chainKey: string): string {
  try {
    const chainlist = getChainlist()
    const config = BRIDGE_CHAINS[DEFAULT_NETWORK as NetworkTier]?.[chainKey]
    const entry = config ? getChainlistEntry(chainlist, chainKey, config.chainId) : undefined
    if (entry?.explorerUrl) return entry.explorerUrl
  } catch {
    // Chainlist not loaded
  }
  return CHAIN_DISPLAY[chainKey]?.explorerUrl ?? ''
}

/**
 * Get chain display info (name, icon) for a bridge chain key.
 */
export function getChainDisplayInfo(chainKey: string): { name: string; icon: string } {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chainlist = getChainlist()
  const config = BRIDGE_CHAINS[tier][chainKey]
  const display = CHAIN_DISPLAY[chainKey] ?? { icon: '○', explorerUrl: '', nativeCurrency: { name: '', symbol: '', decimals: 18 } }
  const chainlistEntry = config ? getChainlistEntry(chainlist, chainKey, config.chainId) : undefined
  return {
    name: chainlistEntry?.name ?? config?.name ?? chainKey,
    icon: chainlistEntry?.icon ?? display.icon,
  }
}

/**
 * Get bridge chain config by chain name (e.g., "ethereum", "bsc", "terra").
 */
export function getBridgeChainByName(name: string): BridgeChainConfig | undefined {
  const tier = DEFAULT_NETWORK as NetworkTier
  return BRIDGE_CHAINS[tier][name]
}

/**
 * Get chain key (e.g. "anvil1", "localterra") from a bridge config by matching bridgeAddress.
 */
export function getChainKeyByConfig(config: BridgeChainConfig): string | undefined {
  const tier = DEFAULT_NETWORK as NetworkTier
  const chains = BRIDGE_CHAINS[tier]
  const entry = Object.entries(chains).find(
    ([_, c]) => c.bridgeAddress === config.bridgeAddress && c.chainId === config.chainId
  )
  return entry?.[0]
}

/**
 * Get EVM bridge chains only.
 */
export function getEvmBridgeChains(): BridgeChainConfig[] {
  return getAllBridgeChains().filter((c) => c.type === 'evm')
}

/**
 * Get Cosmos bridge chains only.
 */
export function getCosmosBridgeChains(): BridgeChainConfig[] {
  return getAllBridgeChains().filter((c) => c.type === 'cosmos')
}
