/**
 * Exact bigint arithmetic for token base units → human display.
 * Avoids Number()/parseFloat precision loss on large micro-amounts.
 */

import { expandScientificNotationToDecimalString } from './scientificDecimal'
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

const MICRO_DECIMAL_RE = /^(-?)(\d*)(?:\.(\d+))?$/

/**
 * Parse a decimal string (optional fraction) as rational n/d in base-unit terms.
 */
export function parseMicroDecimalStringToRational(trimmed: string): { neg: boolean; n: bigint; d: bigint } | null {
  let t = trimmed.trim()
  if (t === '' || t === '+' || t === '-' || t === '.') return null
  if (/[eE]/.test(t)) {
    t = expandScientificNotationToDecimalString(t)
  }
  const m = t.match(MICRO_DECIMAL_RE)
  if (!m) return null
  const neg = m[1] === '-'
  let intPart = m[2] ?? ''
  const fracPart = m[3] ?? ''
  if (intPart === '' && fracPart === '') return null
  if (intPart === '') intPart = '0'
  if (!/^\d+$/.test(intPart) || (fracPart !== '' && !/^\d+$/.test(fracPart))) return null

  if (fracPart === '') {
    try {
      return { neg, n: BigInt(intPart), d: 1n }
    } catch {
      return null
    }
  }

  const den = pow10BigInt(fracPart.length)
  try {
    const n = BigInt(intPart) * den + BigInt(fracPart)
    return { neg, n, d: den }
  } catch {
    return null
  }
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

function numberToMicroRational(n: number): { neg: boolean; n: bigint; d: bigint } | null {
  if (!Number.isFinite(n)) return null
  if (n === 0) return { neg: false, n: 0n, d: 1n }
  const neg = n < 0
  const ax = Math.abs(n)
  if (Number.isInteger(ax) && ax <= Number.MAX_SAFE_INTEGER) {
    return { neg, n: BigInt(ax), d: 1n }
  }
  let s = ax.toString()
  if (!/[eE]/.test(s) && s.includes('.')) {
    const r = parseMicroDecimalStringToRational(s)
    if (r) return { neg, n: r.n, d: r.d }
  }
  if (ax >= 1e-15 && ax < 1e15) {
    s = ax.toFixed(30).replace(/\.?0+$/, '')
    const r = parseMicroDecimalStringToRational(s)
    if (r) return { neg, n: r.n, d: r.d }
  }
  s = ax.toString()
  if (/[eE]/.test(s)) {
    const expanded = expandScientificNotationToDecimalString(s)
    const r = parseMicroDecimalStringToRational(expanded)
    if (!r) return null
    return { neg, n: r.n, d: r.d }
  }
  const r = parseMicroDecimalStringToRational(s)
  if (!r) return null
  return { neg, n: r.n, d: r.d }
}

export type MicroRational = { neg: boolean; n: bigint; d: bigint }

/**
 * Parse micro amount (base units, possibly fractional) as rational n/d in base-unit terms.
 */
export function tryParseMicroRational(microAmount: string | number | bigint): MicroRational | null {
  if (typeof microAmount === 'bigint') {
    if (microAmount === 0n) return { neg: false, n: 0n, d: 1n }
    const neg = microAmount < 0n
    const n = neg ? -microAmount : microAmount
    return { neg, n, d: 1n }
  }
  if (typeof microAmount === 'number') {
    return numberToMicroRational(microAmount)
  }
  return parseMicroDecimalStringToRational(microAmount)
}

/** Denominator of human amount = n / (d * 10^tokenDecimals) */
export function microRationalToHumanDenominator(micro: MicroRational, tokenDecimals: number): bigint {
  return micro.d * pow10BigInt(tokenDecimals)
}

/**
 * Round signed rational numer/denom to `maxFrac` fractional digits (half-up), format en-US.
 * `denom` > 0.
 */
export function formatRationalHumanEnUs(
  numer: bigint,
  denom: bigint,
  maxFrac: number,
  minFrac: number,
  useGrouping: boolean
): string {
  if (denom <= 0n) return '0'

  const neg = numer < 0n
  const n = neg ? -numer : numer
  if (n === 0n) {
    if (maxFrac === 0) return '0'
    const pad = '0'.repeat(minFrac)
    return `${neg ? '-' : ''}0.${pad}`
  }

  const scaleOut = pow10BigInt(maxFrac)
  const half = denom / 2n
  const roundedScaled = (n * scaleOut + half) / denom

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
  return formatRationalHumanEnUs(amount, pow10BigInt(tokenDecimals), maxFrac, minFrac, useGrouping)
}

const COMPACT_INTERNAL_SCALE = 8

/**
 * `numer / denom` as a human-readable mantissa, rounded to ~`sigfigs` significant figures.
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

/** Long-division fractional digits for 0 < n < d → string of digits (no "0."). */
export function longDivisionFractionDigits(n: bigint, d: bigint, maxDigits: number): string {
  let rem = n
  let out = ''
  for (let i = 0; i < maxDigits; i++) {
    rem *= 10n
    const q = rem / d
    out += q.toString()
    rem = rem % d
    if (rem === 0n) break
  }
  return out
}

/**
 * Format 0 < human = n/d < 0.0001 with plain decimals (no parseFloat on the full value).
 */
export function formatCompactTinyFromRational(n: bigint, d: bigint, sigfigs: number): string {
  if (n === 0n) return '0'
  const digits = longDivisionFractionDigits(n, d, Math.min(200, sigfigs + 80))
  const firstNonZero = digits.search(/[1-9]/)
  if (firstNonZero < 0) return '0'
  const decimalPlaces = Math.max(firstNonZero + sigfigs - 1, 1)
  const frac = digits.padEnd(decimalPlaces, '0').slice(0, decimalPlaces)
  const formatted = `0.${frac}`
  return formatted.replace(/(\.\d*?)0+$/, '$1').replace(/\.$/, '')
}

/**
 * Compact label for human amount = hn / hd (hd > 0), with k/m/b suffixes.
 */
export function formatCompactHumanRational(hn: bigint, hd: bigint, sigfigs: number = 4): string {
  if (hd <= 0n) return '0'
  const neg = hn < 0n
  const n = neg ? -hn : hn
  if (n === 0n) return '0'

  if (n * 10000n < hd) {
    let t = formatCompactTinyFromRational(n, hd, sigfigs)
    if (neg && t !== '0') t = `-${t}`
    return t
  }

  const thK = 1000n * hd
  const thM = 1_000_000n * hd
  const thB = 1_000_000_000n * hd

  let unit: bigint
  let suffix: string
  if (n >= thB) {
    unit = thB
    suffix = 'b'
  } else if (n >= thM) {
    unit = thM
    suffix = 'm'
  } else if (n >= thK) {
    unit = thK
    suffix = 'k'
  } else {
    const m = formatCompactMantissa(n, hd, sigfigs)
    return neg ? `-${m}` : m
  }

  const m = formatCompactMantissa(n, unit, sigfigs)
  return (neg ? `-${m}` : m) + suffix
}

/**
 * Compact label for integer base-units bigint.
 */
export function formatCompactBigInt(amount: bigint, decimals: number, sigfigs: number = 4): string {
  const hd = pow10BigInt(decimals)
  return formatCompactHumanRational(amount, hd, sigfigs)
}
