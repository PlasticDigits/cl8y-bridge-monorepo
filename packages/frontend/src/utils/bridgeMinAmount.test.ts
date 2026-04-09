import { describe, expect, it } from 'vitest'
import { minGrossForMinNet } from './bridgeMinAmount'

function netOf(gross: bigint, feeBps: bigint): bigint {
  return gross - (gross * feeBps) / 10000n
}

describe('minGrossForMinNet', () => {
  it('returns target when fee bps is zero', () => {
    expect(minGrossForMinNet(100n, 0n)).toBe(100n)
  })

  it('matches integer fee rounding for 50 bps (0.5%)', () => {
    const feeBps = 50n
    const gross = minGrossForMinNet(1000n, feeBps)
    expect(netOf(gross, feeBps)).toBeGreaterThanOrEqual(1000n)
    expect(gross).toBeGreaterThan(0n)
    expect(netOf(gross - 1n, feeBps)).toBeLessThan(1000n)
  })

  it('adds headroom when target includes +1 buffer', () => {
    const feeBps = 50n
    const minNet = 1000n
    const withoutBuffer = minGrossForMinNet(minNet, feeBps)
    const withBuffer = minGrossForMinNet(minNet + 1n, feeBps)
    expect(netOf(withBuffer, feeBps)).toBeGreaterThanOrEqual(minNet + 1n)
    expect(withBuffer).toBeGreaterThanOrEqual(withoutBuffer)
  })
})
