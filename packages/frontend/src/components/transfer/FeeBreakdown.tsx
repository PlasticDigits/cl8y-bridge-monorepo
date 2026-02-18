import { BRIDGE_CONFIG } from '../../utils/constants'
import { TokenDisplay } from '../ui'

export interface FeeBreakdownProps {
  receiveAmount: string
  /** Display symbol when known (avoids async lookup) */
  symbol?: string
  /** Token id (denom, CW20 addr, or EVM addr) for logo + symbol resolution */
  tokenId?: string
  /** Destination chain id - enables EVM on-chain symbol fetch when token is 0x... */
  destChain?: string
  /** Explorer URL for token on destination chain - links token, not source */
  tokenExplorerUrl?: string
}

export function FeeBreakdown({
  receiveAmount,
  symbol = 'LUNC',
  tokenId,
  destChain,
  tokenExplorerUrl,
}: FeeBreakdownProps) {
  const tokenContent = (
    <span className="inline-flex items-center gap-1.5">
      {receiveAmount}{' '}
      <TokenDisplay tokenId={tokenId} symbol={symbol} sourceChain={destChain} size={16} />
    </span>
  )

  return (
    <div className="space-y-1 border-2 border-white/20 bg-[#161616] p-3">
      <div className="flex justify-between text-xs uppercase tracking-wide">
        <span className="text-gray-300">Bridge Fee</span>
        <span className="text-white">{BRIDGE_CONFIG.feePercent}%</span>
      </div>
      <div className="flex justify-between text-xs uppercase tracking-wide">
        <span className="text-gray-300">Estimated Time</span>
        <span className="text-white">~{Math.ceil(BRIDGE_CONFIG.withdrawDelay / 60)} minutes</span>
      </div>
      <div className="flex justify-between items-center gap-2 border-t border-white/20 pt-2 text-xs uppercase tracking-wide">
        <span className="text-gray-300">You will receive</span>
        {tokenExplorerUrl ? (
          <a
            href={tokenExplorerUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1.5 font-semibold text-[#b8ff3d] hover:underline"
          >
            {tokenContent}
          </a>
        ) : (
          <span className="flex items-center gap-1.5 font-semibold text-[#b8ff3d]">{tokenContent}</span>
        )}
      </div>
    </div>
  )
}
