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
    <div className="border-2 border-white/20 bg-[#161616] p-4">
      <h4 className="mb-3 text-sm font-medium uppercase tracking-wider text-gray-300">Destination (Withdraw)</h4>
      <div className="space-y-2 text-sm">
        <p>
          <span className="text-gray-400">Chain:</span>{' '}
          <span className="text-white">{destChainLabel}</span>
        </p>
        <p>
          <span className="text-gray-400">Src chain:</span>{' '}
          <span className="text-white">{srcChainLabel}</span>
        </p>
        <p>
          <span className="text-gray-400">State:</span>{' '}
          <span className={stateColor}>{stateLabel}</span>
        </p>
        <p>
          <span className="text-gray-400">Amount:</span>{' '}
          <span className="text-white">{formatAmount(data.amount, DECIMALS.LUNC)} LUNC</span>
        </p>
        <p>
          <span className="text-gray-400">Nonce:</span>{' '}
          <span className="text-white">{data.nonce.toString()}</span>
        </p>
        <p className="truncate">
          <span className="text-gray-400">Src account:</span>{' '}
          <span className="text-white font-mono text-xs">{data.srcAccount.slice(0, 10)}...{data.srcAccount.slice(-8)}</span>
        </p>
        <p className="truncate">
          <span className="text-gray-400">Dest account:</span>{' '}
          <span className="text-white font-mono text-xs">{data.destAccount.slice(0, 10)}...{data.destAccount.slice(-8)}</span>
        </p>
        {data.submittedAt > 0n && (
          <p>
            <span className="text-gray-400">Submitted:</span>{' '}
            <span className="text-white">{new Date(Number(data.submittedAt) * 1000).toLocaleString()}</span>
          </p>
        )}
      </div>
    </div>
  )
}
