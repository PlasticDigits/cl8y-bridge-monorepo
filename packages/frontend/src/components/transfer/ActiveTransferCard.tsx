import { useTransferStore } from '../../stores/transfer'
import { TransferStatusBadge } from './TransferStatusBadge'
import { getExplorerTxUrl } from '../../lib/chains'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'

export function ActiveTransferCard() {
  const { activeTransfer } = useTransferStore()

  if (!activeTransfer) return null

  const explorerUrl = activeTransfer.txHash
    ? getExplorerTxUrl(activeTransfer.sourceChain, activeTransfer.txHash)
    : null

  return (
    <div className="bg-amber-900/20 border border-amber-500/30 rounded-xl p-4">
      <div className="flex items-center justify-between mb-2">
        <span className="text-amber-400 font-medium">Transfer in progress</span>
        <TransferStatusBadge status={activeTransfer.status} />
      </div>
      <div className="text-sm text-gray-400 space-y-1">
        <p>
          {formatAmount(activeTransfer.amount, DECIMALS.LUNC)} LUNC {activeTransfer.direction === 'terra-to-evm' ? '→' : '←'}{' '}
          {activeTransfer.sourceChain} to {activeTransfer.destChain}
        </p>
        {activeTransfer.txHash && (
          <p className="font-mono text-xs break-all">
            Tx: {activeTransfer.txHash}
            {explorerUrl && (
              <a
                href={explorerUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="ml-2 text-blue-400 hover:text-blue-300"
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
