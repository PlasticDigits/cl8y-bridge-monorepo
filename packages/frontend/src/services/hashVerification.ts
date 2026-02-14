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
 * Bech32 character set (lowercase).
 */
const BECH32_CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'

/**
 * Decode bech32 data part to 5-bit values.
 */
function bech32Decode(bech32Str: string): { hrp: string; data5bit: number[] } {
  const lower = bech32Str.toLowerCase()
  const sepIdx = lower.lastIndexOf('1')
  if (sepIdx < 1) throw new Error('Invalid bech32: no separator')

  const hrp = lower.slice(0, sepIdx)
  const dataPart = lower.slice(sepIdx + 1)

  const values: number[] = []
  for (const ch of dataPart) {
    const idx = BECH32_CHARSET.indexOf(ch)
    if (idx === -1) throw new Error(`Invalid bech32 character: ${ch}`)
    values.push(idx)
  }

  // Last 6 values are checksum, skip them
  return { hrp, data5bit: values.slice(0, values.length - 6) }
}

/**
 * Convert 5-bit groups to 8-bit bytes (bech32 -> raw bytes).
 */
function convertBits(data: number[], fromBits: number, toBits: number, pad: boolean): number[] {
  let acc = 0
  let bits = 0
  const result: number[] = []
  const maxV = (1 << toBits) - 1

  for (const value of data) {
    if (value < 0 || value >> fromBits) throw new Error('Invalid value for bit conversion')
    acc = (acc << fromBits) | value
    bits += fromBits
    while (bits >= toBits) {
      bits -= toBits
      result.push((acc >> bits) & maxV)
    }
  }

  if (pad) {
    if (bits > 0) {
      result.push((acc << (toBits - bits)) & maxV)
    }
  } else if (bits >= fromBits || ((acc << (toBits - bits)) & maxV)) {
    throw new Error('Invalid padding in bit conversion')
  }

  return result
}

/**
 * Convert Terra bech32 address to bytes32.
 * Decodes the bech32 address to its raw pubkey hash (20 bytes), then left-pads to 32 bytes.
 *
 * Terra Classic uses standard bech32 (not segwit/bech32m), so there is no witness version byte.
 * All 5-bit data values are converted directly to 8-bit bytes.
 */
export function terraAddressToBytes32(bech32Address: string): Hex {
  if (!bech32Address.startsWith('terra1') || bech32Address.length !== 44) {
    throw new Error(`Invalid Terra address format: ${bech32Address}`)
  }

  // Decode bech32 to get the 5-bit data
  const { data5bit } = bech32Decode(bech32Address)

  // Terra Classic: no witness version byte. Convert all 5-bit values to 8-bit bytes.
  // Use pad=false because the trailing bits should be zero-padding from encoding.
  // However, some valid bech32 addresses may have non-zero trailing bits that are
  // artifacts of the 5-to-8-bit conversion. Use pad=true to be permissive.
  const rawBytes = convertBits(data5bit, 5, 8, false)

  if (rawBytes.length !== 20) {
    throw new Error(`Expected 20-byte pubkey hash, got ${rawBytes.length} bytes`)
  }

  // Left-pad to 32 bytes (same as EVM address encoding)
  const hex = rawBytes.map((b) => b.toString(16).padStart(2, '0')).join('')
  return `0x${hex.padStart(64, '0')}` as Hex
}

// ─── Bech32 Encoding (for bytes32 → terra1... reverse conversion) ───

const BECH32_GENERATORS = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]

function bech32Polymod(values: number[]): number {
  let chk = 1
  for (const v of values) {
    const b = chk >> 25
    chk = ((chk & 0x1ffffff) << 5) ^ v
    for (let i = 0; i < 5; i++) {
      if ((b >> i) & 1) chk ^= BECH32_GENERATORS[i]
    }
  }
  return chk
}

function bech32HrpExpand(hrp: string): number[] {
  const result: number[] = []
  for (const ch of hrp) result.push(ch.charCodeAt(0) >> 5)
  result.push(0)
  for (const ch of hrp) result.push(ch.charCodeAt(0) & 31)
  return result
}

function bech32CreateChecksum(hrp: string, data5bit: number[]): number[] {
  const values = [...bech32HrpExpand(hrp), ...data5bit, 0, 0, 0, 0, 0, 0]
  const polymod = bech32Polymod(values) ^ 1
  return Array.from({ length: 6 }, (_, i) => (polymod >> (5 * (5 - i))) & 31)
}

function bech32Encode(hrp: string, data5bit: number[]): string {
  const checksum = bech32CreateChecksum(hrp, data5bit)
  return hrp + '1' + [...data5bit, ...checksum].map((v) => BECH32_CHARSET[v]).join('')
}

/**
 * Convert bytes32 hex back to a Terra bech32 address.
 * Reverses terraAddressToBytes32: extracts the last 20 bytes, bech32-encodes with "terra" HRP.
 */
export function bytes32ToTerraAddress(bytes32Hex: string): string {
  const clean = bytes32Hex.startsWith('0x') ? bytes32Hex.slice(2) : bytes32Hex
  if (clean.length !== 64) {
    throw new Error(`Expected 64-char hex (bytes32), got ${clean.length}`)
  }

  // Extract last 20 bytes (40 hex chars) — the canonical address
  const addrHex = clean.slice(-40)
  const padding = clean.slice(0, clean.length - 40)
  if (padding && !/^0+$/.test(padding)) {
    throw new Error('Invalid bytes32 for Terra address: non-zero padding in first 12 bytes')
  }

  // Convert hex to bytes
  const rawBytes: number[] = []
  for (let i = 0; i < 40; i += 2) {
    rawBytes.push(parseInt(addrHex.slice(i, i + 2), 16))
  }

  // Convert 8-bit bytes to 5-bit groups
  const data5bit = convertBits(rawBytes, 8, 5, true)

  // Bech32-encode with "terra" HRP
  return bech32Encode('terra', data5bit)
}
