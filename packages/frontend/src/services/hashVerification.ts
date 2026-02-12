/**
 * Hash Verification Service
 *
 * Computes transfer hashes matching HashLib.computeTransferHash (Solidity) and
 * multichain-rs compute_transfer_hash (Rust). Used for cross-chain verification.
 *
 * V2 Format: keccak256(abi.encode(
 *   bytes32(srcChain), bytes32(destChain), srcAccount, destAccount, token, amount, uint256(nonce)
 * ))
 * Chain IDs are bytes4 (4 bytes) left-padded to bytes32.
 */

import { keccak256, encodeAbiParameters, parseAbiParameters, toBytes, type Address, type Hex } from 'viem'

// Native uluna token: keccak256("uluna") - matches Solidity keccak256(abi.encodePacked("uluna"))
const ULUNA_TOKEN_BYTES32 = '0x56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da' as Hex

/**
 * Convert chain ID (number) to bytes4, then to bytes32 for hash encoding.
 * Matches Solidity bytes32(bytes4(chainId)) - left-aligned.
 */
export function chainIdToBytes32(chainId: number): Hex {
  if (chainId < 0 || chainId > 0xffffffff) {
    throw new Error(`Chain ID ${chainId} out of bytes4 range`)
  }
  // bytes4 left-aligned in bytes32 (Solidity bytes32(bytes4))
  const hex = chainId.toString(16).padStart(8, '0')
  return `0x${hex}${'0'.repeat(56)}` as Hex
}

/**
 * Convert EVM address to bytes32 (left-padded with zeros).
 * Matches HashLib.addressToBytes32.
 */
export function evmAddressToBytes32(address: Address): Hex {
  const clean = address.slice(2).toLowerCase()
  if (clean.length !== 40) throw new Error('Invalid EVM address length')
  return `0x${clean.padStart(64, '0')}` as Hex
}

/**
 * Token bytes32 for native uluna. Matches cross-chain convention.
 */
export function tokenUlunaBytes32(): Hex {
  return ULUNA_TOKEN_BYTES32
}

/**
 * Compute keccak256("uluna") for native token encoding.
 * Exported for tests and token encoding use cases.
 */
export function keccak256Uluna(): Hex {
  return keccak256(toBytes('uluna'))
}

/**
 * Compute transfer hash matching HashLib.computeTransferHash.
 *
 * @param srcChain - Source chain bytes4 as bytes32 (use chainIdToBytes32)
 * @param destChain - Dest chain bytes4 as bytes32
 * @param srcAccount - Source account bytes32
 * @param destAccount - Dest account bytes32
 * @param token - Token bytes32
 * @param amount - Transfer amount
 * @param nonce - Deposit nonce
 */
export function computeTransferHash(
  srcChain: Hex,
  destChain: Hex,
  srcAccount: Hex,
  destAccount: Hex,
  token: Hex,
  amount: bigint,
  nonce: bigint
): Hex {
  const encoded = encodeAbiParameters(
    parseAbiParameters('bytes32, bytes32, bytes32, bytes32, bytes32, uint256, uint256'),
    [srcChain, destChain, srcAccount, destAccount, token, amount, nonce]
  )
  return keccak256(encoded)
}

/**
 * Compute transfer hash from EVM DepositRecord fields.
 * Call with thisChainId (source) and record from getDeposit(hash).
 */
export function computeTransferHashFromDeposit(
  thisChainId: number,
  destChain: Hex,
  srcAccount: Hex,
  destAccount: Hex,
  token: Hex,
  amount: bigint,
  nonce: bigint
): Hex {
  const srcChain = chainIdToBytes32(thisChainId)
  return computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, amount, nonce)
}

/**
 * Compute transfer hash from EVM PendingWithdraw fields.
 * Call with thisChainId (dest) and withdraw from getPendingWithdraw(hash).
 */
export function computeTransferHashFromWithdraw(
  srcChain: Hex,
  thisChainId: number,
  srcAccount: Hex,
  destAccount: Hex,
  token: Hex,
  amount: bigint,
  nonce: bigint
): Hex {
  const destChain = chainIdToBytes32(thisChainId)
  return computeTransferHash(srcChain, destChain, srcAccount, destAccount, token, amount, nonce)
}

/**
 * Normalize hash to 0x-prefixed 64-char hex for comparison.
 */
export function normalizeHash(hash: string): Hex {
  const trimmed = hash.trim().toLowerCase()
  if (/^0x[a-f0-9]{64}$/.test(trimmed)) return trimmed as Hex
  if (/^[a-f0-9]{64}$/.test(trimmed)) return (`0x${trimmed}` as Hex)
  throw new Error('Invalid transfer hash format (expected 64 hex chars)')
}

/**
 * Convert base64 string to 0x-prefixed hex.
 * Used for Terra LCD Binary field decoding.
 */
export function base64ToHex(b64: string): Hex {
  try {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
    const hex = Array.from(bytes)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('')
    return `0x${hex}` as Hex
  } catch (err) {
    throw new Error(`Invalid base64: ${err instanceof Error ? err.message : 'unknown error'}`)
  }
}

/**
 * Convert 0x-prefixed hex to base64 string.
 * Used for Terra LCD query encoding.
 */
export function hexToBase64(hex: Hex): string {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex
  if (clean.length % 2 !== 0) {
    throw new Error('Hex string must have even length')
  }
  const bytes = Uint8Array.from(
    clean.match(/.{1,2}/g)!.map((byte) => parseInt(byte, 16))
  )
  return btoa(String.fromCharCode(...bytes))
}

/**
 * Convert Terra bech32 address to bytes32.
 * Decodes the bech32 address to its raw pubkey hash (20 bytes), then left-pads to 32 bytes.
 *
 * Note: This is a simplified implementation. For production, use a proper bech32 decoder
 * library. Terra addresses are typically 20-byte pubkey hashes.
 */
export function terraAddressToBytes32(bech32Address: string): Hex {
  // Terra Classic addresses start with "terra1" and are 44 chars total
  // The last 32 chars are the base32-encoded pubkey hash
  // For now, we'll use a placeholder that assumes the address is already
  // in a format we can work with. In production, use @cosmjs/encoding or similar.
  
  // Simple validation
  if (!bech32Address.startsWith('terra1') || bech32Address.length !== 44) {
    throw new Error(`Invalid Terra address format: ${bech32Address}`)
  }

  // For MVP, we'll need to decode bech32 properly. For now, return a placeholder.
  // TODO: Integrate proper bech32 decoding (e.g., from @cosmjs/encoding)
  // This is a temporary implementation that will need proper bech32 decoding.
  
  // Placeholder: assume we can extract 20 bytes somehow
  // In reality, we need to decode bech32 -> bytes -> keccak256 -> take first 20 bytes -> pad to 32
  throw new Error('terraAddressToBytes32: Proper bech32 decoding not yet implemented. Use EVM addresses for now.')
}
