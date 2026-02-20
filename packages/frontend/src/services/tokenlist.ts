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

/**
 * Get Terra contract address for a token when tokenlist has it.
 * Used when registry returns symbol/denom but we need terra1xxx for display.
 * Returns address for cw20 tokens matched by tokenId or symbol.
 */
export function getTerraAddressFromList(
  tokenlist: TokenlistData | null,
  tokenId: string,
  symbol?: string
): string | null {
  if (!tokenlist?.tokens) return null
  const id = tokenId.trim().toLowerCase()
  // Direct match: tokenId is address or denom
  let entry = tokenlist.tokens.find((t) => {
    if (t.address) return t.address.toLowerCase() === id
    if (t.denom) return t.denom.toLowerCase() === id
    return false
  })
  // Fallback: match by symbol for cw20 (registry may store symbol as token id)
  if (!entry?.address && symbol?.trim()) {
    entry = tokenlist.tokens.find(
      (t) => t.type === 'cw20' && t.symbol?.toLowerCase() === symbol.trim().toLowerCase()
    ) ?? undefined
  }
  return entry?.address ?? null
}
