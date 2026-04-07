/**
 * BigInt powers of ten without going through JS Number.
 *
 * `BigInt(10 ** exp)` is unsafe for larger `exp`: `10 ** exp` is a double (precision loss)
 * and may stringify as scientific notation, which `BigInt()` cannot parse.
 */

/** Token decimals and cross-chain decimal diffs stay well below this. */
const POW10_MAX_EXP = 256

/**
 * Returns 10^exp for non-negative integer exp, using bigint exponentiation only.
 */
export function pow10BigInt(exp: number): bigint {
  if (!Number.isInteger(exp) || exp < 0) {
    throw new RangeError(`pow10BigInt: exp must be a non-negative integer, got ${exp}`)
  }
  if (exp > POW10_MAX_EXP) {
    throw new RangeError(`pow10BigInt: exp ${exp} exceeds max ${POW10_MAX_EXP}`)
  }
  if (exp === 0) return 1n
  return 10n ** BigInt(exp)
}
