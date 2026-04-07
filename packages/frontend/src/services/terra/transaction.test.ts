import { describe, it, expect } from 'vitest'
import {
  handleSwapTransactionError,
  TERRA_TX_ERROR,
  TerraTxError,
} from './transaction'

describe('handleSwapTransactionError', () => {
  it('classifies Terra contract "Withdrawal already submitted" as WITHDRAW_ALREADY_SUBMITTED (#87)', () => {
    const err = handleSwapTransactionError(new Error('Withdrawal already submitted'))
    expect(err).toBeInstanceOf(TerraTxError)
    expect(err.code).toBe(TERRA_TX_ERROR.WITHDRAW_ALREADY_SUBMITTED)
  })

  it('still classifies "withdraw already submitted" spelling', () => {
    const err = handleSwapTransactionError(new Error('Generic error: withdraw already submitted'))
    expect(err.code).toBe(TERRA_TX_ERROR.WITHDRAW_ALREADY_SUBMITTED)
  })

  it('classifies execute wasm failure containing Withdrawal already submitted before generic CONTRACT_ERROR', () => {
    const raw =
      'failed to execute message; message index: 0: execute wasm contract failed: ' +
      'execute wasm contract failed: Generic error: Withdrawal already submitted'
    const err = handleSwapTransactionError(new Error(raw))
    expect(err.code).toBe(TERRA_TX_ERROR.WITHDRAW_ALREADY_SUBMITTED)
  })
})
