import Blockies from 'react-blockies'
import { getTokenLogoUrl, getTokenLogoUrlFromId } from '../../utils/tokenLogos'

export interface TokenLogoProps {
  /** Display symbol (e.g. "LUNC", "ETH") */
  symbol?: string
  /** Token id from registry (denom or cw20:addr) - used when symbol is not available */
  tokenId?: string
  /** When no logo exists, render blockie from this address (terra1... or 0x...) */
  addressForBlockie?: string
  size?: number
  className?: string
}

/**
 * Renders a token logo when we have a matching asset in /tokens/.
 * Falls back to blockie when no logo exists and addressForBlockie is provided.
 * Falls back to nothing when neither exists.
 */
export function TokenLogo({ symbol, tokenId, addressForBlockie, size = 20, className = '' }: TokenLogoProps) {
  const src = symbol ? getTokenLogoUrl(symbol) : tokenId ? getTokenLogoUrlFromId(tokenId) : null
  if (src) {
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
  if (addressForBlockie) {
    const scale = Math.max(2, Math.ceil(size / 6))
    return (
      <span
        className={`inline-block shrink-0 overflow-hidden rounded-full ${className}`}
        style={{ width: size, height: size }}
      >
        <Blockies seed={addressForBlockie.toLowerCase()} size={6} scale={scale} />
      </span>
    )
  }
  return null
}
