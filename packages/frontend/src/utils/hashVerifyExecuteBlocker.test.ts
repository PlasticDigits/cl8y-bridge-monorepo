import { describe, it, expect } from 'vitest'
import {
  computeHashVerificationStatus,
  destExecuteBlockerKind,
} from './hashVerifyExecuteBlocker'

describe('computeHashVerificationStatus', () => {
  it('keeps dest-approved not-executed as pending (not verified)', () => {
    expect(
      computeHashVerificationStatus(null, false, {
        cancelled: false,
        executed: false,
        approved: true,
      }, true),
    ).toBe('pending')
  })

  it('marks dest executed as verified', () => {
    expect(
      computeHashVerificationStatus(null, false, {
        cancelled: false,
        executed: true,
        approved: true,
      }, true),
    ).toBe('verified')
  })

  it('marks dest cancelled as canceled', () => {
    expect(
      computeHashVerificationStatus(null, false, {
        cancelled: true,
        executed: false,
        approved: true,
      }, true),
    ).toBe('canceled')
  })
})

describe('destExecuteBlockerKind', () => {
  const approved = {
    approved: true,
    executed: false,
    cancelled: false,
  }

  it('shows cancel remaining when the window is still open', () => {
    expect(
      destExecuteBlockerKind({
        ...approved,
        cancelWindowRemaining: 90,
        rateLimitKind: 'ok',
      }),
    ).toBe('cancel-window')
  })

  it('shows temporary period-full over cancel remaining', () => {
    expect(
      destExecuteBlockerKind({
        ...approved,
        cancelWindowRemaining: 10,
        rateLimitKind: 'temporarily-blocked',
      }),
    ).toBe('temporarily-blocked')
  })

  it('shows permanent over-max', () => {
    expect(
      destExecuteBlockerKind({
        ...approved,
        cancelWindowRemaining: 0,
        rateLimitKind: 'permanently-blocked',
      }),
    ).toBe('permanently-blocked')
  })

  it('shows awaiting-execute after the window when rate-limit is ok', () => {
    expect(
      destExecuteBlockerKind({
        ...approved,
        cancelWindowRemaining: 0,
        rateLimitKind: 'ok',
      }),
    ).toBe('awaiting-execute')
  })

  it('is none once dest is executed', () => {
    expect(
      destExecuteBlockerKind({
        approved: true,
        executed: true,
        cancelled: false,
        cancelWindowRemaining: 0,
        rateLimitKind: 'ok',
      }),
    ).toBe('none')
  })
})
