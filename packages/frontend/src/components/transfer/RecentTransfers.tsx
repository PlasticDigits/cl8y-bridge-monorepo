import { useTransferHistory } from '../../hooks/useTransferHistory'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'
import { TransferStatusBadge } from './TransferStatusBadge'
import { getExplorerTxUrl } from '../../lib/chains'
import type { TransferRecord } from '../../types/transfer'
import { TokenLogo } from '../ui'

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
    <div className="space-y-2">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-gray-300">Recent Transfers</h3>
      <div className="space-y-1.5">
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
    <div className="flex items-center justify-between border-2 border-white/20 bg-[#161616] p-2.5">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <TokenLogo symbol="LUNC" size={18} />
          <span className="text-sm font-semibold text-white">
            {formatAmount(tx.amount, DECIMALS.LUNC)} LUNC
          </span>
          <TransferStatusBadge status={tx.status} />
        </div>
        <p className="mt-0.5 text-xs text-gray-300">
          {tx.sourceChain} → {tx.destChain}
        </p>
        <p className="text-[11px] text-gray-400">{formatTime(tx.timestamp)}</p>
      </div>
      {explorerUrl && (
        <a
          href={explorerUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="ml-2 shrink-0 text-xs text-cyan-300 hover:text-cyan-200"
        >
          View →
        </a>
      )}
    </div>
  )
}
