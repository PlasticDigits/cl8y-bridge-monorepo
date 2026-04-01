import { describe, it, expect } from 'vitest'
import {
  bigintBaseUnitsToDecimalString,
  addThousandsGroupingEnUs,
  tryParseIntegerMicroString,
  formatBaseUnitsEnUs,
  formatCompactBigInt,
  formatCompactMantissa,
} from './bigintAmount'

describe('tryParseIntegerMicroString', () => {
  it('accepts bigint and integer strings', () => {
    expect(tryParseIntegerMicroString(42n)).toBe(42n)
    expect(tryParseIntegerMicroString('99')).toBe(99n)
    expect(tryParseIntegerMicroString('-1')).toBe(-1n)
  })

  it('rejects floats and non-integers', () => {
    expect(tryParseIntegerMicroString('1.5')).toBeNull()
    expect(tryParseIntegerMicroString(1.5)).toBeNull()
  })
})

describe('addThousandsGroupingEnUs', () => {
  it('groups long integers', () => {
    expect(addThousandsGroupingEnUs('1000000')).toBe('1,000,000')
    expect(addThousandsGroupingEnUs('-1000000')).toBe('-1,000,000')
  })
})

describe('bigintBaseUnitsToDecimalString', () => {
  it('pads fractional part', () => {
    expect(bigintBaseUnitsToDecimalString(1_500_000n, 6)).toBe('1.500000')
  })
})

describe('formatBaseUnitsEnUs', () => {
  it('formats amounts too large for exact Number() human conversion', () => {
    const base = BigInt('1000000000000000000000000')
    expect(formatBaseUnitsEnUs(base, 18, 2, 2, true)).toBe('1,000,000.00')
    expect(formatBaseUnitsEnUs(base, 18, 2, 2, false)).toBe('1000000.00')
  })
})

describe('formatCompactBigInt', () => {
  it('matches k-scale without coercing micro amount through Number', () => {
    expect(formatCompactBigInt(BigInt('500000000000000000000000'), 18, 4)).toBe('500k')
  })
})

describe('formatCompactMantissa', () => {
  it('handles mantissa below 1', () => {
    expect(formatCompactMantissa(123456n, 1_000_000n, 4)).toBe('0.1235')
  })
})
