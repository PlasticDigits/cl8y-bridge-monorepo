import { Link } from 'react-router-dom'
import { useTransferHistory } from '../../hooks/useTransferHistory'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'
import { TransferStatusBadge } from './TransferStatusBadge'
import { getExplorerTxUrl } from '../../lib/chains'
import type { TransferRecord, TransferLifecycle } from '../../types/transfer'
import { TokenLogo, Badge } from '../ui'

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
    <div className="mx-auto max-w-[520px] space-y-2">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-gray-300">Recent Transfers</h3>
      <div className="space-y-1.5">
        {transfers.slice(0, limit).map((tx) => (
          <RecentTransferItem key={tx.id} tx={tx} />
        ))}
      </div>
    </div>
  )
}

const lifecycleBadgeConfig: Record<TransferLifecycle, { label: string; variant: 'success' | 'warning' | 'error' | 'default' | 'neutral' }> = {
  deposited: { label: 'Action Required', variant: 'warning' },
  'hash-submitted': { label: 'Hash Submitted', variant: 'default' },
  approved: { label: 'Approved', variant: 'default' },
  executed: { label: 'Complete', variant: 'success' },
  failed: { label: 'Failed', variant: 'error' },
}

function RecentTransferItem({ tx }: { tx: TransferRecord }) {
  const explorerUrl = tx.txHash ? getExplorerTxUrl(tx.sourceChain, tx.txHash) : null

  // Prefer full transfer hash (links to status page); fallback to tx hash + explorer
  const idEl =
    tx.transferHash ? (
      <Link
        to={`/transfer/${tx.transferHash}`}
        className="block break-all text-xs font-mono font-medium text-cyan-300 transition-colors hover:text-cyan-200"
      >
        {tx.transferHash}
      </Link>
    ) : tx.txHash ? (
      explorerUrl ? (
        <a
          href={explorerUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="block break-all text-xs font-mono font-medium text-cyan-300 transition-colors hover:text-cyan-200"
        >
          {tx.txHash} ↗
        </a>
      ) : (
        <span className="block break-all text-xs font-mono text-gray-400">{tx.txHash}</span>
      )
    ) : null

  // Show lifecycle badge when available; otherwise fall back to deposit status
  const lifecycle = tx.lifecycle || (tx.status === 'confirmed' ? 'deposited' : undefined)
  const lcConfig = lifecycle ? lifecycleBadgeConfig[lifecycle] : null

  const statusBadge = lcConfig ? (
    <Badge
      variant={lcConfig.variant}
      className="inline-flex items-center gap-1.5 rounded-none border-2 px-2 py-0.5 shadow-[1px_1px_0_#000]"
    >
      {lcConfig.label}
    </Badge>
  ) : (
    <TransferStatusBadge status={tx.status} />
  )

  // Show warning banner for transfers stuck at 'deposited' that need action
  const needsAction = lifecycle === 'deposited' && tx.transferHash
  const isFailed = lifecycle === 'failed'

  return (
    <div className={`border-2 bg-[#161616] p-2.5 shadow-[3px_3px_0_#000] ${
      isFailed ? 'border-red-700/60' : needsAction ? 'border-yellow-700/60' : 'border-white/20'
    }`}>
      <div className="flex flex-wrap items-center gap-2">
        <TokenLogo symbol="LUNC" size={18} />
        <span className="text-sm font-semibold text-white">
          {formatAmount(tx.amount, DECIMALS.LUNC)} LUNC
        </span>
        {statusBadge}
      </div>
      <p className="mt-0.5 text-xs text-gray-300">
        {tx.sourceChain} → {tx.destChain}
      </p>
      <p className="text-[11px] text-gray-400">{formatTime(tx.timestamp)}</p>
      {needsAction && (
        <div className="mt-1.5 border-t border-yellow-700/30 pt-1.5 flex items-center justify-between gap-2">
          <p className="text-[11px] text-yellow-400">
            Hash submission required — click to continue
          </p>
          <Link
            to={`/transfer/${tx.transferHash}`}
            className="shrink-0 text-[11px] font-semibold uppercase tracking-wide text-[#b8ff3d] hover:text-[#d5ff7f]"
          >
            Continue →
          </Link>
        </div>
      )}
      {isFailed && (
        <div className="mt-1.5 border-t border-red-700/30 pt-1.5 flex items-center justify-between gap-2">
          <p className="text-[11px] text-red-400">
            Transfer failed — click to view details
          </p>
          {tx.transferHash && (
            <Link
              to={`/transfer/${tx.transferHash}`}
              className="shrink-0 text-[11px] font-semibold uppercase tracking-wide text-red-400 hover:text-red-300"
            >
              Details →
            </Link>
          )}
        </div>
      )}
      {idEl && !needsAction && !isFailed && (
        <p className="mt-1.5 border-t border-white/10 pt-1.5">
          {idEl}
        </p>
      )}
      {(needsAction || isFailed) && idEl && (
        <p className="mt-1 text-[10px]">
          {idEl}
        </p>
      )}
    </div>
  )
}
