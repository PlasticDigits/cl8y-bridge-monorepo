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
import { destExecuteBlockerKind } from '../../utils/hashVerifyExecuteBlocker'
import { formatCountdownMmSs } from '../../utils/format'

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
  /** Dest rate-limit status when approved but not executed (Terra LCD or EVM TokenRegistry). */
  terraRateLimitStatus?: TerraRateLimitStatus | null
  /** Seconds remaining in dest cancel window (on-chain approvedAt + cancelWindow). */
  cancelWindowRemaining?: number | null
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
  cancelWindowRemaining,
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
  const blocker = destExecuteBlockerKind({
    approved: !!dest?.approved,
    executed: !!dest?.executed,
    cancelled: !!dest?.cancelled,
    cancelWindowRemaining,
    rateLimitKind: terraRateLimitStatus?.kind ?? null,
  })

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between flex-wrap gap-4">
        <StatusBadge status={status} />
        <ComparisonIndicator result={comparisonResult} />
      </div>

      {dest?.cancelled && (
        <CancelInfo
          canceledAt={(() => {
            if (dest.approvedAt > 0n) return Number(dest.approvedAt) * 1000
            if (dest.submittedAt > 0n) return Number(dest.submittedAt) * 1000
            return undefined
          })()}
        />
      )}

      {blocker === 'permanently-blocked' && (
        <div className="border-2 border-red-700 bg-[#221313] p-4 shadow-[3px_3px_0_#000]">
          <p className="text-red-400 text-xs font-semibold uppercase tracking-wide">
            Execution blocked
          </p>
          <p className="text-red-400/80 text-xs mt-0.5">
            The transfer amount exceeds the maximum daily rate limit. It cannot be executed even after
            the rate window resets. Contact support for assistance.
          </p>
        </div>
      )}
      {blocker === 'temporarily-blocked' && terraRateLimitStatus?.kind === 'temporarily-blocked' && (
        <div className="border-2 border-amber-700 bg-[#221c13] p-4 shadow-[3px_3px_0_#000]">
          <p className="text-amber-400 text-xs font-semibold uppercase tracking-wide">
            Rate limit window full
          </p>
          <p className="text-amber-400/80 text-xs mt-0.5">
            The current rate limit window is full. The operator will retry automatically after the
            window resets at{' '}
            <span className="font-mono tabular-nums text-amber-300">
              {new Date(terraRateLimitStatus.periodEndsAt * 1000).toLocaleString()}
            </span>
            .
          </p>
        </div>
      )}
      {blocker === 'cancel-window' && (
        <div className="border-2 border-cyan-700 bg-[#121c22] p-4 shadow-[3px_3px_0_#000]">
          <p className="text-blue-300 text-xs font-semibold uppercase tracking-wide">
            Cancel Window Active
          </p>
          <p className="text-blue-400/70 text-xs mt-1">
            Approved. Waiting for the cancel window to expire before tokens are released.
            {cancelWindowRemaining != null && cancelWindowRemaining > 0 && (
              <span className="ml-1 font-mono text-base font-semibold tabular-nums text-cyan-300">
                {formatCountdownMmSs(cancelWindowRemaining)} remaining
              </span>
            )}
          </p>
        </div>
      )}
      {blocker === 'unknown-rate-limit' && (
        <div className="border-2 border-amber-700 bg-[#221c13] p-4 shadow-[3px_3px_0_#000]">
          <p className="text-amber-400 text-xs font-semibold uppercase tracking-wide">
            Execution may be delayed
          </p>
          <p className="text-amber-400/80 text-xs mt-0.5">
            Rate limit status could not be determined. The transfer is approved but not yet executed
            — it may be waiting for the rate limit window to reset.
          </p>
        </div>
      )}
      {blocker === 'awaiting-execute' && (
        <div className="border-2 border-cyan-700 bg-[#121c22] p-4 shadow-[3px_3px_0_#000]">
          <p className="text-blue-300 text-xs font-semibold uppercase tracking-wide">
            Awaiting operator execute
          </p>
          <p className="text-blue-400/70 text-xs mt-1">
            The cancel window has elapsed. The operator will call withdraw execute on the destination
            chain. This page stays Pending until dest state is Executed.
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
