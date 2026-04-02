import { afterEach, describe, expect, it } from 'vitest'
import { resolveCw20InstantiateAttempts } from './terraCw20InstantiateAttempts'

describe('resolveCw20InstantiateAttempts', () => {
  const prevAdj = process.env.TERRA_E2E_INSTANTIATE_GAS_ADJUSTMENT
  const prevFees = process.env.TERRA_E2E_INSTANTIATE_FEES_ULUNA

  afterEach(() => {
    if (prevAdj === undefined) delete process.env.TERRA_E2E_INSTANTIATE_GAS_ADJUSTMENT
    else process.env.TERRA_E2E_INSTANTIATE_GAS_ADJUSTMENT = prevAdj
    if (prevFees === undefined) delete process.env.TERRA_E2E_INSTANTIATE_FEES_ULUNA
    else process.env.TERRA_E2E_INSTANTIATE_FEES_ULUNA = prevFees
  })

  it('returns default three-step ladder', () => {
    delete process.env.TERRA_E2E_INSTANTIATE_GAS_ADJUSTMENT
    delete process.env.TERRA_E2E_INSTANTIATE_FEES_ULUNA
    const a = resolveCw20InstantiateAttempts()
    expect(a).toHaveLength(3)
    expect(a[0]).toEqual({ gasAdjustment: '1.5', fees: '10000000uluna' })
    expect(a[2]).toEqual({ gasAdjustment: '3.0', fees: '50000000uluna' })
  })

  it('parses env override pairs', () => {
    process.env.TERRA_E2E_INSTANTIATE_GAS_ADJUSTMENT = '2,3'
    process.env.TERRA_E2E_INSTANTIATE_FEES_ULUNA = '20000000,40000000'
    expect(resolveCw20InstantiateAttempts()).toEqual([
      { gasAdjustment: '2', fees: '20000000uluna' },
      { gasAdjustment: '3', fees: '40000000uluna' },
    ])
  })
})
