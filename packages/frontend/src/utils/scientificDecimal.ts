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
