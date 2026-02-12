import { useTransferHistory } from '../../hooks/useTransferHistory'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'
import { TransferStatusBadge } from './TransferStatusBadge'
import { getExplorerTxUrl } from '../../lib/chains'
import type { TransferRecord } from '../../types/transfer'

export interface RecentTransfersProps {
  limit?: number
}

function formatTime(timestamp: number) {
  return new Date(timestamp).toLocaleDateString() + ' ' + new Date(timestamp).toLocaleTimeString()
}

export function RecentTransfers({ limit = 5 }: RecentTransfersProps) {
  const { transfers } = useTransferHistory(limit)

  if (transfers.length === 0) return null

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium text-gray-400">Recent Transfers</h3>
      <div className="space-y-2">
        {transfers.slice(0, limit).map((tx) => (
          <RecentTransferItem key={tx.id} tx={tx} />
        ))}
      </div>
    </div>
  )
}

function RecentTransferItem({ tx }: { tx: TransferRecord }) {
  const explorerUrl = tx.txHash ? getExplorerTxUrl(tx.sourceChain, tx.txHash) : null

  return (
    <div className="bg-gray-900/50 rounded-lg p-3 border border-gray-700/50 flex items-center justify-between">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-white font-medium">
            {formatAmount(tx.amount, DECIMALS.LUNC)} LUNC
          </span>
          <TransferStatusBadge status={tx.status} />
        </div>
        <p className="text-xs text-gray-500 mt-0.5">
          {tx.sourceChain} → {tx.destChain}
        </p>
        <p className="text-xs text-gray-600">{formatTime(tx.timestamp)}</p>
      </div>
      {explorerUrl && (
        <a
          href={explorerUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="text-blue-400 hover:text-blue-300 text-xs flex-shrink-0 ml-2"
        >
          View →
        </a>
      )}
    </div>
  )
}
