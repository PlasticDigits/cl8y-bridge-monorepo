import { describe, it, expect } from 'vitest'
import { normalizeBridgeAmountToDestDecimals } from './bridgeAmountDecimals'

describe('normalizeBridgeAmountToDestDecimals', () => {
  it('returns same when decimals match', () => {
    expect(normalizeBridgeAmountToDestDecimals(1_000_000n, 6, 6)).toBe(1_000_000n)
  })

  it('scales down when srcDecimals > destDecimals', () => {
    expect(normalizeBridgeAmountToDestDecimals(1_000_000n, 6, 0)).toBe(1n)
  })

  it('scales up when destDecimals > srcDecimals', () => {
    expect(normalizeBridgeAmountToDestDecimals(1n, 0, 6)).toBe(1_000_000n)
  })
})
