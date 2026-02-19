/**
 * TokenDisplay - Renders token logo + human-readable symbol.
 * Never shows raw addresses; uses shortenAddress or on-chain lookup when needed.
 * Use throughout the app for consistent token identity display.
 */

import { TokenLogo } from './TokenLogo'
import { useTokenDisplay } from '../../hooks/useTokenDisplay'
import type { TokenDisplayProps } from '../../hooks/useTokenDisplay'

export interface TokenDisplayComponentProps extends TokenDisplayProps {
  size?: number
  className?: string
}

/**
 * Displays a token as logo + symbol. Never renders raw full address.
 * When token is an address without a known symbol, shows shortened address + blockie.
 */
export function TokenDisplay({
  tokenId,
  symbol: symbolProp,
  sourceChain,
  size = 18,
  className = '',
}: TokenDisplayComponentProps) {
  const { displayLabel, addressForBlockie, symbol } = useTokenDisplay({
    tokenId,
    symbol: symbolProp,
    sourceChain,
  })

  return (
    <span className={`inline-flex items-center gap-1.5 ${className}`}>
      <TokenLogo
        symbol={symbol}
        tokenId={tokenId ?? undefined}
        addressForBlockie={addressForBlockie}
        size={size}
      />
      {displayLabel && <span>{displayLabel}</span>}
    </span>
  )
}
