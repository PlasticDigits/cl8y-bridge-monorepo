import type { DepositData } from '../../hooks/useTransferLookup'
import { bytes4ChainIdToLabel, chainIdToLabel } from '../../utils/chainLabel'
import { formatAmount } from '../../utils/format'
import { DECIMALS } from '../../utils/constants'

export interface SourceHashCardProps {
  data: DepositData
  chainName?: string | null
}

export function SourceHashCard({ data, chainName }: SourceHashCardProps) {
  const srcChainLabel = chainName || chainIdToLabel(data.chainId)
  // destChain is bytes32, extract bytes4 (first 10 chars: 0x + 8 hex)
  const destChainBytes4 = data.destChain.slice(0, 10)
  const destChainLabel = bytes4ChainIdToLabel(destChainBytes4)

  return (
    <div className="bg-gray-900/50 border border-gray-700 rounded-xl p-4">
      <h4 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-3">Source (Deposit)</h4>
      <div className="space-y-2 text-sm">
        <p>
          <span className="text-gray-500">Chain:</span>{' '}
          <span className="text-white">{srcChainLabel}</span>
        </p>
        <p>
          <span className="text-gray-500">Dest chain:</span>{' '}
          <span className="text-white">{destChainLabel}</span>
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
        <p>
          <span className="text-gray-500">Timestamp:</span>{' '}
          <span className="text-white">{new Date(Number(data.timestamp) * 1000).toLocaleString()}</span>
        </p>
      </div>
    </div>
  )
}
