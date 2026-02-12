import type { PendingWithdrawData } from '../../hooks/useTransferLookup'
import { bytes4ChainIdToLabel, chainIdToLabel } from '../../utils/chainLabel'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'

export interface DestHashCardProps {
  data: PendingWithdrawData
  chainName?: string | null
}

export function DestHashCard({ data, chainName }: DestHashCardProps) {
  const destChainLabel = chainName || chainIdToLabel(data.chainId)
  // srcChain is bytes32, extract bytes4 (first 10 chars: 0x + 8 hex)
  const srcChainBytes4 = data.srcChain.slice(0, 10)
  const srcChainLabel = bytes4ChainIdToLabel(srcChainBytes4)

  const stateLabel = data.executed
    ? 'Executed'
    : data.cancelled
    ? 'Canceled'
    : data.approved
    ? 'Approved'
    : 'Pending'

  const stateColor = data.executed
    ? 'text-green-400'
    : data.cancelled
    ? 'text-red-400'
    : data.approved
    ? 'text-yellow-400'
    : 'text-gray-400'

  return (
    <div className="bg-gray-900/50 border border-gray-700 rounded-xl p-4">
      <h4 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-3">Destination (Withdraw)</h4>
      <div className="space-y-2 text-sm">
        <p>
          <span className="text-gray-500">Chain:</span>{' '}
          <span className="text-white">{destChainLabel}</span>
        </p>
        <p>
          <span className="text-gray-500">Src chain:</span>{' '}
          <span className="text-white">{srcChainLabel}</span>
        </p>
        <p>
          <span className="text-gray-500">State:</span>{' '}
          <span className={stateColor}>{stateLabel}</span>
        </p>
        <p>
          <span className="text-gray-500">Amount:</span>{' '}
          <span className="text-white">{formatAmount(data.amount, DECIMALS.LUNC)} LUNC</span>
        </p>
        <p>
          <span className="text-gray-500">Nonce:</span>{' '}
          <span className="text-white">{data.nonce.toString()}</span>
        </p>
        <p className="truncate">
          <span className="text-gray-500">Src account:</span>{' '}
          <span className="text-white font-mono text-xs">{data.srcAccount.slice(0, 10)}...{data.srcAccount.slice(-8)}</span>
        </p>
        <p className="truncate">
          <span className="text-gray-500">Dest account:</span>{' '}
          <span className="text-white font-mono text-xs">{data.destAccount.slice(0, 10)}...{data.destAccount.slice(-8)}</span>
        </p>
        {data.submittedAt > 0n && (
          <p>
            <span className="text-gray-500">Submitted:</span>{' '}
            <span className="text-white">{new Date(Number(data.submittedAt) * 1000).toLocaleString()}</span>
          </p>
        )}
      </div>
    </div>
  )
}
