/**
 * Shorten an address for display: first N chars + ... + last M chars.
 * Used for token addresses when no symbol is available.
 */

const EVM_PREFIX = 4 // hex chars (0x + 4 = 6 total before ...)
const EVM_SUFFIX = 4
const TERRA_PREFIX = 12 // terra1 (6) + 6 more
const TERRA_SUFFIX = 6

/**
 * Check if a string looks like an EVM or Terra address.
 */
export function isAddressLike(s: string): boolean {
  if (!s || typeof s !== 'string') return false
  const t = s.trim()
  return (t.startsWith('0x') && t.length >= 42) || (t.startsWith('terra1') && t.length >= 44)
}

/**
 * Shorten an address for display.
 * EVM: 0x1234...5678 (6 + 4)
 * Terra: terra1ab...xyz123 (8 + 6)
 */
export function shortenAddress(address: string): string {
  if (!address || typeof address !== 'string') return ''
  const s = address.trim()
  if (s.startsWith('0x') && s.length >= 20) {
    return `${s.slice(0, EVM_PREFIX + 2)}...${s.slice(-EVM_SUFFIX)}`
  }
  if (s.startsWith('terra1') && s.length >= 20) {
    return `${s.slice(0, TERRA_PREFIX)}...${s.slice(-TERRA_SUFFIX)}`
  }
  return s
}
