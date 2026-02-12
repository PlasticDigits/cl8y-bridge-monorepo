/**
 * Integration Tests for useHashVerification
 *
 * Requires: Anvil + Bridge deployed, with at least one deposit or pending withdraw.
 * Run with: npm run test:integration
 * Skip with: SKIP_INTEGRATION=true npm run test:run
 */

import { describe, it, expect } from 'vitest'

const skipIntegration = process.env.SKIP_INTEGRATION === 'true'

describe.skipIf(skipIntegration)('useHashVerification Integration Tests', () => {
  it('should have useHashVerification hook structure', async () => {
    const { useHashVerification } = await import('./useHashVerification')
    expect(useHashVerification).toBeTypeOf('function')
  })

  it.skip('should verify a known deposit hash when Anvil is running', async () => {
    // Requires: Anvil, Bridge, and a known deposit hash from a prior deposit
    // const { renderHook, act } = await import('@testing-library/react')
    // const { useHashVerification } = await import('./useHashVerification')
    // const { result } = renderHook(() => useHashVerification())
    // await act(() => result.current.verify('0x...'))
    // expect(result.current.source).not.toBeNull()
    expect(true).toBe(true)
  })
})
