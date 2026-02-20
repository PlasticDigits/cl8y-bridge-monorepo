import { TokenLogo } from '../ui'
import type { TokenOption } from './TokenSelect'
import { sounds } from '../../lib/sounds'
import type { BridgeChainConfig } from '../../types/chain'
import { TokenSelect } from './TokenSelect'
import { isAddressLike, shortenAddress } from '../../utils/shortenAddress'

export interface AmountInputProps {
  value: string
  onChange: (value: string) => void
  onMax?: () => void
  symbol?: string
  /** When provided, shows a token dropdown instead of a static symbol */
  tokens?: TokenOption[]
  selectedTokenId?: string
  onTokenChange?: (tokenId: string) => void
  placeholder?: string
  disabled?: boolean
  /** Source chain config or rpcUrl when EVM - enables onchain symbol lookup with RPC fallbacks */
  sourceChainConfigOrRpcUrl?: BridgeChainConfig | string
  /** Compact-formatted max amount for label (e.g. "100" or "1.23k") */
  maxLabel?: string
  /** Compact-formatted min amount for label (e.g. "0.001" or "1e-5") */
  minLabel?: string
}

export function AmountInput({
  value,
  onChange,
  onMax,
  symbol = 'LUNC',
  tokens,
  selectedTokenId,
  onTokenChange,
  placeholder = '0.0',
  disabled,
  sourceChainConfigOrRpcUrl,
  maxLabel,
  minLabel,
}: AmountInputProps) {
  const hasTokenSelector = tokens && tokens.length > 0 && onTokenChange
  const selectedToken = tokens?.find((t) => t.id === selectedTokenId)
  // Prefer parent-provided symbol (resolved via onchain/tokenlist) over token option's symbol,
  // since token options may have address when not in tokenlist
  const rawSymbol = selectedToken?.symbol ?? symbol
  const displaySymbol = symbol && !isAddressLike(symbol) ? symbol : rawSymbol
  const displayLabel = isAddressLike(displaySymbol) ? shortenAddress(displaySymbol) : displaySymbol
  const addressForBlockie =
    selectedToken?.evmTokenAddress?.startsWith('0x')
      ? selectedToken.evmTokenAddress
      : selectedToken && isAddressLike(selectedToken.tokenId)
        ? selectedToken.tokenId
        : isAddressLike(symbol)
          ? symbol
          : undefined

  return (
    <div>
      <label className="mb-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs font-semibold uppercase tracking-wide text-gray-300">
        <span>Amount</span>
        {(displayLabel || maxLabel != null || minLabel != null) && (
          <span className="flex items-center gap-1.5 normal-case font-normal text-gray-400">
            <TokenLogo
              symbol={selectedToken?.symbol ?? symbol}
              tokenId={selectedToken?.tokenId}
              addressForBlockie={addressForBlockie}
              size={14}
            />
            {displayLabel}
            {maxLabel != null && (
              <>
                {' · '}
                <span className="text-cyan-400/90">MAX {maxLabel}</span>
              </>
            )}
            {minLabel != null && (
              <>
                {' · '}
                <span className="text-amber-400/90">MIN {minLabel}</span>
              </>
            )}
          </span>
        )}
      </label>
      <div className="relative">
        <input
          type="number"
          data-testid="amount-input"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          step="0.000001"
          min="0"
          disabled={disabled}
          className="w-full border-2 border-white/20 bg-[#161616] px-3 py-2 pr-20 text-lg text-white focus:border-cyan-300 focus:outline-none disabled:opacity-50"
        />
        <div className="absolute right-3 top-1/2 z-20 -translate-y-1/2 flex items-center gap-2">
          {onMax && (
            <button
              type="button"
              onClick={() => {
                sounds.playButtonPress()
                onMax()
              }}
              className="border border-cyan-400 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-cyan-300 hover:bg-cyan-400/10"
            >
              MAX
            </button>
          )}
          {hasTokenSelector ? (
            <TokenSelect
              tokens={tokens}
              value={selectedTokenId ?? tokens[0]?.id ?? ''}
              onChange={onTokenChange}
              disabled={disabled}
              sourceChainConfigOrRpcUrl={sourceChainConfigOrRpcUrl}
            />
          ) : (
            <>
              <TokenLogo symbol={symbol} size={18} />
              <span className="text-xs uppercase tracking-wide text-gray-400">{symbol}</span>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
