import type { DepositData, PendingWithdrawData } from '../../hooks/useTransferLookup'
import type { HashStatus } from '../../types/transfer'
import { SourceHashCard } from './SourceHashCard'
import { DestHashCard } from './DestHashCard'
import { HashFieldsTable } from './HashFieldsTable'
import { StatusBadge } from './StatusBadge'
import { CancelInfo } from './CancelInfo'
import { ComparisonIndicator } from './ComparisonIndicator'
import { Spinner } from '../ui'

export interface HashComparisonPanelProps {
  source: DepositData | null
  sourceChainName: string | null
  dest: PendingWithdrawData | null
  destChainName: string | null
  status: HashStatus
  matches: boolean | null
  loading: boolean
  error: string | null
}

export function HashComparisonPanel({
  source,
  sourceChainName,
  dest,
  destChainName,
  status,
  matches,
  loading,
  error,
}: HashComparisonPanelProps) {

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Spinner />
      </div>
    )
  }

  if (error) {
    return (
      <div className="border-2 border-red-700/80 bg-red-900/30 p-4">
        <p className="text-red-400">{error}</p>
      </div>
    )
  }

  if (!source && !dest) {
    return (
      <div className="border-2 border-white/20 bg-[#161616] py-12 text-center text-gray-400">
        Enter a transfer hash and click Verify to look up source and destination data.
      </div>
    )
  }

  const comparisonResult = matches === true ? 'match' : matches === false ? 'mismatch' : 'pending'

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between flex-wrap gap-4">
        <StatusBadge status={status} />
        <ComparisonIndicator result={comparisonResult} />
      </div>

      {dest?.cancelled && (
        <CancelInfo
          canceledAt={
            dest.approvedAt > 0n
              ? Number(dest.approvedAt) * 1000
              : Number(dest.submittedAt) * 1000
          }
        />
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {source && <SourceHashCard data={source} chainName={sourceChainName} />}
        {dest && <DestHashCard data={dest} chainName={destChainName} />}
      </div>

      <HashFieldsTable source={source} dest={dest} />
    </div>
  )
}
