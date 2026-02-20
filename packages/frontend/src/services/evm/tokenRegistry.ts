/**
 * EVM Token Registry Service
 *
 * Queries the on-chain TokenRegistry (via Bridge.tokenRegistry()) to resolve
 * destination token mappings for cross-chain transfers.
 *
 * Used to determine which token address exists on the destination chain for
 * a given source token + destination chain ID pair.
 */

import type { PublicClient, Address, Hex } from 'viem'

// ABI for Bridge.tokenRegistry() -- returns the address of the TokenRegistry contract
const BRIDGE_TOKEN_REGISTRY_ABI = [
  {
    name: 'tokenRegistry',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'address' }],
  },
] as const

// ABI for TokenRegistry view functions
export const TOKEN_REGISTRY_ABI = [
  {
    name: 'getDestToken',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'token', type: 'address' },
      { name: 'destChain', type: 'bytes4' },
    ],
    outputs: [{ name: 'destToken', type: 'bytes32' }],
  },
  {
    name: 'getDestTokenMapping',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'token', type: 'address' },
      { name: 'destChain', type: 'bytes4' },
    ],
    outputs: [
      {
        name: 'mapping_',
        type: 'tuple',
        components: [
          { name: 'destToken', type: 'bytes32' },
          { name: 'destDecimals', type: 'uint8' },
        ],
      },
    ],
  },
  {
    name: 'isTokenRegistered',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'token', type: 'address' }],
    outputs: [{ name: 'registered', type: 'bool' }],
  },
  {
    name: 'getSrcTokenDecimals',
    type: 'function',
    stateMutability: 'view',
    inputs: [
      { name: 'srcChain', type: 'bytes4' },
      { name: 'token', type: 'address' },
    ],
    outputs: [{ name: 'srcDecimals', type: 'uint8' }],
  },
] as const

export interface TokenDestMapping {
  destToken: Hex     // bytes32 representation of the token on the dest chain
  destDecimals: number
}

/**
 * Get the TokenRegistry address from the Bridge contract.
 */
export async function getTokenRegistryAddress(
  publicClient: PublicClient,
  bridgeAddress: Address
): Promise<Address> {
  const registryAddr = await publicClient.readContract({
    address: bridgeAddress,
    abi: BRIDGE_TOKEN_REGISTRY_ABI,
    functionName: 'tokenRegistry',
  })
  return registryAddr as Address
}

/**
 * Query the destination token bytes32 for a given source token and destination chain.
 *
 * @param publicClient - Viem PublicClient for the source chain
 * @param bridgeAddress - Bridge contract address on the source chain
 * @param tokenAddress - Source token ERC20 address
 * @param destChainBytes4 - Destination chain ID as bytes4 hex (e.g. "0x00007a6a")
 * @returns The destination token as bytes32, or null if not mapped
 */
export async function getDestToken(
  publicClient: PublicClient,
  bridgeAddress: Address,
  tokenAddress: Address,
  destChainBytes4: Hex
): Promise<Hex | null> {
  try {
    const registryAddr = await getTokenRegistryAddress(publicClient, bridgeAddress)

    const destToken = await publicClient.readContract({
      address: registryAddr,
      abi: TOKEN_REGISTRY_ABI,
      functionName: 'getDestToken',
      args: [tokenAddress, destChainBytes4 as `0x${string}`],
    })

    const result = destToken as Hex
    // bytes32(0) means no mapping
    if (result === '0x0000000000000000000000000000000000000000000000000000000000000000') {
      return null
    }
    return result
  } catch (err) {
    console.warn('[TokenRegistry] Failed to query getDestToken:', err)
    return null
  }
}

/**
 * Query the full destination token mapping (destToken + destDecimals).
 */
export async function getDestTokenMapping(
  publicClient: PublicClient,
  bridgeAddress: Address,
  tokenAddress: Address,
  destChainBytes4: Hex
): Promise<TokenDestMapping | null> {
  try {
    const registryAddr = await getTokenRegistryAddress(publicClient, bridgeAddress)

    const mapping = await publicClient.readContract({
      address: registryAddr,
      abi: TOKEN_REGISTRY_ABI,
      functionName: 'getDestTokenMapping',
      args: [tokenAddress, destChainBytes4 as `0x${string}`],
    }) as { destToken: Hex; destDecimals: number }

    if (mapping.destToken === '0x0000000000000000000000000000000000000000000000000000000000000000') {
      return null
    }

    return {
      destToken: mapping.destToken,
      destDecimals: mapping.destDecimals,
    }
  } catch (err) {
    console.warn('[TokenRegistry] Failed to query getDestTokenMapping:', err)
    return null
  }
}

/**
 * Check if a token is registered on the TokenRegistry.
 */
export async function isTokenRegistered(
  publicClient: PublicClient,
  bridgeAddress: Address,
  tokenAddress: Address
): Promise<boolean> {
  try {
    const registryAddr = await getTokenRegistryAddress(publicClient, bridgeAddress)
    const registered = await publicClient.readContract({
      address: registryAddr,
      abi: TOKEN_REGISTRY_ABI,
      functionName: 'isTokenRegistered',
      args: [tokenAddress],
    })
    return registered as boolean
  } catch {
    return false
  }
}

/**
 * Query source token decimals for incoming transfers from a specific chain.
 * Returns null if the incoming mapping is not configured (reverts on-chain).
 */
export async function getSrcTokenDecimals(
  publicClient: PublicClient,
  bridgeAddress: Address,
  srcChainBytes4: Hex,
  tokenAddress: Address
): Promise<number | null> {
  try {
    const registryAddr = await getTokenRegistryAddress(publicClient, bridgeAddress)
    const decimals = await publicClient.readContract({
      address: registryAddr,
      abi: TOKEN_REGISTRY_ABI,
      functionName: 'getSrcTokenDecimals',
      args: [srcChainBytes4 as `0x${string}`, tokenAddress],
    })
    return decimals as number
  } catch {
    return null
  }
}

/**
 * Convert a bytes32 value to a 20-byte address.
 * Extracts the last 20 bytes (40 hex chars) from the bytes32.
 * E.g., "0x0000000000000000000000005FbDB2315678afecb367f032d93F642f64180aa3"
 *     -> "0x5FbDB2315678afecb367f032d93F642f64180aa3"
 */
export function bytes32ToAddress(bytes32: Hex): Address {
  const clean = bytes32.startsWith('0x') ? bytes32.slice(2) : bytes32
  if (clean.length !== 64) {
    throw new Error(`Expected 64 hex chars for bytes32, got ${clean.length}`)
  }
  const addressHex = clean.slice(-40)
  return `0x${addressHex}` as Address
}

/**
 * Convert a 20-byte address to bytes32 (left-padded with zeros).
 */
export function addressToBytes32(address: Address): Hex {
  const clean = address.slice(2).toLowerCase()
  return `0x${clean.padStart(64, '0')}` as Hex
}

/**
 * Normalize registry evm_token_address to 20-byte address.
 * Handles both 0x + 40 chars (standard) and 0x + 64 chars (bytes32).
 */
export function normalizeToEvmAddress(value: string): Address {
  if (!value) throw new Error('Empty evm_token_address')
  const clean = value.replace(/^0x/i, '')
  if (clean.length === 40) return `0x${clean}` as Address
  if (clean.length === 64) return bytes32ToAddress(`0x${clean}` as Hex)
  throw new Error(`Invalid evm address format: expected 40 or 64 hex chars, got ${clean.length}`)
}
