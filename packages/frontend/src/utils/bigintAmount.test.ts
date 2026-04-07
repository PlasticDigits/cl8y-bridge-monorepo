import { describe, it, expect } from 'vitest'
import {
  bigintBaseUnitsToDecimalString,
  addThousandsGroupingEnUs,
  tryParseIntegerMicroString,
  tryParseMicroRational,
  parseMicroDecimalStringToRational,
  formatBaseUnitsEnUs,
  formatCompactBigInt,
  formatCompactHumanRational,
  formatCompactMantissa,
  formatRationalHumanEnUs,
  longDivisionFractionDigits,
  formatCompactTinyFromRational,
  microRationalToHumanDenominator,
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

describe('parseMicroDecimalStringToRational', () => {
  it('parses integers and fractions', () => {
    expect(parseMicroDecimalStringToRational('123.45')).toEqual({ neg: false, n: 12345n, d: 100n })
    expect(parseMicroDecimalStringToRational('.5')).toEqual({ neg: false, n: 5n, d: 10n })
    expect(parseMicroDecimalStringToRational('-0.25')).toEqual({ neg: true, n: 25n, d: 100n })
  })
})

describe('tryParseMicroRational', () => {
  it('normalizes bigint zero', () => {
    expect(tryParseMicroRational(0n)).toEqual({ neg: false, n: 0n, d: 1n })
  })

  it('parses fractional number in safe range', () => {
    const r = tryParseMicroRational(1.25)
    expect(r).not.toBeNull()
    expect(r!.n).toBe(125n)
    expect(r!.d).toBe(100n)
    expect(r!.neg).toBe(false)
  })
})

describe('microRationalToHumanDenominator', () => {
  it('multiplies micro denominator by 10^tokenDecimals', () => {
    const r = { neg: false, n: 5n, d: 10n }
    expect(microRationalToHumanDenominator(r, 6)).toBe(10_000_000n)
  })
})

describe('formatRationalHumanEnUs', () => {
  it('formats half-unit human from rational', () => {
    const hd = 10_000_000n
    expect(formatRationalHumanEnUs(5n, hd, 9, 2, false)).toBe('0.0000005')
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

describe('longDivisionFractionDigits', () => {
  it('produces digits for 12/1e6', () => {
    expect(longDivisionFractionDigits(12n, 1_000_000n, 12)).toBe('000012')
  })
})

describe('formatCompactTinyFromRational', () => {
  it('avoids parseFloat on full human value', () => {
    expect(formatCompactTinyFromRational(12n, 1_000_000n, 4)).toBe('0.000012')
  })
})

describe('formatCompactHumanRational', () => {
  it('uses tiny path for sub-0.0001 human', () => {
    expect(formatCompactHumanRational(12n, 1_000_000n, 4)).toBe('0.000012')
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

describe('consistency: integer string vs bigint', () => {
  it('matches formatRationalHumanEnUs for same magnitude', () => {
    const s = tryParseMicroRational('1500000')!
    const hd = microRationalToHumanDenominator(s, 6)
    const hn = s.neg ? -s.n : s.n
    expect(formatRationalHumanEnUs(hn, hd, 2, 2, false)).toBe(formatBaseUnitsEnUs(1_500_000n, 6, 2, 2, false))
  })
})
