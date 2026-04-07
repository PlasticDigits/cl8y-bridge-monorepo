import { describe, it, expect } from 'vitest'
import { pow10BigInt } from './pow10'

describe('pow10BigInt', () => {
  it('returns 1 for exp 0', () => {
    expect(pow10BigInt(0)).toBe(1n)
  })

  it('matches expected values for typical token decimals', () => {
    expect(pow10BigInt(6)).toBe(1_000_000n)
    expect(pow10BigInt(9)).toBe(1_000_000_000n)
    expect(pow10BigInt(18)).toBe(BigInt('1000000000000000000'))
  })

  // JS double cannot represent 10^25 exactly; BigInt(10**25) is wrong vs true 10^25.
  it('matches exact 10^exp for exp where Number(10**exp) loses precision (e.g. 25)', () => {
    const exact = BigInt('10000000000000000000000000')
    expect(pow10BigInt(25)).toBe(exact)
    expect(BigInt(10 ** 25)).not.toBe(exact)
  })

  it('rejects invalid exp', () => {
    expect(() => pow10BigInt(-1)).toThrow(RangeError)
    expect(() => pow10BigInt(1.5)).toThrow(RangeError)
    expect(() => pow10BigInt(257)).toThrow(RangeError)
  })
})
