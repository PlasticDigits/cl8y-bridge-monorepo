/**
 * Tokenlist service - resolves token symbol/name from the USTR CMM token list.
 * Used when registry returns raw addresses (terra1..., 0x...) to show friendly names.
 */

export interface TokenlistEntry {
  symbol: string
  name: string
  denom?: string
  address?: string
  type: 'native' | 'cw20'
}

export interface TokenlistData {
  name: string
  version: string
  tokens: Array<{
    symbol: string
    name: string
    denom?: string
    address?: string
    type: 'native' | 'cw20'
  }>
}

let cached: TokenlistData | null = null

/**
 * Fetch tokenlist from public assets. Caches in memory.
 */
export async function fetchTokenlist(): Promise<TokenlistData> {
  if (cached) return cached
  const res = await fetch('/tokens/tokenlist.json')
  if (!res.ok) throw new Error('Failed to fetch tokenlist')
  cached = await res.json()
  return cached!
}

/**
 * Resolve token symbol/name from tokenlist by token id (denom or address).
 */
export function getTokenFromList(
  tokenlist: TokenlistData | null,
  tokenId: string
): { symbol: string; name?: string } | null {
  if (!tokenlist?.tokens || !tokenId?.trim()) return null
  const id = tokenId.trim().toLowerCase()
  const entry = tokenlist.tokens.find((t) => {
    if (t.denom) return t.denom.toLowerCase() === id
    if (t.address) return t.address.toLowerCase() === id
    return false
  })
  return entry ? { symbol: entry.symbol, name: entry.name } : null
}
