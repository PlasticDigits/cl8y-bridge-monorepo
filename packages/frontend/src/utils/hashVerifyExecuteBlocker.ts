import type { HashStatus } from '../types/transfer'

/** Pure Hash Verification status (AC7 / A8). Verified only when dest executed. */
export function computeHashVerificationStatus(
  error: string | null,
  loading: boolean,
  dest: { cancelled: boolean; executed: boolean; approved: boolean } | null,
  sourcePresent: boolean,
): HashStatus {
  if (error) return 'unknown'
  if (loading) return 'pending'
  if (dest?.cancelled) return 'canceled'
  if (dest?.executed) return 'verified'
  if (dest?.approved) return 'pending'
  if (dest && !dest.approved && !dest.cancelled) return 'pending'
  if (sourcePresent && !dest) return 'pending'
  if (!sourcePresent && !dest) return 'unknown'
  return 'pending'
}

export type DestExecuteBlockerKind =
  | 'none'
  | 'cancel-window'
  | 'permanently-blocked'
  | 'temporarily-blocked'
  | 'unknown-rate-limit'
  | 'awaiting-execute'

/**
 * Visible Hash Verification blocker for dest-approved, not-executed rows (GL-170).
 * Rate-limit perm/temp outrank cancel remaining (same order as Transfer Status).
 */
export function destExecuteBlockerKind(input: {
  approved: boolean
  executed: boolean
  cancelled: boolean
  cancelWindowRemaining?: number | null
  rateLimitKind?: 'ok' | 'temporarily-blocked' | 'permanently-blocked' | 'unknown' | null
}): DestExecuteBlockerKind {
  if (!input.approved || input.executed || input.cancelled) return 'none'
  if (input.rateLimitKind === 'permanently-blocked') return 'permanently-blocked'
  if (input.rateLimitKind === 'temporarily-blocked') return 'temporarily-blocked'
  if ((input.cancelWindowRemaining ?? 0) > 0) return 'cancel-window'
  if (input.rateLimitKind === 'unknown') return 'unknown-rate-limit'
  return 'awaiting-execute'
}
