import { describe, it, expect } from 'vitest'
import { expandScientificNotationToDecimalString } from './scientificDecimal'

describe('expandScientificNotationToDecimalString', () => {
  it('expands large exponents', () => {
    expect(expandScientificNotationToDecimalString('1e+21')).toBe('1000000000000000000000')
  })

  it('passes through non-scientific strings', () => {
    expect(expandScientificNotationToDecimalString('123.45')).toBe('123.45')
  })
})
