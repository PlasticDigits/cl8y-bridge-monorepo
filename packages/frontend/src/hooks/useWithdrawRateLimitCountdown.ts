import { useState, useEffect } from 'react'

function computeRemainingSeconds(
  periodEndsAt: number,
  fetchedAtWallMs: number | null | undefined,
): number {
  if (fetchedAtWallMs != null) {
    const elapsed = Date.now() - fetchedAtWallMs
    const remainMs = periodEndsAt * 1000 - (fetchedAtWallMs + elapsed)
    return Math.max(0, Math.floor(remainMs / 1000))
  }
  return Math.max(0, periodEndsAt - Math.floor(Date.now() / 1000))
}

/**
 * Seconds until `periodEndsAt` (unix seconds), updating every second.
 * When `fetchedAtWallMs` is set, extrapolates like `SourceChainSelector` so the
 * timer stays aligned with the snapshot taken at fetch time.
 */
export function useWithdrawRateLimitCountdown(
  periodEndsAt: number | null | undefined,
  fetchedAtWallMs: number | null | undefined,
  enabled: boolean,
): number | null {
  const [remainingSec, setRemainingSec] = useState<number | null>(() => {
    if (!enabled || periodEndsAt == null || periodEndsAt <= 0) return null
    return computeRemainingSeconds(periodEndsAt, fetchedAtWallMs)
  })

  useEffect(() => {
    if (!enabled || periodEndsAt == null || periodEndsAt <= 0) {
      setRemainingSec(null)
      return
    }
    const tick = () => {
      setRemainingSec(computeRemainingSeconds(periodEndsAt, fetchedAtWallMs))
    }
    tick()
    const id = window.setInterval(tick, 1000)
    return () => window.clearInterval(id)
  }, [periodEndsAt, fetchedAtWallMs, enabled])

  return enabled ? remainingSec : null
}
