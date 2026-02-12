import { BRIDGE_CONFIG } from '../../utils/constants'

export interface FeeBreakdownProps {
  receiveAmount: string
  symbol?: string
}

export function FeeBreakdown({ receiveAmount, symbol = 'LUNC' }: FeeBreakdownProps) {
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
      <div className="flex justify-between border-t border-white/20 pt-2 text-xs uppercase tracking-wide">
        <span className="text-gray-300">You will receive</span>
        <span className="font-semibold text-[#b8ff3d]">{receiveAmount} {symbol}</span>
      </div>
    </div>
  )
}
