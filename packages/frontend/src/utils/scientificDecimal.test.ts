import { describe, it, expect } from 'vitest'
import {
  bigintFromBaseUnitsString,
  expandScientificNotationToDecimalString,
} from './scientificDecimal'

describe('expandScientificNotationToDecimalString', () => {
  it('expands large exponents', () => {
    expect(expandScientificNotationToDecimalString('1e+21')).toBe('1000000000000000000000')
  })

  it('passes through non-scientific strings', () => {
    expect(expandScientificNotationToDecimalString('123.45')).toBe('123.45')
  })
})

describe('bigintFromBaseUnitsString', () => {
  it('parses plain integer strings', () => {
    expect(bigintFromBaseUnitsString('1000000000000000000000')).toBe(
      1_000_000_000_000_000_000_000n
    )
    expect(bigintFromBaseUnitsString('0')).toBe(0n)
  })

  it('expands scientific notation (GitLab #95)', () => {
    expect(bigintFromBaseUnitsString('1e+21')).toBe(1_000_000_000_000_000_000_000n)
    expect(bigintFromBaseUnitsString(1e21)).toBe(1_000_000_000_000_000_000_000n)
  })

  it('truncates fractional base units toward zero', () => {
    expect(bigintFromBaseUnitsString('1000.9')).toBe(1000n)
    expect(bigintFromBaseUnitsString('-1000.9')).toBe(-1000n)
  })

  it('handles null/undefined as zero', () => {
    expect(bigintFromBaseUnitsString(null)).toBe(0n)
    expect(bigintFromBaseUnitsString(undefined)).toBe(0n)
  })

  it('rejects non-integer garbage', () => {
    expect(() => bigintFromBaseUnitsString('abc')).toThrow(SyntaxError)
  })
})
