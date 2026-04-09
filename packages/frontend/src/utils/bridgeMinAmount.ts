/**
 * Bridge deposit fee uses integer bps: fee = floor(gross * feeBps / 10000), net = gross - fee.
 * Returns the smallest gross such that net >= targetNet (in the same base units).
 */
export function minGrossForMinNet(targetNet: bigint, feeBps: bigint): bigint {
  if (targetNet <= 0n) return 0n
  if (feeBps <= 0n) return targetNet
  if (feeBps >= 10000n) {
    // Degenerate: would need infinite gross; return target as best effort
    return targetNet
  }
  const denom = 10000n - feeBps
  let gross = (targetNet * 10000n + denom - 1n) / denom
  if (gross < targetNet) gross = targetNet
  while (gross - (gross * feeBps) / 10000n < targetNet) {
    gross += 1n
  }
  while (gross > targetNet) {
    const lower = gross - 1n
    if (lower - (lower * feeBps) / 10000n < targetNet) break
    gross = lower
  }
  return gross
}
