import { pow10BigInt } from './pow10'

/**
 * Normalize a bridge pending-withdraw amount from source token decimals to
 * destination (local) token decimals. Matches EVM `Bridge._normalizeDecimals`
 * and Terra payout normalization.
 */
export function normalizeBridgeAmountToDestDecimals(
  amount: bigint,
  srcDecimals: number,
  destDecimals: number,
): bigint {
  if (srcDecimals === destDecimals) return amount
  if (srcDecimals > destDecimals) {
    return amount / pow10BigInt(srcDecimals - destDecimals)
  }
  return amount * pow10BigInt(destDecimals - srcDecimals)
}
