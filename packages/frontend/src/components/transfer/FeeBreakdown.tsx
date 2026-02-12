import { BRIDGE_CONFIG } from '../../utils/constants'

export interface FeeBreakdownProps {
  receiveAmount: string
  symbol?: string
}

export function FeeBreakdown({ receiveAmount, symbol = 'LUNC' }: FeeBreakdownProps) {
  return (
    <div className="bg-gray-900/50 rounded-lg p-4 space-y-2">
      <div className="flex justify-between text-sm">
        <span className="text-gray-400">Bridge Fee</span>
        <span className="text-white">{BRIDGE_CONFIG.feePercent}%</span>
      </div>
      <div className="flex justify-between text-sm">
        <span className="text-gray-400">Estimated Time</span>
        <span className="text-white">~{Math.ceil(BRIDGE_CONFIG.withdrawDelay / 60)} minutes</span>
      </div>
      <div className="flex justify-between text-sm">
        <span className="text-gray-400">You will receive</span>
        <span className="text-white font-medium">{receiveAmount} {symbol}</span>
      </div>
    </div>
  )
}
