import { useState, useEffect, useRef } from 'react'

/**
 * Expected wall-clock durations per transfer lifecycle step.
 * deposited    – waiting for auto-submit (chain switch + tx)
 * hash-submitted – waiting for operator withdrawApprove
 * approved     – cancel window (BRIDGE_CONFIG.withdrawDelay = 300s)
 * executed     – terminal, not animated
 */
const STEP_EXPECTED_MS: Record<string, number> = {
  deposited: 90_000,
  'hash-submitted': 90_000,
  approved: 300_000,
  executed: 0,
}

const MIN_EXPECTED_MS = 90_000

/**
 * Returns a 0–100 progress value for a transfer step with a
 * three-phase easing curve:
 *
 *   Phase 1 (first 10% of expected time) → quick rise to 20-35 %
 *   Phase 2 (remaining 90% of expected time) → steady rise to 70-85 %
 *   Phase 3 (10× expected time after that) → slow crawl toward 95 %
 *   Completion → 100 %
 */
export function useStepProgress(
  stepKey: string,
  isDone: boolean,
  isActive: boolean,
): number {
  const expectedMs = Math.max(
    STEP_EXPECTED_MS[stepKey] ?? MIN_EXPECTED_MS,
    MIN_EXPECTED_MS,
  )

  const targetsRef = useRef<{ phase1: number; phase2: number } | null>(null)
  const startRef = useRef(0)
  const [progress, setProgress] = useState(() => (isDone ? 100 : 0))

  useEffect(() => {
    if (isDone) {
      setProgress(100)
      targetsRef.current = null
      return
    }

    if (!isActive) {
      setProgress(0)
      targetsRef.current = null
      return
    }

    // Seed random targets once per activation
    if (!targetsRef.current) {
      targetsRef.current = {
        phase1: 20 + Math.random() * 15, // 20–35 %
        phase2: 70 + Math.random() * 15, // 70–85 %
      }
      startRef.current = Date.now()
    }

    const { phase1, phase2 } = targetsRef.current
    const p1End = expectedMs * 0.1
    const p3Cap = 95

    const tick = () => {
      const elapsed = Date.now() - startRef.current
      let v: number

      if (elapsed <= p1End) {
        v = (elapsed / p1End) * phase1
      } else if (elapsed <= expectedMs) {
        v = phase1 + ((elapsed - p1End) / (expectedMs - p1End)) * (phase2 - phase1)
      } else {
        const extra = elapsed - expectedMs
        v = phase2 + (extra / (expectedMs * 10)) * (p3Cap - phase2)
        v = Math.min(v, p3Cap)
      }

      setProgress(v)
    }

    tick()
    const id = setInterval(tick, 200)
    return () => clearInterval(id)
  }, [isDone, isActive, expectedMs])

  return progress
}
