import { useTransferStore } from '../../stores/transfer'
import { TransferStatusBadge } from './TransferStatusBadge'
import { getExplorerTxUrl } from '../../lib/chains'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'
import { TokenLogo } from '../ui'

export function ActiveTransferCard() {
  const { activeTransfer } = useTransferStore()

  if (!activeTransfer) return null

  const explorerUrl = activeTransfer.txHash
    ? getExplorerTxUrl(activeTransfer.sourceChain, activeTransfer.txHash)
    : null

  return (
    <div className="border-2 border-amber-500/40 bg-amber-900/20 p-3">
      <div className="mb-1 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <img
            src="/assets/status-pending.png"
            alt=""
            className="h-4 w-4 shrink-0 object-contain"
          />
          <span className="text-xs font-semibold uppercase tracking-wide text-amber-300">Transfer in progress</span>
        </div>
        <TransferStatusBadge status={activeTransfer.status} />
      </div>
      <div className="space-y-1 text-sm text-gray-300">
        <p className="flex items-center gap-1.5">
          <TokenLogo symbol="LUNC" size={18} />
          {formatAmount(activeTransfer.amount, DECIMALS.LUNC)} LUNC {activeTransfer.direction === 'terra-to-evm' ? '→' : '←'}{' '}
          {activeTransfer.sourceChain} to {activeTransfer.destChain}
        </p>
        {activeTransfer.txHash && (
          <p className="break-all font-mono text-xs text-gray-400">
            Tx: {activeTransfer.txHash}
            {explorerUrl && (
              <a
                href={explorerUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="ml-2 text-cyan-300 hover:text-cyan-200"
              >
                View →
              </a>
            )}
          </p>
        )}
      </div>
    </div>
  )
}
