import { describe, it, expect } from 'vitest'
import { parseAmountAsBigInt } from './format'
import {
  formatBaseUnitsAsExactDecimalString,
  humanAmountHasExcessFractionDigits,
  formatCappedGrossForAmountInput,
} from './amountInputLimits'

describe('humanAmountHasExcessFractionDigits', () => {
  it('returns false when within precision', () => {
    expect(humanAmountHasExcessFractionDigits('1.123456', 6)).toBe(false)
    expect(humanAmountHasExcessFractionDigits('10', 18)).toBe(false)
  })

  it('returns true when fractional part exceeds token decimals', () => {
    expect(humanAmountHasExcessFractionDigits('1.1234567', 6)).toBe(true)
    expect(humanAmountHasExcessFractionDigits('0.00000001', 6)).toBe(true)
  })

  it('ignores trailing fractional zeros', () => {
    expect(humanAmountHasExcessFractionDigits('1.23000000', 2)).toBe(false)
  })
})

describe('formatCappedGrossForAmountInput', () => {
  it('returns a string that parses to at most cap', () => {
    const cap = 2063202n // 2.063202 with 6 decimals
    const s = formatCappedGrossForAmountInput(cap, 6)
    expect(s).not.toBe('')
    expect(parseAmountAsBigInt(s, 6) <= cap).toBe(true)
  })
})

describe('formatBaseUnitsAsExactDecimalString', () => {
  it('preserves full fractional width (matches floored parse)', () => {
    expect(formatBaseUnitsAsExactDecimalString(1_000_000n, 6)).toBe('1.000000')
    expect(formatBaseUnitsAsExactDecimalString(1n, 6)).toBe('0.000001')
  })
})
