import type { PendingWithdrawData } from '../hooks/useTransferLookup'
import type { WithdrawRateLimitInfo } from '../hooks/useBridgeConfig'
import { bigintFromBaseUnitsString } from '../utils/scientificDecimal'
import { normalizeBridgeAmountToDestDecimals } from '../utils/bridgeAmountDecimals'
import type { TerraRateLimitStatus } from './terraBridgeQueries'

/**
 * Classify whether an approved EVM pending withdraw is blocked by destination
 * TokenRegistry / TokenRateLimit window semantics (same data as Transfer form MAX).
 */
export function computeEvmExecutionRateLimitStatus(
  dest: PendingWithdrawData,
  withdrawRateLimit: WithdrawRateLimitInfo | null,
): TerraRateLimitStatus {
  if (!withdrawRateLimit?.maxPerPeriod) {
    return { kind: 'unknown' }
  }
  const maxPerPeriod = bigintFromBaseUnitsString(withdrawRateLimit.maxPerPeriod)
  if (maxPerPeriod === 0n) {
    return { kind: 'unknown' }
  }

  const srcDec = dest.srcDecimals ?? 18
  const destDec = dest.destDecimals ?? 18
  const payoutAmount = normalizeBridgeAmountToDestDecimals(dest.amount, srcDec, destDec)
  const remainingAmount = bigintFromBaseUnitsString(withdrawRateLimit.remainingAmount)

  if (payoutAmount > maxPerPeriod) {
    return { kind: 'permanently-blocked', maxPerPeriod: withdrawRateLimit.maxPerPeriod }
  }
  if (payoutAmount > remainingAmount) {
    return {
      kind: 'temporarily-blocked',
      periodEndsAt: withdrawRateLimit.periodEndsAt,
      remainingAmount: withdrawRateLimit.remainingAmount,
      fetchedAtWallMs: withdrawRateLimit.fetchedAtWallMs,
    }
  }
  return { kind: 'ok' }
}
