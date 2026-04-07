/**
 * Gas/fee retry ladder for LocalTerra `terrad tx wasm instantiate` (CW20 mintable deploy).
 * Override with comma-separated env vars (same length):
 * - TERRA_E2E_INSTANTIATE_GAS_ADJUSTMENT e.g. `1.5,2.2,3.0`
 * - TERRA_E2E_INSTANTIATE_FEES_ULUNA e.g. `10000000,25000000,50000000` (amounts only; `uluna` appended)
 */
export type InstantiateFeeAttempt = { gasAdjustment: string; fees: string }

export function resolveCw20InstantiateAttempts(): InstantiateFeeAttempt[] {
  const adjRaw = process.env.TERRA_E2E_INSTANTIATE_GAS_ADJUSTMENT?.trim()
  const feesRaw = process.env.TERRA_E2E_INSTANTIATE_FEES_ULUNA?.trim()
  if (adjRaw && feesRaw) {
    const gasAdjs = adjRaw.split(',').map((s) => s.trim()).filter(Boolean)
    const feeParts = feesRaw.split(',').map((s) => s.trim()).filter(Boolean)
    const n = Math.min(gasAdjs.length, feeParts.length)
    const out: InstantiateFeeAttempt[] = []
    for (let i = 0; i < n; i++) {
      const f = feeParts[i]!
      const fees = f.endsWith('uluna') ? f : `${f}uluna`
      out.push({ gasAdjustment: gasAdjs[i]!, fees })
    }
    if (out.length > 0) return out
  }
  return [
    { gasAdjustment: '1.5', fees: '10000000uluna' },
    { gasAdjustment: '2.2', fees: '25000000uluna' },
    { gasAdjustment: '3.0', fees: '50000000uluna' },
  ]
}
