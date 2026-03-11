import type { DepositData, PendingWithdrawData } from '../../hooks/useTransferLookup'
import type { HashStatus } from '../../types/transfer'
import type { BridgeChainConfig } from '../../types/chain'
import type { TerraRateLimitStatus } from '../../services/terraBridgeQueries'
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
  sourceChainConfig?: BridgeChainConfig | null
  dest: PendingWithdrawData | null
  destChainName: string | null
  destChainConfig?: BridgeChainConfig | null
  status: HashStatus
  matches: boolean | null
  loading: boolean
  error: string | null
  /** Terra rate limit status when dest is approved but not executed (for EVM→Terra). */
  terraRateLimitStatus?: TerraRateLimitStatus | null
}

export function HashComparisonPanel({
  source,
  sourceChainName,
  sourceChainConfig,
  dest,
  destChainName,
  destChainConfig,
  status,
  matches,
  loading,
  error,
  terraRateLimitStatus,
}: HashComparisonPanelProps) {

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-12">
        <Spinner branded size="lg" />
        <span className="text-sm text-gray-400">Verifying across chains…</span>
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
      <div className="border-2 border-white/20 bg-[#161616] py-12 text-center">
        <img
          src="/assets/empty-verify.png"
          alt=""
          className="mx-auto mb-4 max-h-[414px] max-w-[446px] w-2/3 object-contain opacity-80"
        />
        <p className="text-gray-400">
          Enter an XChain Hash ID and click Verify to look up source and destination data.
        </p>
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

      {terraRateLimitStatus?.kind === 'permanently-blocked' && (
        <div className="border-2 border-amber-700/80 bg-amber-950/30 p-4">
          <p className="text-amber-400 text-xs font-semibold uppercase tracking-wide">
            Rate limit exceeded (permanently blocked)
          </p>
          <p className="text-amber-400/80 text-xs mt-0.5">
            The transfer amount exceeds the Terra contract&apos;s per-period rate limit. The operator cannot
            execute until the limit is raised. Contact support for assistance.
          </p>
        </div>
      )}
      {terraRateLimitStatus?.kind === 'temporarily-blocked' && (
        <div className="border-2 border-amber-700/80 bg-amber-950/30 p-4">
          <p className="text-amber-400 text-xs font-semibold uppercase tracking-wide">
            Rate limit exceeded (temporarily blocked)
          </p>
          <p className="text-amber-400/80 text-xs mt-0.5">
            The current rate limit window is full. The operator will retry automatically after the
            window resets at{' '}
            <span className="font-mono tabular-nums">
              {new Date(terraRateLimitStatus.periodEndsAt * 1000).toLocaleString()}
            </span>
            .
          </p>
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {source && <SourceHashCard data={source} chainName={sourceChainName} chainConfig={sourceChainConfig ?? null} />}
        {dest && <DestHashCard data={dest} chainName={destChainName} chainConfig={destChainConfig ?? null} />}
      </div>

      <HashFieldsTable source={source} dest={dest} />
    </div>
  )
}
