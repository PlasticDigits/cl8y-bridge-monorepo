/**
 * Validation utilities for addresses and hashes
 */

const EVM_ADDRESS_REGEX = /^0x[a-fA-F0-9]{40}$/
const TERRA_ADDRESS_REGEX = /^terra1[a-zA-Z0-9]{38}$/
const HEX_HASH_REGEX = /^0x[a-fA-F0-9]{64}$/
const HEX_HASH_NO_PREFIX_REGEX = /^[a-fA-F0-9]{64}$/

export function isValidEvmAddress(value: string): boolean {
  return EVM_ADDRESS_REGEX.test(value)
}

export function isValidTerraAddress(value: string): boolean {
  return TERRA_ADDRESS_REGEX.test(value)
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
  if (Number.isNaN(num) || num < 0) return false
  if (!allowDecimals && !Number.isInteger(num)) return false
  return true
}
