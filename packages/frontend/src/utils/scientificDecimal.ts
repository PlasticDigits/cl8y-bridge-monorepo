/**
 * Expands scientific notation (e.g. "1e+21", "1.5e-3") to a plain decimal string.
 * Used so downstream BigInt() never sees exponential form.
 */
export function expandScientificNotationToDecimalString(sci: string): string {
  const trimmed = sci.trim()
  const m = trimmed.match(/^(-?)(\d+(?:\.\d*)?)[eE]([-+]?\d+)$/)
  if (!m) return trimmed

  const neg = m[1] === '-'
  const coefficient = m[2]!
  const exp = parseInt(m[3]!, 10)
  if (!Number.isFinite(exp)) return '0'

  const coeffParts = coefficient.split('.')
  const intPartRaw = coeffParts[0] ?? ''
  const fracPartRaw = coeffParts[1] ?? ''
  const dotIndex = intPartRaw.length
  const allDigits = intPartRaw + fracPartRaw
  if (allDigits === '' || /^0+$/.test(allDigits)) return '0'

  const sign = neg ? '-' : ''

  if (exp === 0) {
    const t: string = fracPartRaw ? `${intPartRaw}.${fracPartRaw}` : intPartRaw
    return sign + (t.startsWith('.') ? `0${t}` : t)
  }

  if (exp > 0) {
    const newDot = dotIndex + exp
    if (newDot >= allDigits.length) {
      return sign + allDigits + '0'.repeat(newDot - allDigits.length)
    }
    if (newDot <= 0) {
      return sign + `0.${'0'.repeat(-newDot)}${allDigits}`
    }
    return sign + allDigits.slice(0, newDot) + '.' + allDigits.slice(newDot)
  }

  const newDot = dotIndex + exp
  if (newDot <= 0) {
    return sign + `0.${'0'.repeat(-newDot)}${allDigits}`
  }
  return sign + allDigits.slice(0, newDot) + '.' + allDigits.slice(newDot)
}

/**
 * Parse on-chain / API base-unit amounts (integer strings, possibly from JSON `Number` re-encoding).
 * Expands scientific notation before `BigInt` so values like `"1e+21"` never throw (GitLab #95).
 * Truncates toward zero at `.` if a fractional part appears after expansion.
 */
export function bigintFromBaseUnitsString(
  value: string | number | bigint | null | undefined
): bigint {
  if (value === null || value === undefined) return 0n
  if (typeof value === 'bigint') return value
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new SyntaxError('bigintFromBaseUnitsString: non-finite number')
    }
    if (Number.isInteger(value) && Number.isSafeInteger(value)) {
      return BigInt(value)
    }
    return bigintFromBaseUnitsString(String(value))
  }

  const trimmed = value.trim()
  if (trimmed === '' || trimmed === '+' || trimmed === '-') return 0n

  let s = trimmed
  if (/[eE]/.test(s)) {
    s = expandScientificNotationToDecimalString(s).trim()
  }

  const neg = s.startsWith('-')
  let body = neg ? s.slice(1) : s
  if (body.startsWith('+')) body = body.slice(1).trim()

  const dotIdx = body.indexOf('.')
  const intSliceRaw = dotIdx >= 0 ? body.slice(0, dotIdx) : body
  const intSlice = intSliceRaw === '' ? '0' : intSliceRaw

  if (!/^\d+$/.test(intSlice)) {
    throw new SyntaxError(
      `bigintFromBaseUnitsString: expected integer base units, got ${JSON.stringify(value)}`
    )
  }

  const normalized = intSlice.replace(/^0+/, '') || '0'
  const signed = neg && normalized !== '0' ? `-${normalized}` : normalized

  try {
    return BigInt(signed)
  } catch {
    throw new SyntaxError(
      `bigintFromBaseUnitsString: cannot convert to BigInt: ${JSON.stringify(value)}`
    )
  }
}
