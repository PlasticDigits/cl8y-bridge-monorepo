/**
 * Token logo utilities.
 * Uses logos from /tokens/ when symbol matches (case-insensitive).
 * Tokenlist crosschain info is not required - we match purely on symbol.
 */

/** Symbols we have logo assets for (from /tokens/*.png) */
const LOGO_SYMBOLS = new Set([
  'ALPHA', 'BTC', 'CL8Y', 'CZB', 'CZUSD', 'DOGE', 'ETH', 'LUNC', 'SOL',
  'SPACEUSD', 'TRX', 'USDT', 'USTC', 'USTR', 'USTRIX',
])

/** Terra native denom -> display symbol for logo lookup */
const DENOM_TO_SYMBOL: Record<string, string> = {
  uluna: 'LUNC',
  uusd: 'USTC',
}

/**
 * Returns display symbol for a token identifier (denom or cw20 address).
 * Maps known Terra denoms to friendly symbols (uluna -> LUNC, uusd -> USTC).
 */
export function getTokenDisplaySymbol(tokenId: string): string {
  if (!tokenId?.trim()) return ''
  const lower = tokenId.toLowerCase()
  return DENOM_TO_SYMBOL[lower] ?? tokenId
}

/**
 * Returns the logo URL for a symbol when we have a matching asset.
 * Match is case-insensitive (e.g. "lunc" or "SpaceUSD" -> SPACEUSD.png).
 */
export function getTokenLogoUrl(symbol: string): string | null {
  if (!symbol?.trim()) return null
  const normalized = symbol.trim().toUpperCase()
  if (!LOGO_SYMBOLS.has(normalized)) return null
  return `/tokens/${normalized}.png`
}

/**
 * Returns the logo URL for a token identifier (denom or symbol).
 * Maps known Terra denoms (uluna, uusd) to their display symbol for logo lookup.
 */
export function getTokenLogoUrlFromId(tokenId: string): string | null {
  if (!tokenId?.trim()) return null
  const lower = tokenId.toLowerCase()
  const symbol = DENOM_TO_SYMBOL[lower] ?? tokenId.trim()
  return getTokenLogoUrl(symbol)
}
