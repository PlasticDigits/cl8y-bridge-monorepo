import { describe, it, expect } from 'vitest'
import { computeTransferStepIdx } from './transferStatusStep'

describe('computeTransferStepIdx (GitLab #131)', () => {
  const baseArgs = {
    transferLifecycle: 'deposited' as const,
    destIsSolana: false,
    effectiveCancelWindowRemaining: 0,
    autoPhase: 'idle' as const,
    retryingHash: false,
    source: { stub: true } as unknown,
    dest: null,
    lookupLoading: false,
  }

  it('stays at Submit Hash (idx 1) across lookupLoading toggles when deposited, source present, dest absent', () => {
    expect(computeTransferStepIdx({ ...baseArgs, lookupLoading: false })).toBe(1)
    expect(computeTransferStepIdx({ ...baseArgs, lookupLoading: true })).toBe(1)
    expect(computeTransferStepIdx({ ...baseArgs, lookupLoading: false })).toBe(1)
  })

  it('falls back to base deposit step (0) when source is missing', () => {
    expect(
      computeTransferStepIdx({
        ...baseArgs,
        source: null,
        lookupLoading: false,
      }),
    ).toBe(0)
  })
})
