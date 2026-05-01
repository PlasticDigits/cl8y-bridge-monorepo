import { describe, it, expect } from 'vitest'
import { computeEvmExecutionRateLimitStatus } from './evmExecutionRateLimit'
import type { PendingWithdrawData } from '../hooks/useTransferLookup'
import type { WithdrawRateLimitInfo } from '../hooks/useBridgeConfig'

function makeDest(overrides: Partial<PendingWithdrawData> = {}): PendingWithdrawData {
  return {
    chainId: 1,
    srcChain: '0x01',
    destChain: '0x02',
    srcAccount: '0x03',
    destAccount: '0x04',
    token: '0x05',
    amount: 1_000_000n,
    nonce: 1n,
    submittedAt: 1n,
    approvedAt: 1n,
    approved: true,
    cancelled: false,
    executed: false,
    srcDecimals: 6,
    destDecimals: 6,
    ...overrides,
  }
}

function makeRl(overrides: Partial<WithdrawRateLimitInfo> = {}): WithdrawRateLimitInfo {
  const base: WithdrawRateLimitInfo = {
    maxPerPeriod: '5000000',
    usedAmount: '4000000',
    remainingAmount: '1000000',
    periodEndsAt: 1_800_000_000,
    fetchedAt: 1_700_000_000,
    fetchedAtWallMs: 1_700_000_000_000,
    windowActive: true,
  }
  return { ...base, ...overrides }
}

describe('computeEvmExecutionRateLimitStatus', () => {
  it('returns unknown when no withdraw rate limit snapshot', () => {
    expect(computeEvmExecutionRateLimitStatus(makeDest(), null).kind).toBe('unknown')
  })

  it('returns unknown when maxPerPeriod is zero', () => {
    const rl = makeRl({ maxPerPeriod: '0', remainingAmount: '0' })
    expect(computeEvmExecutionRateLimitStatus(makeDest(), rl).kind).toBe('unknown')
  })

  it('returns permanently-blocked when payout exceeds max per period', () => {
    const dest = makeDest({ amount: 10_000_000n })
    const rl = makeRl({ maxPerPeriod: '5000000', remainingAmount: '5000000' })
    const s = computeEvmExecutionRateLimitStatus(dest, rl)
    expect(s.kind).toBe('permanently-blocked')
    if (s.kind === 'permanently-blocked') {
      expect(s.maxPerPeriod).toBe('5000000')
    }
  })

  it('returns temporarily-blocked when payout exceeds remaining in window', () => {
    const dest = makeDest({ amount: 2_000_000n })
    const rl = makeRl()
    const s = computeEvmExecutionRateLimitStatus(dest, rl)
    expect(s.kind).toBe('temporarily-blocked')
    if (s.kind === 'temporarily-blocked') {
      expect(s.periodEndsAt).toBe(1_800_000_000)
      expect(s.fetchedAtWallMs).toBe(1_700_000_000_000)
    }
  })

  it('returns ok when payout fits remaining', () => {
    const dest = makeDest({ amount: 500_000n })
    const rl = makeRl()
    expect(computeEvmExecutionRateLimitStatus(dest, rl).kind).toBe('ok')
  })

  it('normalizes decimals like the bridge contract', () => {
    const scale = 10n ** 12n // 18 src decimals → 6 dest
    const dest = makeDest({ amount: 600_000n * scale, srcDecimals: 18, destDecimals: 6 })
    const rl = makeRl({ maxPerPeriod: '5000000', remainingAmount: '500', usedAmount: '0' })
    const s = computeEvmExecutionRateLimitStatus(dest, rl)
    expect(s.kind).toBe('temporarily-blocked')
  })
})
