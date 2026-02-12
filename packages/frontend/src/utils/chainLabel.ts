/**
 * Shared utility to map chain IDs (numeric, string, or bytes4) to human-readable labels.
 * Used by SourceHashCard, DestHashCard, and other verify components.
 */

import { getChainByChainId } from '../lib/chains'
import { getBridgeChainByBytes4, getBridgeChainByChainId } from './bridgeChains'

/**
 * Map an EVM chain ID (number) to a label, using the chain registry.
 * Falls back to "Chain {id}" for unknown chains.
 */
export function evmChainIdToLabel(chainId: number): string {
  const chain = getChainByChainId(chainId)
  if (chain) return chain.name
  return `Chain ${chainId}`
}

/**
 * Map a bytes4 chain ID (hex string, e.g., "0x00000001") to a label.
 * First checks bridge chain config, then falls back to numeric conversion.
 */
export function bytes4ChainIdToLabel(bytes4Hex: string): string {
  const bridgeChain = getBridgeChainByBytes4(bytes4Hex)
  if (bridgeChain) return bridgeChain.name

  // Try to parse as numeric chain ID and check if it's a known chain
  try {
    const numericId = parseInt(bytes4Hex.slice(2).slice(0, 8), 16)
    if (!isNaN(numericId)) {
      const label = evmChainIdToLabel(numericId)
      // Only use numeric label if it's not a fallback (i.e., found in registry)
      if (!label.startsWith('Chain ')) {
        return label
      }
    }
  } catch {
    // Ignore parse errors
  }

  return `Chain ${bytes4Hex}`
}

/**
 * Map a chain ID (number, string, or bytes4 hex) to a label.
 * Handles all chain ID formats.
 */
export function chainIdToLabel(chainId: number | string): string {
  if (typeof chainId === 'number') {
    return evmChainIdToLabel(chainId)
  }

  // Check if it's a bytes4 hex string
  if (chainId.startsWith('0x') && chainId.length === 10) {
    return bytes4ChainIdToLabel(chainId)
  }

  // Try bridge chain lookup (for Cosmos chain IDs like "columbus-5")
  const bridgeChain = getBridgeChainByChainId(chainId)
  if (bridgeChain) return bridgeChain.name

  // Fallback to chain registry
  const chain = getChainByChainId(chainId)
  if (chain) return chain.name

  return `Chain ${chainId}`
}
