/**
 * Exact bigint arithmetic for token base units → human display.
 * Avoids Number()/parseFloat precision loss on large micro-amounts.
 */

import { pow10BigInt } from './pow10'

/** Full decimal string for signed base-units amount (frac padded to `decimals` places). */
export function bigintBaseUnitsToDecimalString(amount: bigint, decimals: number): string {
  const neg = amount < 0n
  const a = neg ? -amount : amount
  const d = pow10BigInt(decimals)
  const whole = a / d
  const frac = (a % d).toString().padStart(decimals, '0')
  const sign = neg ? '-' : ''
  if (decimals === 0) return `${sign}${whole}`
  return `${sign}${whole}.${frac}`
}

/** Add US-style thousands separators to an integer digit string (optional leading '-'). */
export function addThousandsGroupingEnUs(intDigits: string): string {
  const neg = intDigits.startsWith('-')
  let s = neg ? intDigits.slice(1) : intDigits
  if (s === '') s = '0'
  const out: string[] = []
  while (s.length > 3) {
    out.unshift(s.slice(-3))
    s = s.slice(0, -3)
  }
  if (s.length > 0) out.unshift(s)
  const grouped = out.join(',')
  return neg ? `-${grouped}` : grouped
}

/**
 * Integer base-units string (optional '-'), or null if not purely integral decimal.
 */
export function tryParseIntegerMicroString(microAmount: string | number | bigint): bigint | null {
  if (typeof microAmount === 'bigint') return microAmount
  if (typeof microAmount === 'number') {
    if (!Number.isFinite(microAmount) || !Number.isInteger(microAmount)) return null
    if (Math.abs(microAmount) > Number.MAX_SAFE_INTEGER) return null
    return BigInt(microAmount)
  }
  const t = microAmount.trim()
  if (!/^-?\d+$/.test(t)) return null
  try {
    return BigInt(t)
  } catch {
    return null
  }
}

/**
 * Round base units to `maxFrac` fractional digits (half-up, |amount|), then format en-US.
 */
export function formatBaseUnitsEnUs(
  amount: bigint,
  tokenDecimals: number,
  maxFrac: number,
  minFrac: number,
  useGrouping: boolean
): string {
  const neg = amount < 0n
  const a = neg ? -amount : amount
  const scaleDec = pow10BigInt(tokenDecimals)
  const scaleOut = pow10BigInt(maxFrac)
  const roundedScaled = (a * scaleOut + scaleDec / 2n) / scaleDec

  const whole = roundedScaled / scaleOut
  let fracRaw = (roundedScaled % scaleOut).toString().padStart(maxFrac, '0')
  while (fracRaw.length > minFrac && fracRaw.endsWith('0')) {
    fracRaw = fracRaw.slice(0, -1)
  }

  const wholeStr = useGrouping ? addThousandsGroupingEnUs(whole.toString()) : whole.toString()
  const sign = neg ? '-' : ''
  if (maxFrac === 0) {
    return `${sign}${wholeStr}`
  }
  return `${sign}${wholeStr}.${fracRaw}`
}

const COMPACT_INTERNAL_SCALE = 8

/**
 * `numer / denom` as a human-readable mantissa, rounded to ~`sigfigs` significant figures.
 * `denom` positive; uses fixed-point scale then `Number` only on the bounded mantissa (< 1e15).
 */
export function formatCompactMantissa(numer: bigint, denom: bigint, sigfigs: number): string {
  if (numer === 0n) return '0'
  const neg = numer < 0n
  const n = neg ? -numer : numer
  const half = denom / 2n
  const scale = pow10BigInt(COMPACT_INTERNAL_SCALE)
  const v = (n * scale + half) / denom
  const intPart = v / scale
  const fracRaw = (v % scale).toString().padStart(COMPACT_INTERNAL_SCALE, '0')
  const trimmedFrac = fracRaw.replace(/0+$/, '')
  let s: string
  if (intPart === 0n) {
    s = trimmedFrac ? `0.${trimmedFrac}` : '0'
  } else {
    s = trimmedFrac ? `${intPart}.${trimmedFrac}` : intPart.toString()
  }
  const num = parseFloat(s)
  if (!Number.isFinite(num)) return '0'
  const out = numToSigFigPositive(num, sigfigs)
  return neg ? `-${out}` : out
}

function numToSigFigPositive(n: number, sigfigs: number): string {
  if (n === 0) return '0'
  const s = n.toPrecision(sigfigs)
  return parseFloat(s).toString()
}

/**
 * Compact label for base-units bigint (matches legacy `formatCompact` behavior for tested cases).
 */
export function formatCompactBigInt(amount: bigint, decimals: number, sigfigs: number = 4): string {
  const neg = amount < 0n
  const a = neg ? -amount : amount
  const d = pow10BigInt(decimals)
  if (a === 0n) return '0'

  if (a * 10000n < d) {
    const humanStr = bigintBaseUnitsToDecimalString(a, decimals)
    let t = formatCompactTinyHumanString(humanStr, sigfigs)
    if (neg && t !== '0') t = `-${t}`
    return t
  }

  const thK = d * 1000n
  const thM = d * 1_000_000n
  const thB = d * 1_000_000_000n

  let unit: bigint
  let suffix: string
  if (a >= thB) {
    unit = thB
    suffix = 'b'
  } else if (a >= thM) {
    unit = thM
    suffix = 'm'
  } else if (a >= thK) {
    unit = thK
    suffix = 'k'
  } else {
    const m = formatCompactMantissa(a, d, sigfigs)
    return neg ? `-${m}` : m
  }

  const m = formatCompactMantissa(a, unit, sigfigs)
  return (neg ? `-${m}` : m) + suffix
}

/** Human decimal string (e.g. "0.0000123"); must satisfy |value| < 0.0001. */
function formatCompactTinyHumanString(humanStr: string, sigfigs: number): string {
  const n = parseFloat(humanStr)
  if (!Number.isFinite(n) || n === 0) return '0'
  const abs = Math.abs(n)
  const exp = Math.floor(Math.log10(abs))
  const decimalPlaces = Math.max(-exp + (sigfigs - 1), 1)
  const formatted = n.toFixed(decimalPlaces)
  return formatted.replace(/(\.\d*?)0+$/, '$1').replace(/\.$/, '')
}
