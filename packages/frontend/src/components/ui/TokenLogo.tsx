import { getTokenLogoUrl, getTokenLogoUrlFromId } from '../../utils/tokenLogos'

export interface TokenLogoProps {
  /** Display symbol (e.g. "LUNC", "ETH") */
  symbol?: string
  /** Token id from registry (denom or cw20:addr) - used when symbol is not available */
  tokenId?: string
  size?: number
  className?: string
}

/**
 * Renders a token logo when we have a matching asset in /tokens/.
 * Falls back to nothing when no logo exists.
 */
export function TokenLogo({ symbol, tokenId, size = 20, className = '' }: TokenLogoProps) {
  const src = symbol ? getTokenLogoUrl(symbol) : tokenId ? getTokenLogoUrlFromId(tokenId) : null
  if (!src) return null
  return (
    <img
      src={src}
      alt=""
      width={size}
      height={size}
      className={`shrink-0 rounded-full object-contain ${className}`}
    />
  )
}
