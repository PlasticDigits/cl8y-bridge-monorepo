/**
 * Chain Discovery Service
 *
 * Resolves bytes4 chain IDs to bridge chain configurations.
 * Uses a static map for well-known chains, with optional RPC discovery for unknown chains.
 */

import type { BridgeChainConfig } from '../types/chain'
import { getAllBridgeChains, getBridgeChainByBytes4 } from '../utils/bridgeChains'
import { getEvmClient } from './evmClient'
import type { Address } from 'viem'

// Static map of well-known V2 bytes4 chain IDs.
// NOTE: These are V2 ChainRegistry IDs, NOT native chain IDs.
// V2 IDs are predetermined per deployment — they do not follow an algorithm.
// The tier-aware getBridgeChainByBytes4() should be preferred for lookups.
const WELL_KNOWN_CHAIN_IDS: Record<string, string> = {
  '0x00000001': 'terra',    // Also anvil in local mode
  '0x00000002': 'localterra', // Local-only Terra
  '0x00000003': 'anvil1',   // Local-only second EVM chain
  '0x00000038': 'bsc',      // V2 ID for BSC
  '0x000000cc': 'opbnb',    // V2 ID for opBNB
  '0x00000061': 'bsc-testnet',   // 97
  '0x000015eb': 'opbnb-testnet', // 5611
}

// Bridge view ABI
const BRIDGE_ABI = [
  {
    name: 'getThisChainId',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'bytes4' }],
  },
  {
    name: 'chainRegistry',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'address' }],
  },
] as const

// ChainRegistry view ABI
const CHAIN_REGISTRY_ABI = [
  {
    name: 'getRegisteredChains',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: 'chainIds', type: 'bytes4[]' }],
  },
] as const

/**
 * Discover bytes4 chain IDs from bridge contracts via RPC.
 * Queries each EVM bridge's getThisChainId() function.
 *
 * @param chains - Bridge chain configs to discover
 * @returns Map of bytes4 hex string -> BridgeChainConfig
 */
export async function discoverChainIds(
  chains: BridgeChainConfig[]
): Promise<Map<string, BridgeChainConfig>> {
  const result = new Map<string, BridgeChainConfig>()

  // First, populate static map for well-known chains
  for (const chain of chains) {
    if (chain.bytes4ChainId) {
      const normalized = chain.bytes4ChainId.toLowerCase()
      result.set(normalized, chain)
    } else if (chain.type === 'evm') {
      // Check if this chain ID matches a well-known mapping
      const chainIdHex = typeof chain.chainId === 'number'
        ? `0x${chain.chainId.toString(16).padStart(8, '0')}`
        : null
      if (chainIdHex && WELL_KNOWN_CHAIN_IDS[chainIdHex]) {
        result.set(chainIdHex.toLowerCase(), chain)
      }
    }
  }

  // Then, discover unknown EVM chains via RPC
  const evmChains = chains.filter((c) => c.type === 'evm' && !result.has(c.bytes4ChainId?.toLowerCase() || ''))
  
  if (evmChains.length === 0) {
    return result
  }

  const discoveryPromises = evmChains.map(async (chain) => {
    if (!chain.bridgeAddress || !chain.rpcUrl) {
      return null
    }

    try {
      const client = getEvmClient(chain as BridgeChainConfig & { chainId: number })

      const bytes4 = await client.readContract({
        address: chain.bridgeAddress as Address,
        abi: BRIDGE_ABI,
        functionName: 'getThisChainId',
      })

      const bytes4Hex = `0x${bytes4.slice(2).padStart(8, '0')}`.toLowerCase()
      return { bytes4Hex, chain }
    } catch (err) {
      console.warn(`Failed to discover chain ID for ${chain.name}:`, err)
      return null
    }
  })

  const discoveries = await Promise.allSettled(discoveryPromises)
  
  for (const discovery of discoveries) {
    if (discovery.status === 'fulfilled' && discovery.value) {
      const { bytes4Hex, chain } = discovery.value
      result.set(bytes4Hex, chain)
    }
  }

  return result
}

/**
 * Resolve a bytes4 chain ID to a bridge chain config.
 * First checks static map, then falls back to discovery if needed.
 *
 * @param bytes4Hex - bytes4 chain ID as hex string (e.g., "0x00000001")
 * @param chains - Optional chain list (defaults to getAllBridgeChains())
 * @returns BridgeChainConfig or undefined
 */
export async function resolveChainByBytes4(
  bytes4Hex: string,
  chains?: BridgeChainConfig[]
): Promise<BridgeChainConfig | undefined> {
  const normalized = bytes4Hex.toLowerCase()
  
  // Try static lookup first
  const staticResult = getBridgeChainByBytes4(normalized)
  if (staticResult) {
    return staticResult
  }

  // If not found and chains provided, try discovery
  if (chains) {
    const discoveryMap = await discoverChainIds(chains)
    return discoveryMap.get(normalized)
  }

  return undefined
}

/**
 * Build a complete bytes4 -> chain config map for all configured chains.
 * Uses static map + discovery for unknown chains.
 */
export async function buildChainIdMap(): Promise<Map<string, BridgeChainConfig>> {
  const chains = getAllBridgeChains()
  return discoverChainIds(chains)
}

/**
 * Query the on-chain ChainRegistry via a seed EVM bridge to discover
 * which V2 chain IDs are actually registered.
 *
 * Flow:
 *   1. Pick a seed EVM chain (first one with bridgeAddress + rpcUrl)
 *   2. Call bridge.chainRegistry() to get the ChainRegistry address
 *   3. Call chainRegistry.getRegisteredChains() to get bytes4[]
 *
 * @returns Set of lowercase hex bytes4 chain IDs registered on-chain, or null on failure
 */
export async function discoverRegisteredChains(
  chains: BridgeChainConfig[]
): Promise<Set<string> | null> {
  const seed = chains.find((c) => c.type === 'evm' && c.bridgeAddress && c.rpcUrl)
  if (!seed) return null

  try {
    const client = getEvmClient(seed as BridgeChainConfig & { chainId: number })

    const registryAddr = await client.readContract({
      address: seed.bridgeAddress as Address,
      abi: BRIDGE_ABI,
      functionName: 'chainRegistry',
    })

    const registered = await client.readContract({
      address: registryAddr,
      abi: CHAIN_REGISTRY_ABI,
      functionName: 'getRegisteredChains',
    })

    const set = new Set<string>()
    for (const id of registered) {
      set.add(`0x${id.slice(2).padStart(8, '0')}`.toLowerCase())
    }
    return set
  } catch (err) {
    console.warn('[chainDiscovery] Failed to discover registered chains:', err)
    return null
  }
}
