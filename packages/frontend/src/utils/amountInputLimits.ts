/**
 * Amount field helpers: HTML5 `step` conflicts, display-vs-parse caps (GitLab #119).
 */

import { expandScientificNotationToDecimalString } from './scientificDecimal'
import { formatAmountForNumberInput, parseAmountAsBigInt } from './format'

/**
 * True when the human-entered string has more fractional digits than the token allows.
 * Trailing zeros after the last non-zero fractional digit are ignored (e.g. `1.200000` vs 2 dp).
 */
export function humanAmountHasExcessFractionDigits(raw: string, maxDecimals: number): boolean {
  if (maxDecimals < 0) return false
  let t = raw.trim()
  if (!t) return false
  if (/[eE]/.test(t)) {
    try {
      t = expandScientificNotationToDecimalString(t)
    } catch {
      return false
    }
  }
  const dot = t.indexOf('.')
  if (dot < 0) return false
  let frac = t.slice(dot + 1)
  if (frac === '') return false
  frac = frac.replace(/0+$/, '')
  if (frac.length === 0) return false
  return frac.length > maxDecimals
}

/**
 * Format a gross cap (base units) for `type="number"` such that parsing rounds down to ≤ cap.
 * Guards display rounding that could otherwise exceed the wallet / bridge limit (GitLab #119).
 */
export function formatCappedGrossForAmountInput(cap: bigint, decimals: number): string {
  if (cap <= 0n) return formatAmountForNumberInput(0n, decimals, decimals)
  let g = cap
  for (let i = 0; i < 4096 && g >= 0n; i++) {
    const s = formatAmountForNumberInput(g, decimals, decimals)
    if (!s) {
      if (g === 0n) return ''
      g -= 1n
      continue
    }
    const parsed = parseAmountAsBigInt(s, decimals)
    if (parsed <= cap) return s
    if (g === 0n) return s
    g -= 1n
  }
  return ''
}
