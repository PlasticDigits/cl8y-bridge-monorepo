/**
 * Validation utilities for addresses and hashes.
 *
 * `isValidXchainHashId` / `normalizeXchainHashId` are permissive for URL and form input.
 * For strict comparisons to on-chain values, use `normalizeHash` in `services/hashVerification.ts`.
 * See `docs/SOLANA_BRIDGE_INVARIANTS.md` (INV-HFE1).
 * Recipient-field rules (Terra bech32 + EVM EIP-55 + Solana ed25519): `docs/FRONTEND_BRIDGE_INVARIANTS.md` (INV-RCP1).
 */

import { isAddress } from 'viem'
import { terraAddressToBytes32 } from '../services/hashVerification'

const HEX_HASH_REGEX = /^0x[a-fA-F0-9]{64}$/
const HEX_HASH_NO_PREFIX_REGEX = /^[a-fA-F0-9]{64}$/

/**
 * EVM `0x` address: `viem` strict mode enforces EIP-55 when the string contains mixed case
 * (catches single-character typos in checksummed addresses — GL-117).
 * All-lowercase / all-uppercase hex remains accepted per EIP-55 optional checksum.
 */
export function isValidEvmAddress(value: string): boolean {
  const t = value.trim()
  return isAddress(t as `0x${string}`, { strict: true })
}

/**
 * Terra / CosmWasm `terra1…` addresses: length/prefix plus full bech32 checksum
 * (typos in the last groups must be rejected — see GL-117).
 */
export function isValidTerraAddress(value: string): boolean {
  const t = value.trim()
  if (!t.startsWith('terra1') || (t.length !== 44 && t.length !== 64)) {
    return false
  }
  try {
    terraAddressToBytes32(t)
    return true
  } catch {
    return false
  }
}

export function isValidXchainHashId(value: string): boolean {
  return HEX_HASH_REGEX.test(value) || HEX_HASH_NO_PREFIX_REGEX.test(value)
}

export function normalizeXchainHashId(value: string): string {
  const trimmed = value.trim()
  if (HEX_HASH_NO_PREFIX_REGEX.test(trimmed)) {
    return `0x${trimmed}`
  }
  return trimmed
}

export function isValidAmount(value: string, allowDecimals = true): boolean {
  if (!value || value.trim() === '') return false
  const num = parseFloat(value)
  if (Number.isNaN(num) || num <= 0) return false
  if (!allowDecimals && !Number.isInteger(num)) return false
  return true
}
