/**
 * Bridge Chain Configuration
 *
 * Per-network configuration for all supported bridge chains.
 * Used by multi-chain hash verification to query deposits/withdraws across chains.
 */

import type { BridgeChainConfig } from '../types/chain'
import { DEFAULT_NETWORK } from './constants'

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
      bytes4ChainId: '0x00007a69', // 31337 = 0x7a69
    },
    localterra: {
      chainId: 'localterra',
      type: 'cosmos',
      name: 'LocalTerra',
      rpcUrl: 'http://localhost:26657',
      lcdUrl: 'http://localhost:1317',
      lcdFallbacks: ['http://localhost:1317'],
      bridgeAddress: import.meta.env.VITE_TERRA_BRIDGE_ADDRESS || '',
    },
  },
  testnet: {
    ethereum: {
      chainId: 1,
      type: 'evm',
      name: 'Ethereum',
      rpcUrl: import.meta.env.VITE_ETH_RPC_URL || 'https://eth.llamarpc.com',
      bridgeAddress: import.meta.env.VITE_ETH_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000001',
    },
    bsc: {
      chainId: 97,
      type: 'evm',
      name: 'BSC Testnet',
      rpcUrl: 'https://data-seed-prebsc-1-s1.binance.org:8545',
      bridgeAddress: import.meta.env.VITE_BSC_TESTNET_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000061', // 97 = 0x61
    },
    opbnb: {
      chainId: 5611,
      type: 'evm',
      name: 'opBNB Testnet',
      rpcUrl: import.meta.env.VITE_OPBNB_TESTNET_RPC_URL || 'https://opbnb-testnet-rpc.bnbchain.org',
      bridgeAddress: import.meta.env.VITE_OPBNB_TESTNET_BRIDGE_ADDRESS || '',
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
    },
  },
  mainnet: {
    ethereum: {
      chainId: 1,
      type: 'evm',
      name: 'Ethereum',
      rpcUrl: import.meta.env.VITE_ETH_RPC_URL || 'https://eth.llamarpc.com',
      bridgeAddress: import.meta.env.VITE_ETH_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000001',
    },
    bsc: {
      chainId: 56,
      type: 'evm',
      name: 'BNB Chain',
      rpcUrl: 'https://bsc-dataseed1.binance.org',
      bridgeAddress: import.meta.env.VITE_BSC_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x00000038', // 56 = 0x38
    },
    opbnb: {
      chainId: 204,
      type: 'evm',
      name: 'opBNB',
      rpcUrl: 'https://opbnb-mainnet-rpc.bnbchain.org',
      bridgeAddress: import.meta.env.VITE_OPBNB_BRIDGE_ADDRESS || '',
      bytes4ChainId: '0x000000cc', // 204 = 0xcc
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
    },
  },
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
 * Get bridge chain config by chain name (e.g., "ethereum", "bsc", "terra").
 */
export function getBridgeChainByName(name: string): BridgeChainConfig | undefined {
  const tier = DEFAULT_NETWORK as NetworkTier
  return BRIDGE_CHAINS[tier][name]
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
